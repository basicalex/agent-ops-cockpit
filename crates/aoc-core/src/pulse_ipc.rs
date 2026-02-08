use serde::de::{self, DeserializeOwned, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::fmt;
use std::marker::PhantomData;
use thiserror::Error;

pub const DEFAULT_MAX_FRAME_BYTES: usize = 256 * 1024;
pub const CURRENT_PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProtocolVersion(pub u16);

impl ProtocolVersion {
    pub const CURRENT: Self = Self(CURRENT_PROTOCOL_VERSION);
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self::CURRENT
    }
}

impl Serialize for ProtocolVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for ProtocolVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ProtocolVersionVisitor;

        impl<'de> Visitor<'de> for ProtocolVersionVisitor {
            type Value = ProtocolVersion;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a protocol version as string or integer")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let version = u16::try_from(value)
                    .map_err(|_| E::custom(format!("protocol version out of range: {value}")))?;
                Ok(ProtocolVersion(version))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value < 0 {
                    return Err(E::custom(format!(
                        "protocol version cannot be negative: {value}"
                    )));
                }
                self.visit_u64(value as u64)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let cleaned = value.trim().trim_start_matches('v');
                let version = cleaned.parse::<u16>().map_err(|err| {
                    E::custom(format!("invalid protocol version '{value}': {err}"))
                })?;
                Ok(ProtocolVersion(version))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_any(ProtocolVersionVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireEnvelope {
    #[serde(default)]
    pub version: ProtocolVersion,
    pub session_id: String,
    pub sender_id: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(flatten)]
    pub msg: WireMsg,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum WireMsg {
    Hello(HelloPayload),
    Subscribe(SubscribePayload),
    Snapshot(SnapshotPayload),
    Delta(DeltaPayload),
    LayoutState(LayoutStatePayload),
    Heartbeat(HeartbeatPayload),
    Command(CommandPayload),
    CommandResult(CommandResultPayload),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HelloPayload {
    pub client_id: String,
    pub role: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub pane_id: Option<String>,
    #[serde(default)]
    pub project_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubscribePayload {
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub since_seq: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnapshotPayload {
    pub seq: u64,
    #[serde(default)]
    pub states: Vec<AgentState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeltaPayload {
    pub seq: u64,
    #[serde(default)]
    pub changes: Vec<StateChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct LayoutStatePayload {
    pub layout_seq: u64,
    pub session_id: String,
    pub emitted_at_ms: i64,
    #[serde(default)]
    pub tabs: Vec<LayoutTab>,
    #[serde(default)]
    pub panes: Vec<LayoutPane>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct LayoutTab {
    pub index: u64,
    pub name: String,
    pub focused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct LayoutPane {
    pub pane_id: String,
    pub tab_index: u64,
    pub tab_name: String,
    pub tab_focused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentState {
    pub agent_id: String,
    pub session_id: String,
    pub pane_id: String,
    pub lifecycle: String,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub last_heartbeat_ms: Option<i64>,
    #[serde(default)]
    pub last_activity_ms: Option<i64>,
    #[serde(default)]
    pub updated_at_ms: Option<i64>,
    #[serde(default)]
    pub source: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateChange {
    pub op: StateChangeOp,
    pub agent_id: String,
    #[serde(default)]
    pub state: Option<AgentState>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StateChangeOp {
    Upsert,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatPayload {
    pub agent_id: String,
    pub last_heartbeat_ms: i64,
    #[serde(default)]
    pub lifecycle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandPayload {
    pub command: String,
    #[serde(default)]
    pub target_agent_id: Option<String>,
    #[serde(default)]
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandResultPayload {
    pub command: String,
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error: Option<CommandError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FrameError {
    #[error("frame exceeds max size: {size} > {max}")]
    OversizedFrame { size: usize, max: usize },
    #[error("buffer exceeds max size without delimiter: {size} > {max}")]
    OversizedBuffer { size: usize, max: usize },
    #[error("frame encode failed: {0}")]
    Encode(String),
    #[error("frame decode failed: {0}")]
    Decode(String),
}

#[derive(Debug, Clone)]
pub struct DecodeReport<T> {
    pub frames: Vec<T>,
    pub errors: Vec<FrameError>,
}

impl<T> Default for DecodeReport<T> {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            errors: Vec::new(),
        }
    }
}

impl<T> DecodeReport<T> {
    fn push_frame(&mut self, frame: T) {
        self.frames.push(frame);
    }

    fn push_error(&mut self, error: FrameError) {
        self.errors.push(error);
    }
}

pub fn encode_frame<T: Serialize>(
    value: &T,
    max_frame_bytes: usize,
) -> Result<Vec<u8>, FrameError> {
    let mut encoded =
        serde_json::to_vec(value).map_err(|err| FrameError::Encode(err.to_string()))?;
    if encoded.len() > max_frame_bytes {
        return Err(FrameError::OversizedFrame {
            size: encoded.len(),
            max: max_frame_bytes,
        });
    }
    encoded.push(b'\n');
    Ok(encoded)
}

pub fn decode_frame<T: DeserializeOwned>(
    bytes: &[u8],
    max_frame_bytes: usize,
) -> Result<T, FrameError> {
    let mut raw = bytes;
    if raw.ends_with(b"\n") {
        raw = &raw[..raw.len() - 1];
    }
    if raw.ends_with(b"\r") {
        raw = &raw[..raw.len() - 1];
    }
    if raw.len() > max_frame_bytes {
        return Err(FrameError::OversizedFrame {
            size: raw.len(),
            max: max_frame_bytes,
        });
    }
    serde_json::from_slice(raw).map_err(|err| FrameError::Decode(err.to_string()))
}

pub struct NdjsonFrameDecoder<T> {
    max_frame_bytes: usize,
    pending: Vec<u8>,
    marker: PhantomData<T>,
}

impl<T> NdjsonFrameDecoder<T> {
    pub fn new(max_frame_bytes: usize) -> Self {
        Self {
            max_frame_bytes,
            pending: Vec::new(),
            marker: PhantomData,
        }
    }
}

impl<T> Default for NdjsonFrameDecoder<T> {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_FRAME_BYTES)
    }
}

impl<T: DeserializeOwned> NdjsonFrameDecoder<T> {
    pub fn push_chunk(&mut self, chunk: &[u8]) -> DecodeReport<T> {
        let mut report = DecodeReport::default();
        if !chunk.is_empty() {
            self.pending.extend_from_slice(chunk);
        }

        while let Some(newline_idx) = self.pending.iter().position(|byte| *byte == b'\n') {
            let mut frame = self.pending.drain(..=newline_idx).collect::<Vec<u8>>();
            if frame.ends_with(b"\n") {
                frame.pop();
            }
            if frame.ends_with(b"\r") {
                frame.pop();
            }
            if frame.is_empty() {
                continue;
            }
            self.decode_raw_frame(&frame, &mut report);
        }

        if !self.pending.is_empty() && self.pending.len() > self.max_frame_bytes {
            report.push_error(FrameError::OversizedBuffer {
                size: self.pending.len(),
                max: self.max_frame_bytes,
            });
            self.pending.clear();
        }

        report
    }

    pub fn finish(&mut self) -> DecodeReport<T> {
        if self.pending.is_empty() {
            return DecodeReport::default();
        }

        let final_frame = std::mem::take(&mut self.pending);
        let mut report = DecodeReport::default();
        self.decode_raw_frame(&final_frame, &mut report);
        report
    }

    fn decode_raw_frame(&self, frame: &[u8], report: &mut DecodeReport<T>) {
        if frame.len() > self.max_frame_bytes {
            report.push_error(FrameError::OversizedFrame {
                size: frame.len(),
                max: self.max_frame_bytes,
            });
            return;
        }
        match serde_json::from_slice(frame) {
            Ok(parsed) => report.push_frame(parsed),
            Err(err) => report.push_error(FrameError::Decode(err.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello_envelope() -> WireEnvelope {
        WireEnvelope {
            version: ProtocolVersion::CURRENT,
            session_id: "session-alpha".to_string(),
            sender_id: "wrapper-1".to_string(),
            timestamp: "2026-02-07T21:00:00Z".to_string(),
            request_id: None,
            msg: WireMsg::Hello(HelloPayload {
                client_id: "wrapper-1".to_string(),
                role: "publisher".to_string(),
                capabilities: vec!["state_update".to_string(), "heartbeat".to_string()],
                agent_id: Some("session-alpha::12".to_string()),
                pane_id: Some("12".to_string()),
                project_root: Some("/tmp/repo".to_string()),
            }),
        }
    }

    #[test]
    fn encode_decode_round_trip_for_all_variants() {
        let heartbeat = WireEnvelope {
            msg: WireMsg::Heartbeat(HeartbeatPayload {
                agent_id: "session-alpha::12".to_string(),
                last_heartbeat_ms: 1_707_335_222_222,
                lifecycle: Some("running".to_string()),
            }),
            ..hello_envelope()
        };
        let subscribe = WireEnvelope {
            sender_id: "pulse-client".to_string(),
            msg: WireMsg::Subscribe(SubscribePayload {
                topics: vec!["agent_state".to_string(), "health".to_string()],
                since_seq: Some(10),
            }),
            ..hello_envelope()
        };
        let snapshot = WireEnvelope {
            sender_id: "aoc-hub".to_string(),
            msg: WireMsg::Snapshot(SnapshotPayload {
                seq: 11,
                states: vec![AgentState {
                    agent_id: "session-alpha::12".to_string(),
                    session_id: "session-alpha".to_string(),
                    pane_id: "12".to_string(),
                    lifecycle: "running".to_string(),
                    snippet: Some("building index".to_string()),
                    last_heartbeat_ms: Some(1_707_335_222_222),
                    last_activity_ms: Some(1_707_335_222_111),
                    updated_at_ms: Some(1_707_335_222_222),
                    source: Some(serde_json::json!({"kind": "wrapper"})),
                }],
            }),
            ..hello_envelope()
        };
        let delta = WireEnvelope {
            msg: WireMsg::Delta(DeltaPayload {
                seq: 12,
                changes: vec![StateChange {
                    op: StateChangeOp::Upsert,
                    agent_id: "session-alpha::12".to_string(),
                    state: Some(AgentState {
                        agent_id: "session-alpha::12".to_string(),
                        session_id: "session-alpha".to_string(),
                        pane_id: "12".to_string(),
                        lifecycle: "needs_input".to_string(),
                        snippet: Some("awaiting prompt".to_string()),
                        last_heartbeat_ms: Some(1_707_335_222_223),
                        last_activity_ms: Some(1_707_335_222_223),
                        updated_at_ms: Some(1_707_335_222_223),
                        source: None,
                    }),
                }],
            }),
            ..hello_envelope()
        };
        let layout_state = WireEnvelope {
            sender_id: "aoc-hub".to_string(),
            msg: WireMsg::LayoutState(LayoutStatePayload {
                layout_seq: 7,
                session_id: "session-alpha".to_string(),
                emitted_at_ms: 1_707_335_222_300,
                tabs: vec![
                    LayoutTab {
                        index: 1,
                        name: "Agent".to_string(),
                        focused: false,
                    },
                    LayoutTab {
                        index: 2,
                        name: "Agent".to_string(),
                        focused: true,
                    },
                ],
                panes: vec![
                    LayoutPane {
                        pane_id: "11".to_string(),
                        tab_index: 1,
                        tab_name: "Agent".to_string(),
                        tab_focused: false,
                    },
                    LayoutPane {
                        pane_id: "12".to_string(),
                        tab_index: 2,
                        tab_name: "Agent".to_string(),
                        tab_focused: true,
                    },
                ],
            }),
            ..hello_envelope()
        };
        let command = WireEnvelope {
            sender_id: "pulse-client".to_string(),
            request_id: Some("req-7".to_string()),
            msg: WireMsg::Command(CommandPayload {
                command: "stop_agent".to_string(),
                target_agent_id: Some("session-alpha::12".to_string()),
                args: serde_json::json!({"reason": "user_request"}),
            }),
            ..hello_envelope()
        };
        let command_result = WireEnvelope {
            sender_id: "wrapper-1".to_string(),
            request_id: Some("req-7".to_string()),
            msg: WireMsg::CommandResult(CommandResultPayload {
                command: "stop_agent".to_string(),
                status: "accepted".to_string(),
                message: Some("ctrl-c sent".to_string()),
                error: None,
            }),
            ..hello_envelope()
        };

        for message in [
            hello_envelope(),
            heartbeat,
            subscribe,
            snapshot,
            delta,
            layout_state,
            command,
            command_result,
        ] {
            let frame = encode_frame(&message, DEFAULT_MAX_FRAME_BYTES).expect("encode");
            let decoded: WireEnvelope =
                decode_frame(&frame, DEFAULT_MAX_FRAME_BYTES).expect("decode");
            assert_eq!(decoded, message);
        }
    }

    #[test]
    fn decoder_recovers_after_malformed_json_line() {
        let valid_a =
            encode_frame(&hello_envelope(), DEFAULT_MAX_FRAME_BYTES).expect("encode first");
        let malformed = b"{\"not\":\"valid\"\n";
        let valid_b = encode_frame(
            &WireEnvelope {
                msg: WireMsg::Heartbeat(HeartbeatPayload {
                    agent_id: "session-alpha::12".to_string(),
                    last_heartbeat_ms: 123,
                    lifecycle: None,
                }),
                ..hello_envelope()
            },
            DEFAULT_MAX_FRAME_BYTES,
        )
        .expect("encode second");

        let mut decoder = NdjsonFrameDecoder::<WireEnvelope>::default();
        let mut chunk = Vec::new();
        chunk.extend_from_slice(&valid_a);
        chunk.extend_from_slice(malformed);
        chunk.extend_from_slice(&valid_b);

        let report = decoder.push_chunk(&chunk);
        assert_eq!(report.frames.len(), 2);
        assert_eq!(report.errors.len(), 1);
        match &report.errors[0] {
            FrameError::Decode(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn encoder_rejects_oversized_payload() {
        let huge = "x".repeat(128);
        let message = WireEnvelope {
            msg: WireMsg::Command(CommandPayload {
                command: "emit".to_string(),
                target_agent_id: None,
                args: serde_json::json!({"blob": huge}),
            }),
            ..hello_envelope()
        };

        let result = encode_frame(&message, 64);
        assert!(matches!(result, Err(FrameError::OversizedFrame { .. })));
    }

    #[test]
    fn decoder_rejects_oversized_line_and_continues() {
        let oversized = format!("{{\"blob\":\"{}\"}}\n", "x".repeat(2_000));
        let valid = encode_frame(&hello_envelope(), DEFAULT_MAX_FRAME_BYTES).expect("encode valid");

        let mut chunk = oversized.into_bytes();
        chunk.extend_from_slice(&valid);

        let mut decoder = NdjsonFrameDecoder::<WireEnvelope>::new(1_024);
        let report = decoder.push_chunk(&chunk);

        assert_eq!(report.frames.len(), 1);
        assert_eq!(report.errors.len(), 1);
        assert!(matches!(
            report.errors[0],
            FrameError::OversizedFrame { .. }
        ));
    }

    #[test]
    fn version_field_accepts_string_number_and_missing() {
        let string_version: WireEnvelope = serde_json::from_str(
            r#"{
                "version": "1",
                "type": "hello",
                "session_id": "session-alpha",
                "sender_id": "client-a",
                "timestamp": "2026-02-07T21:00:00Z",
                "payload": {"client_id":"client-a","role":"subscriber","capabilities":["snapshot"]}
            }"#,
        )
        .expect("parse string version");
        assert_eq!(string_version.version, ProtocolVersion(1));

        let numeric_version: WireEnvelope = serde_json::from_str(
            r#"{
                "version": 1,
                "type": "hello",
                "session_id": "session-alpha",
                "sender_id": "client-a",
                "timestamp": "2026-02-07T21:00:00Z",
                "payload": {"client_id":"client-a","role":"subscriber","capabilities":["snapshot"]}
            }"#,
        )
        .expect("parse numeric version");
        assert_eq!(numeric_version.version, ProtocolVersion(1));

        let missing_version: WireEnvelope = serde_json::from_str(
            r#"{
                "type": "hello",
                "session_id": "session-alpha",
                "sender_id": "client-a",
                "timestamp": "2026-02-07T21:00:00Z",
                "payload": {"client_id":"client-a","role":"subscriber","capabilities":["snapshot"]}
            }"#,
        )
        .expect("parse missing version");
        assert_eq!(missing_version.version, ProtocolVersion::CURRENT);
    }
}
