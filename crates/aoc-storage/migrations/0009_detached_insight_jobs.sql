CREATE TABLE IF NOT EXISTS detached_insight_jobs (
    job_id TEXT PRIMARY KEY,
    owner_plane TEXT NOT NULL,
    worker_kind TEXT,
    mode TEXT NOT NULL,
    status TEXT NOT NULL,
    agent TEXT,
    team TEXT,
    chain_name TEXT,
    created_at_ms INTEGER NOT NULL,
    started_at_ms INTEGER,
    finished_at_ms INTEGER,
    current_step_index INTEGER,
    step_count INTEGER,
    output_excerpt TEXT,
    error_text TEXT,
    fallback_used INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_detached_insight_jobs_owner_created
    ON detached_insight_jobs(owner_plane, created_at_ms DESC);

CREATE INDEX IF NOT EXISTS idx_detached_insight_jobs_owner_status_created
    ON detached_insight_jobs(owner_plane, status, created_at_ms DESC);

INSERT OR IGNORE INTO mind_schema_migrations(version, applied_at)
VALUES (9, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
