CREATE TABLE IF NOT EXISTS compaction_slices_t0 (
    slice_id TEXT PRIMARY KEY,
    slice_hash TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
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
    source_kind TEXT NOT NULL,
    source_event_ids_json TEXT NOT NULL DEFAULT '[]',
    read_files_json TEXT NOT NULL DEFAULT '[]',
    modified_files_json TEXT NOT NULL DEFAULT '[]',
    checkpoint_id TEXT,
    policy_version TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_compaction_slices_t0_conversation_ts
    ON compaction_slices_t0(conversation_id, ts);

CREATE INDEX IF NOT EXISTS idx_compaction_slices_t0_session_ts
    ON compaction_slices_t0(session_id, ts);

CREATE INDEX IF NOT EXISTS idx_compaction_slices_t0_checkpoint
    ON compaction_slices_t0(checkpoint_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_compaction_slices_t0_conversation_entry
    ON compaction_slices_t0(conversation_id, compaction_entry_id)
    WHERE compaction_entry_id IS NOT NULL;

INSERT OR IGNORE INTO mind_schema_migrations(version, applied_at)
VALUES (8, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
