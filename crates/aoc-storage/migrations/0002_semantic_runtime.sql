CREATE TABLE IF NOT EXISTS semantic_runtime_provenance (
    artifact_id TEXT NOT NULL,
    stage TEXT NOT NULL,
    runtime TEXT NOT NULL,
    provider_name TEXT,
    model_id TEXT,
    prompt_version TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    output_hash TEXT,
    latency_ms INTEGER,
    attempt_count INTEGER NOT NULL,
    fallback_used INTEGER NOT NULL DEFAULT 0,
    fallback_reason TEXT,
    failure_kind TEXT,
    created_at TEXT NOT NULL,
    PRIMARY KEY (artifact_id, stage, attempt_count)
);

CREATE INDEX IF NOT EXISTS idx_semantic_runtime_provenance_artifact
    ON semantic_runtime_provenance(artifact_id, stage, attempt_count);

CREATE INDEX IF NOT EXISTS idx_semantic_runtime_provenance_runtime
    ON semantic_runtime_provenance(runtime, stage, created_at DESC);

INSERT OR IGNORE INTO mind_schema_migrations(version, applied_at)
VALUES (2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
