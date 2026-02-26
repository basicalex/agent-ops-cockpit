CREATE TABLE IF NOT EXISTS conversation_lineage (
    conversation_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    parent_conversation_id TEXT,
    root_conversation_id TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_conversation_lineage_session
    ON conversation_lineage(session_id, root_conversation_id);

CREATE INDEX IF NOT EXISTS idx_conversation_lineage_parent
    ON conversation_lineage(parent_conversation_id);

INSERT OR IGNORE INTO mind_schema_migrations(version, applied_at)
VALUES (4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
