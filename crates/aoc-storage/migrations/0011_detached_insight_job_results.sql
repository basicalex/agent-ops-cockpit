ALTER TABLE detached_insight_jobs ADD COLUMN stdout_excerpt TEXT;
ALTER TABLE detached_insight_jobs ADD COLUMN stderr_excerpt TEXT;
ALTER TABLE detached_insight_jobs ADD COLUMN step_results_json TEXT;

INSERT OR IGNORE INTO mind_schema_migrations(version, applied_at)
VALUES (11, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
