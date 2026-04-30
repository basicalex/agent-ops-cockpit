DROP INDEX IF EXISTS idx_compaction_checkpoints_entry;

CREATE UNIQUE INDEX IF NOT EXISTS idx_compaction_checkpoints_conversation_entry
    ON compaction_checkpoints(conversation_id, compaction_entry_id)
    WHERE compaction_entry_id IS NOT NULL;
