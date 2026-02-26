CREATE TABLE IF NOT EXISTS reflector_runtime_leases (
    scope_id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    owner_pid INTEGER,
    acquired_at TEXT NOT NULL,
    heartbeat_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_reflector_runtime_leases_expires
    ON reflector_runtime_leases(expires_at);

CREATE TABLE IF NOT EXISTS reflector_jobs_t2 (
    job_id TEXT PRIMARY KEY,
    active_tag TEXT NOT NULL,
    observation_ids_json TEXT NOT NULL,
    conversation_ids_json TEXT NOT NULL,
    estimated_tokens INTEGER NOT NULL,
    status TEXT NOT NULL,
    claimed_by TEXT,
    claimed_at TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_reflector_jobs_t2_status_created
    ON reflector_jobs_t2(status, created_at);

CREATE INDEX IF NOT EXISTS idx_reflector_jobs_t2_tag_status
    ON reflector_jobs_t2(active_tag, status, created_at);

INSERT OR IGNORE INTO mind_schema_migrations(version, applied_at)
VALUES (3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
