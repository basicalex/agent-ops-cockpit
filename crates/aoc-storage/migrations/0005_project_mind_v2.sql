CREATE TABLE IF NOT EXISTS t3_backlog_jobs (
    job_id TEXT PRIMARY KEY,
    project_root TEXT NOT NULL,
    session_id TEXT NOT NULL,
    pane_id TEXT NOT NULL,
    active_tag TEXT,
    slice_start_id TEXT,
    slice_end_id TEXT,
    artifact_refs_json TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    claimed_by TEXT,
    claimed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_t3_backlog_jobs_slice_dedupe
    ON t3_backlog_jobs(
        project_root,
        session_id,
        pane_id,
        COALESCE(slice_start_id, ''),
        COALESCE(slice_end_id, '')
    );

CREATE INDEX IF NOT EXISTS idx_t3_backlog_jobs_status_created
    ON t3_backlog_jobs(status, created_at);

CREATE INDEX IF NOT EXISTS idx_t3_backlog_jobs_project_status
    ON t3_backlog_jobs(project_root, status, created_at);

CREATE TABLE IF NOT EXISTS t3_runtime_leases (
    scope_id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    owner_pid INTEGER,
    acquired_at TEXT NOT NULL,
    heartbeat_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_t3_runtime_leases_expires
    ON t3_runtime_leases(expires_at);

CREATE TABLE IF NOT EXISTS project_canon_revisions (
    entry_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    state TEXT NOT NULL,
    topic TEXT,
    summary TEXT NOT NULL,
    confidence_bps INTEGER NOT NULL,
    freshness_score INTEGER NOT NULL,
    supersedes_entry_id TEXT,
    evidence_refs_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    PRIMARY KEY (entry_id, revision)
);

CREATE INDEX IF NOT EXISTS idx_project_canon_revisions_state_topic_created
    ON project_canon_revisions(state, topic, created_at DESC);

CREATE TABLE IF NOT EXISTS handshake_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    scope_key TEXT NOT NULL,
    payload_text TEXT NOT NULL,
    payload_hash TEXT NOT NULL,
    token_estimate INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_handshake_snapshots_scope_hash
    ON handshake_snapshots(scope, scope_key, payload_hash);

CREATE INDEX IF NOT EXISTS idx_handshake_snapshots_scope_created
    ON handshake_snapshots(scope, scope_key, created_at DESC);

CREATE TABLE IF NOT EXISTS project_watermarks (
    scope_key TEXT PRIMARY KEY,
    last_artifact_ts TEXT,
    last_artifact_id TEXT,
    updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO mind_schema_migrations(version, applied_at)
VALUES (5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
