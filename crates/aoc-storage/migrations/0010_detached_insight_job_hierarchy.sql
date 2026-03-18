ALTER TABLE detached_insight_jobs ADD COLUMN parent_job_id TEXT;

CREATE INDEX IF NOT EXISTS idx_detached_insight_jobs_parent_created
    ON detached_insight_jobs(parent_job_id, created_at_ms DESC);

INSERT OR IGNORE INTO mind_schema_migrations(version, applied_at)
VALUES (10, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
