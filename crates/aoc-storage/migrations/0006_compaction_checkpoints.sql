CREATE TABLE IF NOT EXISTS compaction_checkpoints (
    checkpoint_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    ts TEXT NOT NULL,
    trigger_source TEXT NOT NULL,
    reason TEXT,
    summary TEXT,
    tokens_before INTEGER,
    first_kept_entry_id TEXT,
    compaction_entry_id TEXT,
    from_extension INTEGER NOT NULL DEFAULT 0,
    marker_event_id TEXT,
    schema_version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_compaction_checkpoints_entry
    ON compaction_checkpoints(compaction_entry_id)
    WHERE compaction_entry_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_compaction_checkpoints_conversation_ts
    ON compaction_checkpoints(conversation_id, ts DESC, checkpoint_id DESC);

CREATE INDEX IF NOT EXISTS idx_compaction_checkpoints_session_ts
    ON compaction_checkpoints(session_id, ts DESC, checkpoint_id DESC);
