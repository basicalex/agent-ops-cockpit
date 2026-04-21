//! Hub transport runtime glue.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

#[cfg(not(unix))]
pub(crate) async fn hub_loop(
    _config: Config,
    tx: mpsc::Sender<HubEvent>,
    mut command_rx: mpsc::Receiver<HubOutbound>,
) {
    let _ = tx.send(HubEvent::Disconnected).await;
    while command_rx.recv().await.is_some() {}
}

#[cfg(unix)]
pub(crate) async fn hub_loop(
    config: Config,
    tx: mpsc::Sender<HubEvent>,
    mut command_rx: mpsc::Receiver<HubOutbound>,
) {
    let mut backoff = Duration::from_secs(1);
    let mut command_open = true;

    loop {
        let stream = match UnixStream::connect(&config.pulse_socket_path).await {
            Ok(stream) => stream,
            Err(err) => {
                warn!("pulse_connect_error: {err}");
                tokio::time::sleep(backoff).await;
                backoff = next_backoff(backoff);
                continue;
            }
        };
        backoff = Duration::from_secs(1);

        let (reader_half, mut writer_half) = stream.into_split();
        let hello = build_pulse_hello(&config);
        if send_wire_envelope(&mut writer_half, &hello).await.is_err() {
            tokio::time::sleep(backoff).await;
            backoff = next_backoff(backoff);
            continue;
        }
        let subscribe = build_pulse_subscribe(&config);
        if send_wire_envelope(&mut writer_half, &subscribe)
            .await
            .is_err()
        {
            tokio::time::sleep(backoff).await;
            backoff = next_backoff(backoff);
            continue;
        }

        let _ = tx.send(HubEvent::Connected).await;
        let mut reader = BufReader::new(reader_half);
        let mut decoder = NdjsonFrameDecoder::<WireEnvelope>::new(DEFAULT_MAX_FRAME_BYTES);
        let mut read_buf = [0u8; 8192];
        let mut last_seq = 0u64;
        let mut reconnect_requested = false;

        loop {
            tokio::select! {
                read = reader.read(&mut read_buf) => {
                    let read = match read {
                        Ok(value) => value,
                        Err(err) => {
                            warn!("pulse_read_error: {err}");
                            break;
                        }
                    };
                    if read == 0 {
                        break;
                    }
                    let report = decoder.push_chunk(&read_buf[..read]);
                    for err in report.errors {
                        warn!("pulse_decode_error: {err}");
                    }
                    for envelope in report.frames {
                        if envelope.session_id != config.session_id {
                            continue;
                        }
                        if envelope.version.0 > CURRENT_PROTOCOL_VERSION {
                            continue;
                        }
                        let event_at = parse_event_at(&envelope.timestamp);
                        match envelope.msg {
                            WireMsg::Snapshot(payload) => {
                                last_seq = payload.seq;
                                let _ = tx.send(HubEvent::Snapshot { payload, event_at }).await;
                            }
                            WireMsg::Delta(payload) => {
                                if payload.seq <= last_seq {
                                    continue;
                                }
                                if last_seq > 0 && payload.seq > last_seq + 1 {
                                    warn!("pulse_delta_gap: last_seq={last_seq} next_seq={}", payload.seq);
                                    reconnect_requested = true;
                                    break;
                                }
                                last_seq = payload.seq;
                                let _ = tx.send(HubEvent::Delta { payload, event_at }).await;
                            }
                            WireMsg::ObserverSnapshot(payload) => {
                                let _ = tx.send(HubEvent::ObserverSnapshot { payload }).await;
                            }
                            WireMsg::ObserverTimeline(payload) => {
                                let _ = tx.send(HubEvent::ObserverTimeline { payload }).await;
                            }
                            WireMsg::LayoutState(payload) => {
                                let _ = tx.send(HubEvent::LayoutState { payload }).await;
                            }
                            WireMsg::Heartbeat(payload) => {
                                let _ = tx.send(HubEvent::Heartbeat { payload, event_at }).await;
                            }
                            WireMsg::CommandResult(payload) => {
                                let _ = tx
                                    .send(HubEvent::CommandResult {
                                        payload,
                                        request_id: envelope.request_id,
                                    })
                                    .await;
                            }
                            WireMsg::ConsultationResponse(payload) => {
                                let _ = tx
                                    .send(HubEvent::ConsultationResponse {
                                        payload,
                                        request_id: envelope.request_id,
                                    })
                                    .await;
                            }
                            _ => {}
                        }
                    }
                }
                maybe_command = command_rx.recv(), if command_open => {
                    match maybe_command {
                        Some(command) => {
                            let envelope = build_outbound_envelope(&config, command);
                            if send_wire_envelope(&mut writer_half, &envelope).await.is_err() {
                                break;
                            }
                        }
                        None => {
                            command_open = false;
                        }
                    }
                }
            }

            if reconnect_requested {
                break;
            }
        }

        let final_report = decoder.finish();
        for err in final_report.errors {
            warn!("pulse_decode_error: {err}");
        }
        let _ = tx.send(HubEvent::Disconnected).await;
        tokio::time::sleep(backoff).await;
        backoff = next_backoff(backoff);
    }
}

#[cfg(unix)]
pub(crate) async fn send_wire_envelope(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    envelope: &WireEnvelope,
) -> io::Result<()> {
    let frame = encode_frame(envelope, DEFAULT_MAX_FRAME_BYTES)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    writer.write_all(&frame).await?;
    writer.flush().await
}
