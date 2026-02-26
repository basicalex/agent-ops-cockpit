use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const MIND_SCHEMA_VERSION: u32 = 1;
pub const T0_POLICY_VERSION_V1: &str = "t0.v1";
pub const T1_PARSER_TARGET_TOKENS: u32 = 28_000;
pub const T1_PARSER_HARD_CAP_TOKENS: u32 = 32_000;
pub const CONTEXT_LAYER_PRECEDENCE: [ContextLayer; 3] = [
    ContextLayer::AocMem,
    ContextLayer::AocStm,
    ContextLayer::AocMind,
];

#[derive(Debug, Error)]
pub enum MindContractError {
    #[error("serialization failed: {0}")]
    Serialization(String),
    #[error("confidence bps out of range for {field}: {value}")]
    InvalidConfidenceBps { field: &'static str, value: u16 },
    #[error("t1 batches mixed conversations: expected {expected}, found {found}")]
    T1CrossConversation { expected: String, found: String },
    #[error("t1 batch exceeds hard cap: estimated {estimated_tokens}, hard cap {hard_cap}")]
    T1OverHardCap {
        estimated_tokens: u32,
        hard_cap: u32,
    },
    #[error("observation list cannot be empty")]
    EmptyObservations,
    #[error("t2 observations mixed tags: expected {expected}, found {found}")]
    T2TagMismatch { expected: String, found: String },
    #[error("context pack max lines must be > 0")]
    InvalidContextBudget,
    #[error("invalid temporal range: end_ts must be >= start_ts")]
    InvalidTemporalRange,
    #[error("semantic output invalid: {reason}")]
    InvalidSemanticOutput { reason: String },
    #[error("semantic adapter error ({kind}): {message}")]
    SemanticAdapter {
        kind: SemanticFailureKind,
        message: String,
    },
    #[error("invalid lineage metadata: {reason}")]
    InvalidLineageMetadata { reason: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ConversationRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionStatus {
    Success,
    Failure,
}

impl From<bool> for ToolExecutionStatus {
    fn from(success: bool) -> Self {
        if success {
            Self::Success
        } else {
            Self::Failure
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawEvent {
    pub event_id: String,
    pub conversation_id: String,
    pub agent_id: String,
    pub ts: DateTime<Utc>,
    pub body: RawEventBody,
    #[serde(default)]
    pub attrs: BTreeMap<String, Value>,
}

pub const LINEAGE_ATTRS_KEY: &str = "mind_lineage";
pub const LINEAGE_SESSION_ID_KEY: &str = "session_id";
pub const LINEAGE_PARENT_CONVERSATION_ID_KEY: &str = "parent_conversation_id";
pub const LINEAGE_ROOT_CONVERSATION_ID_KEY: &str = "root_conversation_id";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationLineageMetadata {
    pub session_id: String,
    pub parent_conversation_id: Option<String>,
    pub root_conversation_id: String,
}

pub fn canonical_lineage_attrs(metadata: &ConversationLineageMetadata) -> BTreeMap<String, Value> {
    let mut lineage = Map::new();
    lineage.insert(
        LINEAGE_SESSION_ID_KEY.to_string(),
        Value::String(metadata.session_id.clone()),
    );
    if let Some(parent) = metadata.parent_conversation_id.as_ref() {
        lineage.insert(
            LINEAGE_PARENT_CONVERSATION_ID_KEY.to_string(),
            Value::String(parent.clone()),
        );
    }
    lineage.insert(
        LINEAGE_ROOT_CONVERSATION_ID_KEY.to_string(),
        Value::String(metadata.root_conversation_id.clone()),
    );

    let mut attrs = BTreeMap::new();
    attrs.insert(LINEAGE_ATTRS_KEY.to_string(), Value::Object(lineage));
    attrs
}

pub fn parse_conversation_lineage_metadata(
    attrs: &BTreeMap<String, Value>,
    conversation_id: &str,
    agent_id: &str,
) -> Result<Option<ConversationLineageMetadata>, MindContractError> {
    let nested = attrs.get(LINEAGE_ATTRS_KEY).and_then(Value::as_object);

    let explicit_session = attr_string(
        attrs,
        nested,
        &[
            LINEAGE_SESSION_ID_KEY,
            "sessionId",
            "zellij_session_id",
            "zellijSessionId",
        ],
    );
    let parent_conversation_id = attr_string(
        attrs,
        nested,
        &[
            LINEAGE_PARENT_CONVERSATION_ID_KEY,
            "parentConversationId",
            "parent_id",
            "branch_parent_conversation_id",
            "branchParentConversationId",
        ],
    );
    let root_conversation_id = attr_string(
        attrs,
        nested,
        &[
            LINEAGE_ROOT_CONVERSATION_ID_KEY,
            "rootConversationId",
            "conversation_root_id",
            "conversationRootId",
        ],
    );

    let lineage_declared = nested.is_some()
        || has_any_key(
            attrs,
            &[
                LINEAGE_SESSION_ID_KEY,
                "sessionId",
                "zellij_session_id",
                "zellijSessionId",
                LINEAGE_PARENT_CONVERSATION_ID_KEY,
                "parentConversationId",
                "parent_id",
                "branch_parent_conversation_id",
                "branchParentConversationId",
                LINEAGE_ROOT_CONVERSATION_ID_KEY,
                "rootConversationId",
                "conversation_root_id",
                "conversationRootId",
            ],
        );

    if (parent_conversation_id.is_some() || root_conversation_id.is_some())
        && explicit_session.is_none()
    {
        return Err(MindContractError::InvalidLineageMetadata {
            reason: "branch lineage requires explicit session_id".to_string(),
        });
    }

    let session_id = explicit_session.or_else(|| {
        agent_id
            .split_once("::")
            .map(|(session, _)| session.trim().to_string())
            .filter(|session| !session.is_empty())
    });

    let Some(session_id) = session_id else {
        if lineage_declared {
            return Err(MindContractError::InvalidLineageMetadata {
                reason: "lineage metadata present but session_id is missing".to_string(),
            });
        }
        return Ok(None);
    };

    if let Some(parent) = parent_conversation_id.as_ref() {
        if parent == conversation_id {
            return Err(MindContractError::InvalidLineageMetadata {
                reason: "parent_conversation_id must not equal conversation_id".to_string(),
            });
        }
    }

    let root_conversation_id = match (parent_conversation_id.as_ref(), root_conversation_id) {
        (Some(_), Some(root)) => {
            if root == conversation_id {
                return Err(MindContractError::InvalidLineageMetadata {
                    reason:
                        "root_conversation_id must refer to the branch root for child conversations"
                            .to_string(),
                });
            }
            root
        }
        (Some(_), None) => {
            return Err(MindContractError::InvalidLineageMetadata {
                reason: "parent_conversation_id requires root_conversation_id".to_string(),
            });
        }
        (None, Some(root)) => {
            if root != conversation_id {
                return Err(MindContractError::InvalidLineageMetadata {
                    reason:
                        "root_conversation_id without parent_conversation_id must equal conversation_id"
                            .to_string(),
                });
            }
            root
        }
        (None, None) => conversation_id.to_string(),
    };

    Ok(Some(ConversationLineageMetadata {
        session_id,
        parent_conversation_id,
        root_conversation_id,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawEventBody {
    Message(MessageEvent),
    ToolResult(ToolResultEvent),
    TaskSignal(TaskSignalEvent),
    Other { payload: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageEvent {
    pub role: ConversationRole,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResultEvent {
    pub tool_name: String,
    pub status: ToolExecutionStatus,
    pub latency_ms: Option<u64>,
    pub exit_code: Option<i32>,
    pub output: Option<String>,
    #[serde(default)]
    pub redacted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskSignalEvent {
    pub active_tag: Option<String>,
    #[serde(default)]
    pub task_ids: Vec<String>,
    pub lifecycle: Option<String>,
    #[serde(default)]
    pub signal_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct T0CompactionPolicy {
    pub policy_version: String,
    pub keep_roles: BTreeSet<ConversationRole>,
    pub tool_snippet_allowlist: BTreeMap<String, usize>,
    pub redaction_marker: String,
}

impl Default for T0CompactionPolicy {
    fn default() -> Self {
        let mut keep_roles = BTreeSet::new();
        keep_roles.insert(ConversationRole::System);
        keep_roles.insert(ConversationRole::User);
        keep_roles.insert(ConversationRole::Assistant);

        Self {
            policy_version: T0_POLICY_VERSION_V1.to_string(),
            keep_roles,
            tool_snippet_allowlist: BTreeMap::new(),
            redaction_marker: "[redacted]".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolMetadataLine {
    pub tool_name: String,
    pub status: ToolExecutionStatus,
    pub latency_ms: Option<u64>,
    pub exit_code: Option<i32>,
    pub output_bytes: usize,
    pub redacted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct T0CompactEvent {
    pub schema_version: u32,
    pub compact_id: String,
    pub compact_hash: String,
    pub conversation_id: String,
    pub ts: DateTime<Utc>,
    pub role: Option<ConversationRole>,
    pub text: Option<String>,
    pub tool_meta: Option<ToolMetadataLine>,
    pub snippet: Option<String>,
    pub source_event_ids: Vec<String>,
    pub policy_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct T0CompactEventCore {
    pub conversation_id: String,
    pub ts: DateTime<Utc>,
    pub role: Option<ConversationRole>,
    pub text: Option<String>,
    pub tool_meta: Option<ToolMetadataLine>,
    pub snippet: Option<String>,
    pub source_event_ids: Vec<String>,
    pub policy_version: String,
}

pub fn compact_raw_event_to_t0(
    raw: &RawEvent,
    policy: &T0CompactionPolicy,
) -> Result<Option<T0CompactEvent>, MindContractError> {
    let core = match &raw.body {
        RawEventBody::Message(message) => {
            if !policy.keep_roles.contains(&message.role) {
                return Ok(None);
            }
            T0CompactEventCore {
                conversation_id: raw.conversation_id.clone(),
                ts: raw.ts,
                role: Some(message.role),
                text: Some(message.text.clone()),
                tool_meta: None,
                snippet: None,
                source_event_ids: vec![raw.event_id.clone()],
                policy_version: policy.policy_version.clone(),
            }
        }
        RawEventBody::ToolResult(tool) => {
            let output_bytes = tool.output.as_deref().map_or(0, str::len);
            let snippet =
                policy
                    .tool_snippet_allowlist
                    .get(&tool.tool_name)
                    .and_then(|max_chars| {
                        tool.output.as_deref().map(|output| {
                            bounded_snippet(output, *max_chars, tool.redacted, policy)
                        })
                    });

            T0CompactEventCore {
                conversation_id: raw.conversation_id.clone(),
                ts: raw.ts,
                role: None,
                text: None,
                tool_meta: Some(ToolMetadataLine {
                    tool_name: tool.tool_name.clone(),
                    status: tool.status,
                    latency_ms: tool.latency_ms,
                    exit_code: tool.exit_code,
                    output_bytes,
                    redacted: tool.redacted,
                }),
                snippet,
                source_event_ids: vec![raw.event_id.clone()],
                policy_version: policy.policy_version.clone(),
            }
        }
        RawEventBody::TaskSignal(_) | RawEventBody::Other { .. } => return Ok(None),
    };

    let compact_hash = sha256_hex(canonical_json(&core)?.as_bytes());
    let compact_id = format!("t0:{}", &compact_hash[..16]);

    Ok(Some(T0CompactEvent {
        schema_version: MIND_SCHEMA_VERSION,
        compact_id,
        compact_hash,
        conversation_id: core.conversation_id,
        ts: core.ts,
        role: core.role,
        text: core.text,
        tool_meta: core.tool_meta,
        snippet: core.snippet,
        source_event_ids: core.source_event_ids,
        policy_version: core.policy_version,
    }))
}

fn attr_string(
    attrs: &BTreeMap<String, Value>,
    nested: Option<&Map<String, Value>>,
    keys: &[&str],
) -> Option<String> {
    for key in keys {
        if let Some(value) = attrs
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }

    let Some(nested) = nested else {
        return None;
    };

    for key in keys {
        if let Some(value) = nested
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }

    None
}

fn has_any_key(attrs: &BTreeMap<String, Value>, keys: &[&str]) -> bool {
    keys.iter().any(|key| attrs.contains_key(*key))
}

fn bounded_snippet(
    output: &str,
    max_chars: usize,
    redacted: bool,
    policy: &T0CompactionPolicy,
) -> String {
    if redacted {
        return policy.redaction_marker.clone();
    }

    if max_chars == 0 {
        return String::new();
    }

    output.chars().take(max_chars).collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

pub fn canonical_json<T: Serialize>(value: &T) -> Result<String, MindContractError> {
    let json = serde_json::to_value(value)
        .map_err(|err| MindContractError::Serialization(err.to_string()))?;
    let canonical = canonicalize_value(json);
    serde_json::to_string(&canonical)
        .map_err(|err| MindContractError::Serialization(err.to_string()))
}

pub fn canonical_payload_hash<T: Serialize>(value: &T) -> Result<String, MindContractError> {
    let rendered = canonical_json(value)?;
    Ok(sha256_hex(rendered.as_bytes()))
}

fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries: Vec<(String, Value)> = object.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));

            let mut sorted = Map::new();
            for (key, value) in entries {
                sorted.insert(key, canonicalize_value(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_value).collect()),
        scalar => scalar,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct T1Batch {
    pub conversation_id: String,
    pub compact_event_ids: Vec<String>,
    pub estimated_tokens: u32,
}

impl T1Batch {
    pub fn validate_hard_cap(&self, hard_cap: u32) -> Result<(), MindContractError> {
        if self.estimated_tokens > hard_cap {
            return Err(MindContractError::T1OverHardCap {
                estimated_tokens: self.estimated_tokens,
                hard_cap,
            });
        }
        Ok(())
    }
}

pub fn validate_t1_scope(batches: &[T1Batch]) -> Result<(), MindContractError> {
    if let Some(first) = batches.first() {
        first.validate_hard_cap(T1_PARSER_HARD_CAP_TOKENS)?;
        for batch in &batches[1..] {
            batch.validate_hard_cap(T1_PARSER_HARD_CAP_TOKENS)?;
            if batch.conversation_id != first.conversation_id {
                return Err(MindContractError::T1CrossConversation {
                    expected: first.conversation_id.clone(),
                    found: batch.conversation_id.clone(),
                });
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservationRef {
    pub artifact_id: String,
    pub conversation_id: String,
    pub active_tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct T2WorkstreamBatch {
    pub active_tag: String,
    pub observation_ids: Vec<String>,
    pub conversation_ids: Vec<String>,
}

pub fn build_t2_workstream_batch(
    active_tag: &str,
    observations: &[ObservationRef],
) -> Result<T2WorkstreamBatch, MindContractError> {
    if observations.is_empty() {
        return Err(MindContractError::EmptyObservations);
    }

    for observation in observations {
        if observation.active_tag != active_tag {
            return Err(MindContractError::T2TagMismatch {
                expected: active_tag.to_string(),
                found: observation.active_tag.clone(),
            });
        }
    }

    let mut observation_ids = observations
        .iter()
        .map(|observation| observation.artifact_id.clone())
        .collect::<Vec<_>>();
    observation_ids.sort();
    observation_ids.dedup();

    let mut conversation_ids = observations
        .iter()
        .map(|observation| observation.conversation_id.clone())
        .collect::<Vec<_>>();
    conversation_ids.sort();
    conversation_ids.dedup();

    Ok(T2WorkstreamBatch {
        active_tag: active_tag.to_string(),
        observation_ids,
        conversation_ids,
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRuntimeMode {
    DeterministicOnly,
    SemanticWithFallback,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticRuntime {
    Deterministic,
    PiSemantic,
    ExternalSemantic,
}

impl SemanticRuntime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::PiSemantic => "pi-semantic",
            Self::ExternalSemantic => "external-semantic",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticStage {
    T1Observer,
    T2Reflector,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticFailureKind {
    Timeout,
    InvalidOutput,
    BudgetExceeded,
    ProviderError,
    LockConflict,
}

impl SemanticFailureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::InvalidOutput => "invalid_output",
            Self::BudgetExceeded => "budget_exceeded",
            Self::ProviderError => "provider_error",
            Self::LockConflict => "lock_conflict",
        }
    }
}

impl std::fmt::Display for SemanticFailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticAdapterError {
    pub kind: SemanticFailureKind,
    pub message: String,
}

impl SemanticAdapterError {
    pub fn new(kind: SemanticFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl From<SemanticAdapterError> for MindContractError {
    fn from(value: SemanticAdapterError) -> Self {
        MindContractError::SemanticAdapter {
            kind: value.kind,
            message: value.message,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticModelProfile {
    pub provider_name: String,
    pub model_id: String,
    pub prompt_version: String,
    pub max_input_tokens: u32,
    pub max_output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticGuardrails {
    pub timeout_ms: u64,
    pub max_retries: u8,
    pub max_budget_tokens: u32,
    pub max_budget_cost_micros: u64,
    pub queue_debounce_ms: u64,
    pub reflector_lease_ttl_ms: u64,
}

impl Default for SemanticGuardrails {
    fn default() -> Self {
        Self {
            timeout_ms: 8_000,
            max_retries: 1,
            max_budget_tokens: 4_096,
            max_budget_cost_micros: 0,
            queue_debounce_ms: 250,
            reflector_lease_ttl_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObserverInput {
    pub conversation_id: String,
    pub active_tag: String,
    pub compact_event_ids: Vec<String>,
    pub compact_payload_lines: Vec<String>,
    pub estimated_tokens: u32,
    pub prompt_version: String,
    pub input_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ObserverInputCore {
    pub conversation_id: String,
    pub active_tag: String,
    pub compact_event_ids: Vec<String>,
    pub compact_payload_lines: Vec<String>,
    pub estimated_tokens: u32,
    pub prompt_version: String,
}

impl ObserverInput {
    pub fn new(
        conversation_id: impl Into<String>,
        active_tag: impl Into<String>,
        mut compact_event_ids: Vec<String>,
        compact_payload_lines: Vec<String>,
        estimated_tokens: u32,
        prompt_version: impl Into<String>,
    ) -> Result<Self, MindContractError> {
        compact_event_ids.sort();
        compact_event_ids.dedup();

        let core = ObserverInputCore {
            conversation_id: conversation_id.into(),
            active_tag: active_tag.into(),
            compact_event_ids,
            compact_payload_lines,
            estimated_tokens,
            prompt_version: prompt_version.into(),
        };
        let input_hash = canonical_payload_hash(&core)?;

        Ok(Self {
            conversation_id: core.conversation_id,
            active_tag: core.active_tag,
            compact_event_ids: core.compact_event_ids,
            compact_payload_lines: core.compact_payload_lines,
            estimated_tokens: core.estimated_tokens,
            prompt_version: core.prompt_version,
            input_hash,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReflectorInput {
    pub active_tag: String,
    pub observation_ids: Vec<String>,
    pub conversation_ids: Vec<String>,
    pub observation_payload_lines: Vec<String>,
    pub estimated_tokens: u32,
    pub prompt_version: String,
    pub input_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ReflectorInputCore {
    pub active_tag: String,
    pub observation_ids: Vec<String>,
    pub conversation_ids: Vec<String>,
    pub observation_payload_lines: Vec<String>,
    pub estimated_tokens: u32,
    pub prompt_version: String,
}

impl ReflectorInput {
    pub fn new(
        active_tag: impl Into<String>,
        mut observation_ids: Vec<String>,
        mut conversation_ids: Vec<String>,
        observation_payload_lines: Vec<String>,
        estimated_tokens: u32,
        prompt_version: impl Into<String>,
    ) -> Result<Self, MindContractError> {
        observation_ids.sort();
        observation_ids.dedup();
        conversation_ids.sort();
        conversation_ids.dedup();

        let core = ReflectorInputCore {
            active_tag: active_tag.into(),
            observation_ids,
            conversation_ids,
            observation_payload_lines,
            estimated_tokens,
            prompt_version: prompt_version.into(),
        };
        let input_hash = canonical_payload_hash(&core)?;

        Ok(Self {
            active_tag: core.active_tag,
            observation_ids: core.observation_ids,
            conversation_ids: core.conversation_ids,
            observation_payload_lines: core.observation_payload_lines,
            estimated_tokens: core.estimated_tokens,
            prompt_version: core.prompt_version,
            input_hash,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObserverOutput {
    pub summary: String,
    #[serde(default)]
    pub key_points: Vec<String>,
    #[serde(default)]
    pub citations: Vec<String>,
}

impl ObserverOutput {
    pub fn validate(&self) -> Result<(), MindContractError> {
        if self.summary.trim().is_empty() {
            return Err(MindContractError::InvalidSemanticOutput {
                reason: "observer summary must be non-empty".to_string(),
            });
        }
        if self.key_points.iter().any(|point| point.trim().is_empty()) {
            return Err(MindContractError::InvalidSemanticOutput {
                reason: "observer key_points cannot contain empty lines".to_string(),
            });
        }
        Ok(())
    }

    pub fn parse_json(raw: &str) -> Result<Self, MindContractError> {
        let output = serde_json::from_str::<Self>(raw).map_err(|err| {
            MindContractError::InvalidSemanticOutput {
                reason: format!("observer output parse error: {err}"),
            }
        })?;
        output.validate()?;
        Ok(output)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReflectorOutput {
    pub reflection: String,
    #[serde(default)]
    pub action_items: Vec<String>,
    #[serde(default)]
    pub citations: Vec<String>,
}

impl ReflectorOutput {
    pub fn validate(&self) -> Result<(), MindContractError> {
        if self.reflection.trim().is_empty() {
            return Err(MindContractError::InvalidSemanticOutput {
                reason: "reflector reflection must be non-empty".to_string(),
            });
        }
        if self.action_items.iter().any(|item| item.trim().is_empty()) {
            return Err(MindContractError::InvalidSemanticOutput {
                reason: "reflector action_items cannot contain empty lines".to_string(),
            });
        }
        Ok(())
    }

    pub fn parse_json(raw: &str) -> Result<Self, MindContractError> {
        let output = serde_json::from_str::<Self>(raw).map_err(|err| {
            MindContractError::InvalidSemanticOutput {
                reason: format!("reflector output parse error: {err}"),
            }
        })?;
        output.validate()?;
        Ok(output)
    }
}

pub trait ObserverAdapter {
    fn observe_t1(
        &self,
        input: &ObserverInput,
        profile: &SemanticModelProfile,
        guardrails: &SemanticGuardrails,
    ) -> Result<ObserverOutput, SemanticAdapterError>;
}

pub trait ReflectorAdapter {
    fn reflect_t2(
        &self,
        input: &ReflectorInput,
        profile: &SemanticModelProfile,
        guardrails: &SemanticGuardrails,
    ) -> Result<ReflectorOutput, SemanticAdapterError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticProvenance {
    pub artifact_id: String,
    pub stage: SemanticStage,
    pub runtime: SemanticRuntime,
    pub provider_name: Option<String>,
    pub model_id: Option<String>,
    pub prompt_version: String,
    pub input_hash: String,
    pub output_hash: Option<String>,
    pub latency_ms: Option<u64>,
    pub attempt_count: u16,
    pub fallback_used: bool,
    pub fallback_reason: Option<String>,
    pub failure_kind: Option<SemanticFailureKind>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactTaskRelation {
    Active,
    WorkedOn,
    Mentioned,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactTaskLink {
    pub artifact_id: String,
    pub task_id: String,
    pub relation: ArtifactTaskRelation,
    pub confidence_bps: u16,
    pub evidence_event_ids: Vec<String>,
    pub source: String,
    pub start_ts: DateTime<Utc>,
    pub end_ts: Option<DateTime<Utc>>,
}

impl ArtifactTaskLink {
    pub fn new(
        artifact_id: String,
        task_id: String,
        relation: ArtifactTaskRelation,
        confidence_bps: u16,
        mut evidence_event_ids: Vec<String>,
        source: String,
        start_ts: DateTime<Utc>,
        end_ts: Option<DateTime<Utc>>,
    ) -> Result<Self, MindContractError> {
        validate_confidence_bps("confidence_bps", confidence_bps)?;

        if let Some(end_ts) = end_ts {
            if end_ts < start_ts {
                return Err(MindContractError::InvalidTemporalRange);
            }
        }

        evidence_event_ids.sort();
        evidence_event_ids.dedup();

        Ok(Self {
            artifact_id,
            task_id,
            relation,
            confidence_bps,
            evidence_event_ids,
            source,
            start_ts,
            end_ts,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteOrigin {
    Taskmaster,
    Heuristic,
    ManualOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SegmentCandidate {
    pub segment_id: String,
    pub confidence_bps: u16,
}

impl SegmentCandidate {
    pub fn new(segment_id: String, confidence_bps: u16) -> Result<Self, MindContractError> {
        validate_confidence_bps("segment_confidence_bps", confidence_bps)?;
        Ok(Self {
            segment_id,
            confidence_bps,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SegmentRoute {
    pub artifact_id: String,
    pub primary: SegmentCandidate,
    #[serde(default)]
    pub secondary: Vec<SegmentCandidate>,
    pub routed_by: RouteOrigin,
    pub reason: String,
    pub overridden_by: Option<String>,
}

impl SegmentRoute {
    pub fn validate(&self) -> Result<(), MindContractError> {
        validate_confidence_bps("primary_confidence_bps", self.primary.confidence_bps)?;
        for candidate in &self.secondary {
            validate_confidence_bps("secondary_confidence_bps", candidate.confidence_bps)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderGuardrails {
    pub background_only: bool,
    pub timeout_ms: u64,
    pub max_retries: u8,
    pub max_budget_tokens: u32,
}

impl Default for ProviderGuardrails {
    fn default() -> Self {
        Self {
            background_only: true,
            timeout_ms: 5_000,
            max_retries: 1,
            max_budget_tokens: 4_096,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderRequest {
    pub active_tag: String,
    pub observation_ids: Vec<String>,
    pub payload_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderEnhancement {
    pub summary: String,
    pub confidence_bps: u16,
    #[serde(default)]
    pub citations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProviderOutcome {
    BaselineOnly,
    Enhanced {
        provider: String,
        latency_ms: u64,
        enhancement: ProviderEnhancement,
    },
    FailOpenFallback {
        provider: String,
        reason: String,
    },
}

pub trait MindProviderAdapter {
    fn provider_name(&self) -> &'static str;
    fn enhance_t2(
        &self,
        request: &ProviderRequest,
        guardrails: &ProviderGuardrails,
    ) -> Result<ProviderOutcome, MindContractError>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextLayer {
    AocMem,
    AocStm,
    AocMind,
}

impl ContextLayer {
    pub fn precedence(self) -> usize {
        match self {
            Self::AocMem => 0,
            Self::AocStm => 1,
            Self::AocMind => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextPackInput {
    pub layer: ContextLayer,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextPack {
    pub lines: Vec<String>,
    pub truncated: bool,
}

pub fn compose_context_pack(
    inputs: &[ContextPackInput],
    max_lines: usize,
) -> Result<ContextPack, MindContractError> {
    if max_lines == 0 {
        return Err(MindContractError::InvalidContextBudget);
    }

    let mut indexed = inputs
        .iter()
        .cloned()
        .enumerate()
        .collect::<Vec<(usize, ContextPackInput)>>();
    indexed.sort_by_key(|(index, input)| (input.layer.precedence(), *index));

    let mut lines = Vec::new();
    let mut truncated = false;

    for (_, input) in indexed {
        for line in input.lines {
            if lines.len() == max_lines {
                truncated = true;
                break;
            }
            lines.push(line);
        }
        if truncated {
            break;
        }
    }

    Ok(ContextPack { lines, truncated })
}

fn validate_confidence_bps(
    field: &'static str,
    confidence_bps: u16,
) -> Result<(), MindContractError> {
    if confidence_bps > 10_000 {
        return Err(MindContractError::InvalidConfidenceBps {
            field,
            value: confidence_bps,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 23, 12, 0, 0)
            .single()
            .expect("valid timestamp")
    }

    fn raw_message(
        event_id: &str,
        conversation_id: &str,
        role: ConversationRole,
        text: &str,
    ) -> RawEvent {
        RawEvent {
            event_id: event_id.to_string(),
            conversation_id: conversation_id.to_string(),
            agent_id: "agent-1".to_string(),
            ts: ts(),
            body: RawEventBody::Message(MessageEvent {
                role,
                text: text.to_string(),
            }),
            attrs: BTreeMap::new(),
        }
    }

    fn raw_tool(event_id: &str, conversation_id: &str, tool_name: &str, output: &str) -> RawEvent {
        RawEvent {
            event_id: event_id.to_string(),
            conversation_id: conversation_id.to_string(),
            agent_id: "agent-1".to_string(),
            ts: ts(),
            body: RawEventBody::ToolResult(ToolResultEvent {
                tool_name: tool_name.to_string(),
                status: ToolExecutionStatus::Success,
                latency_ms: Some(51),
                exit_code: Some(0),
                output: Some(output.to_string()),
                redacted: false,
            }),
            attrs: BTreeMap::new(),
        }
    }

    #[test]
    fn lineage_parser_accepts_canonical_nested_metadata() {
        let attrs = BTreeMap::from([(
            LINEAGE_ATTRS_KEY.to_string(),
            serde_json::json!({
                "session_id": "session-a",
                "parent_conversation_id": "conv-root",
                "root_conversation_id": "conv-root"
            }),
        )]);

        let lineage = parse_conversation_lineage_metadata(&attrs, "conv-branch", "ignored::12")
            .expect("lineage should parse")
            .expect("lineage should exist");

        assert_eq!(lineage.session_id, "session-a");
        assert_eq!(lineage.parent_conversation_id.as_deref(), Some("conv-root"));
        assert_eq!(lineage.root_conversation_id, "conv-root");
    }

    #[test]
    fn lineage_parser_rejects_partial_branch_metadata() {
        let attrs = BTreeMap::from([(
            LINEAGE_PARENT_CONVERSATION_ID_KEY.to_string(),
            Value::String("conv-root".to_string()),
        )]);

        let err = parse_conversation_lineage_metadata(&attrs, "conv-branch", "session-a::12")
            .expect_err("partial branch metadata must fail");
        assert!(matches!(
            err,
            MindContractError::InvalidLineageMetadata { .. }
        ));
    }

    #[test]
    fn lineage_parser_falls_back_to_agent_session_for_legacy_events() {
        let lineage =
            parse_conversation_lineage_metadata(&BTreeMap::new(), "conv-legacy", "session-z::42")
                .expect("lineage parse")
                .expect("legacy session should still be inferred");
        assert_eq!(lineage.session_id, "session-z");
        assert_eq!(lineage.parent_conversation_id, None);
        assert_eq!(lineage.root_conversation_id, "conv-legacy");
    }

    #[test]
    fn t0_compaction_keeps_conversation_message_roles() {
        let event = raw_message(
            "e1",
            "c1",
            ConversationRole::User,
            "ship task 101 contracts",
        );
        let compact = compact_raw_event_to_t0(&event, &T0CompactionPolicy::default())
            .expect("compaction should succeed")
            .expect("message should be preserved");

        assert_eq!(compact.schema_version, MIND_SCHEMA_VERSION);
        assert_eq!(compact.role, Some(ConversationRole::User));
        assert_eq!(compact.text.as_deref(), Some("ship task 101 contracts"));
        assert!(compact.tool_meta.is_none());
        assert!(compact.snippet.is_none());
    }

    #[test]
    fn t0_compaction_strips_tool_output_by_default_but_keeps_metadata() {
        let event = raw_tool("e2", "c1", "bash", "very long output that should not pass");
        let compact = compact_raw_event_to_t0(&event, &T0CompactionPolicy::default())
            .expect("compaction should succeed")
            .expect("tool metadata event should be preserved");

        assert!(compact.text.is_none());
        assert!(compact.snippet.is_none());
        let tool_meta = compact.tool_meta.expect("tool metadata should exist");
        assert_eq!(tool_meta.tool_name, "bash");
        assert_eq!(tool_meta.status, ToolExecutionStatus::Success);
        assert_eq!(
            tool_meta.output_bytes,
            "very long output that should not pass".len()
        );
    }

    #[test]
    fn t0_compaction_allowlisted_snippets_are_deterministic() {
        let event = raw_tool("e3", "c1", "bash", "1234567890abcdef");
        let mut policy = T0CompactionPolicy::default();
        policy.tool_snippet_allowlist.insert("bash".to_string(), 10);

        let a = compact_raw_event_to_t0(&event, &policy)
            .expect("compaction should succeed")
            .expect("tool event should compact");
        let b = compact_raw_event_to_t0(&event, &policy)
            .expect("compaction should succeed")
            .expect("tool event should compact");

        assert_eq!(a.snippet.as_deref(), Some("1234567890"));
        assert_eq!(a.compact_hash, b.compact_hash);
        assert_eq!(
            canonical_json(&a).expect("serialize"),
            canonical_json(&b).expect("serialize")
        );
    }

    #[test]
    fn canonical_json_sorts_nested_object_keys() {
        let mut object = Map::new();
        object.insert("z".to_string(), Value::from(2));
        object.insert("a".to_string(), Value::from(1));

        let rendered = canonical_json(&Value::Object(object)).expect("canonical json");
        assert_eq!(rendered, "{\"a\":1,\"z\":2}");
    }

    #[test]
    fn t1_scope_rejects_cross_conversation_mixing() {
        let err = validate_t1_scope(&[
            T1Batch {
                conversation_id: "c1".to_string(),
                compact_event_ids: vec!["t0:1".to_string()],
                estimated_tokens: 100,
            },
            T1Batch {
                conversation_id: "c2".to_string(),
                compact_event_ids: vec!["t0:2".to_string()],
                estimated_tokens: 100,
            },
        ])
        .expect_err("mixed conversation ids must fail");

        assert!(matches!(err, MindContractError::T1CrossConversation { .. }));
    }

    #[test]
    fn t2_batch_allows_cross_conversation_within_single_tag() {
        let workstream = build_t2_workstream_batch(
            "mind",
            &[
                ObservationRef {
                    artifact_id: "obs-1".to_string(),
                    conversation_id: "c1".to_string(),
                    active_tag: "mind".to_string(),
                },
                ObservationRef {
                    artifact_id: "obs-2".to_string(),
                    conversation_id: "c2".to_string(),
                    active_tag: "mind".to_string(),
                },
            ],
        )
        .expect("same-tag cross-conversation should be valid");

        assert_eq!(workstream.active_tag, "mind");
        assert_eq!(
            workstream.observation_ids,
            vec!["obs-1".to_string(), "obs-2".to_string()]
        );
        assert_eq!(
            workstream.conversation_ids,
            vec!["c1".to_string(), "c2".to_string()]
        );
    }

    #[test]
    fn t2_batch_rejects_cross_tag_mixing() {
        let err = build_t2_workstream_batch(
            "mind",
            &[
                ObservationRef {
                    artifact_id: "obs-1".to_string(),
                    conversation_id: "c1".to_string(),
                    active_tag: "mind".to_string(),
                },
                ObservationRef {
                    artifact_id: "obs-2".to_string(),
                    conversation_id: "c2".to_string(),
                    active_tag: "omo".to_string(),
                },
            ],
        )
        .expect_err("cross-tag synthesis should fail by default");

        assert!(matches!(err, MindContractError::T2TagMismatch { .. }));
    }

    #[test]
    fn artifact_link_confidence_and_evidence_are_normalized() {
        let link = ArtifactTaskLink::new(
            "art-1".to_string(),
            "101".to_string(),
            ArtifactTaskRelation::WorkedOn,
            9_350,
            vec!["e3".to_string(), "e1".to_string(), "e1".to_string()],
            "taskmaster+conversation".to_string(),
            ts(),
            Some(ts()),
        )
        .expect("valid link");

        assert_eq!(
            link.evidence_event_ids,
            vec!["e1".to_string(), "e3".to_string()]
        );

        let err = ArtifactTaskLink::new(
            "art-1".to_string(),
            "101".to_string(),
            ArtifactTaskRelation::WorkedOn,
            10_001,
            vec!["e1".to_string()],
            "taskmaster+conversation".to_string(),
            ts(),
            None,
        )
        .expect_err("out-of-range confidence should fail");

        assert!(matches!(
            err,
            MindContractError::InvalidConfidenceBps {
                field: "confidence_bps",
                value: 10_001
            }
        ));
    }

    #[test]
    fn context_pack_precedence_and_budget_are_stable() {
        let inputs = vec![
            ContextPackInput {
                layer: ContextLayer::AocMind,
                lines: vec!["mind-1".to_string(), "mind-2".to_string()],
            },
            ContextPackInput {
                layer: ContextLayer::AocStm,
                lines: vec!["stm-1".to_string()],
            },
            ContextPackInput {
                layer: ContextLayer::AocMem,
                lines: vec!["mem-1".to_string(), "mem-2".to_string()],
            },
        ];

        let first = compose_context_pack(&inputs, 4).expect("compose should work");
        let second = compose_context_pack(&inputs, 4).expect("compose should work");

        assert_eq!(first, second);
        assert_eq!(
            first.lines,
            vec![
                "mem-1".to_string(),
                "mem-2".to_string(),
                "stm-1".to_string(),
                "mind-1".to_string(),
            ]
        );
        assert!(first.truncated);
    }

    #[test]
    fn observer_input_hash_is_stable_for_equivalent_payloads() {
        let a = ObserverInput::new(
            "conv-1",
            "mind",
            vec!["t0:b".to_string(), "t0:a".to_string()],
            vec!["user: build contracts".to_string()],
            42,
            "prompt.observer.v1",
        )
        .expect("observer input");

        let b = ObserverInput::new(
            "conv-1",
            "mind",
            vec!["t0:a".to_string(), "t0:b".to_string(), "t0:a".to_string()],
            vec!["user: build contracts".to_string()],
            42,
            "prompt.observer.v1",
        )
        .expect("observer input");

        assert_eq!(a.input_hash, b.input_hash);
    }

    #[test]
    fn observer_output_json_parse_rejects_empty_summary() {
        let err = ObserverOutput::parse_json(r#"{"summary":"   ","key_points":["a"]}"#)
            .expect_err("empty summary must fail");

        assert!(matches!(
            err,
            MindContractError::InvalidSemanticOutput { .. }
        ));
    }

    #[test]
    fn reflector_output_json_parse_accepts_valid_payload() {
        let output = ReflectorOutput::parse_json(
            r#"{"reflection":"Ship singleton reflector","action_items":["add lease tests"]}"#,
        )
        .expect("valid reflector output");

        assert_eq!(output.reflection, "Ship singleton reflector");
        assert_eq!(output.action_items, vec!["add lease tests".to_string()]);
    }

    #[test]
    fn canonical_payload_hash_matches_sha_of_canonical_json() {
        let payload = serde_json::json!({"z": 1, "a": {"b": 2, "a": 1}});
        let rendered = canonical_json(&payload).expect("canonical");
        let hash = canonical_payload_hash(&payload).expect("hash");

        assert_eq!(hash, sha256_hex(rendered.as_bytes()));
    }
}
