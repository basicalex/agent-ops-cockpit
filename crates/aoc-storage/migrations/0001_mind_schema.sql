PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS raw_events (
    event_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    ts TEXT NOT NULL,
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    attrs_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_raw_events_conversation_ts
    ON raw_events(conversation_id, ts);

CREATE TABLE IF NOT EXISTS compact_events_t0 (
    compact_id TEXT PRIMARY KEY,
    compact_hash TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    conversation_id TEXT NOT NULL,
    ts TEXT NOT NULL,
    role TEXT,
    text TEXT,
    snippet TEXT,
    source_event_ids_json TEXT NOT NULL,
    tool_meta_json TEXT,
    policy_version TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_compact_events_t0_conversation_ts
    ON compact_events_t0(conversation_id, ts);

CREATE UNIQUE INDEX IF NOT EXISTS idx_compact_events_t0_hash_policy
    ON compact_events_t0(compact_hash, policy_version);

CREATE TABLE IF NOT EXISTS observations_t1 (
    artifact_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    ts TEXT NOT NULL,
    importance INTEGER,
    text TEXT NOT NULL,
    trace_ids_json TEXT NOT NULL DEFAULT '[]'
);

CREATE INDEX IF NOT EXISTS idx_observations_t1_conversation_ts
    ON observations_t1(conversation_id, ts);

CREATE TABLE IF NOT EXISTS reflections_t2 (
    artifact_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    ts TEXT NOT NULL,
    text TEXT NOT NULL,
    trace_ids_json TEXT NOT NULL DEFAULT '[]'
);

CREATE INDEX IF NOT EXISTS idx_reflections_t2_conversation_ts
    ON reflections_t2(conversation_id, ts);

CREATE TABLE IF NOT EXISTS artifact_task_links (
    artifact_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    relation TEXT NOT NULL,
    confidence_bps INTEGER NOT NULL,
    source TEXT NOT NULL,
    evidence_event_ids_json TEXT NOT NULL DEFAULT '[]',
    start_ts TEXT NOT NULL,
    end_ts TEXT,
    PRIMARY KEY (artifact_id, task_id, relation)
);

CREATE INDEX IF NOT EXISTS idx_artifact_task_links_task
    ON artifact_task_links(task_id, confidence_bps DESC);

CREATE TABLE IF NOT EXISTS conversation_context_state (
    conversation_id TEXT NOT NULL,
    ts TEXT NOT NULL,
    active_tag TEXT,
    active_tasks_json TEXT NOT NULL DEFAULT '[]',
    lifecycle TEXT,
    signal_task_ids_json TEXT NOT NULL DEFAULT '[]',
    signal_source TEXT NOT NULL,
    PRIMARY KEY (conversation_id, ts)
);

CREATE INDEX IF NOT EXISTS idx_conversation_context_state_tag
    ON conversation_context_state(active_tag, ts);

CREATE TABLE IF NOT EXISTS segment_routes (
    artifact_id TEXT NOT NULL,
    segment_id TEXT NOT NULL,
    confidence_bps INTEGER NOT NULL,
    routed_by TEXT NOT NULL,
    reason TEXT,
    overridden_by TEXT,
    PRIMARY KEY (artifact_id, segment_id)
);

CREATE INDEX IF NOT EXISTS idx_segment_routes_segment_confidence
    ON segment_routes(segment_id, confidence_bps DESC);

CREATE TABLE IF NOT EXISTS aoc_mem_decisions (
    decision_id TEXT PRIMARY KEY,
    ts TEXT NOT NULL,
    project_id TEXT NOT NULL,
    segment_id TEXT,
    text TEXT NOT NULL,
    supersedes_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_aoc_mem_decisions_project_ts
    ON aoc_mem_decisions(project_id, ts DESC);

CREATE TABLE IF NOT EXISTS ingestion_checkpoints (
    conversation_id TEXT PRIMARY KEY,
    raw_cursor INTEGER NOT NULL,
    t0_cursor INTEGER NOT NULL,
    policy_version TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS mind_schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

INSERT OR IGNORE INTO mind_schema_migrations(version, applied_at)
VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
