//! Wire protocol envelope builders for Pulse IPC.
//!
//! Extracted from main.rs (Phase 2).

use super::*;
use chrono::Utc;

pub(crate) fn build_pulse_hello(config: &Config) -> WireEnvelope {
    let capabilities = vec![
        "snapshot".to_string(),
        "delta".to_string(),
        "heartbeat".to_string(),
        "command".to_string(),
        "command_result".to_string(),
    ];

    WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: config.session_id.clone(),
        sender_id: config.client_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: None,
        msg: WireMsg::Hello(PulseHelloPayload {
            client_id: config.client_id.clone(),
            role: "subscriber".to_string(),
            capabilities,
            agent_id: None,
            pane_id: None,
            project_root: Some(config.project_root.to_string_lossy().to_string()),
        }),
    }
}

pub(crate) fn build_pulse_subscribe(config: &Config) -> WireEnvelope {
    let topics = vec![
        "agent_state".to_string(),
        "command_result".to_string(),
        "layout_state".to_string(),
        "consultation_response".to_string(),
        "observer_snapshot".to_string(),
        "observer_timeline".to_string(),
    ];

    WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: config.session_id.clone(),
        sender_id: config.client_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: None,
        msg: WireMsg::Subscribe(SubscribePayload {
            topics,
            since_seq: None,
        }),
    }
}

pub(crate) fn build_outbound_envelope(config: &Config, outbound: HubOutbound) -> WireEnvelope {
    WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: config.session_id.clone(),
        sender_id: config.client_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: Some(outbound.request_id),
        msg: outbound.msg,
    }
}

pub(crate) fn parse_event_at(timestamp: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}
