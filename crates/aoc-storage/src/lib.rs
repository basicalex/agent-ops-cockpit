use aoc_core::{
    insight_contracts::{
        InsightDetachedJob, InsightDetachedJobStatus, InsightDetachedMode,
        InsightDispatchStepResult,
    },
    mind_contracts::{
        canonical_payload_hash, parse_conversation_lineage_metadata, ArtifactTaskLink,
        ArtifactTaskRelation, CompactionT0Slice, ConversationRole, RawEvent, RawEventBody,
        RouteOrigin, SegmentCandidate, SegmentRoute, SemanticFailureKind, SemanticProvenance,
        SemanticRuntime, SemanticStage, T0CompactEvent, ToolMetadataLine,
    },
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeSet;
use std::path::Path;
use thiserror::Error;

pub const MIND_SCHEMA_VERSION: i64 = 11;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("timestamp parse error: {0}")]
    Timestamp(String),
    #[error("unsupported schema version {found}, max supported {supported}")]
    UnsupportedSchemaVersion { found: i64, supported: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestionCheckpoint {
    pub conversation_id: String,
    pub raw_cursor: u64,
    pub t0_cursor: u64,
    pub policy_version: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationContextState {
    pub conversation_id: String,
    pub ts: DateTime<Utc>,
    pub active_tag: Option<String>,
    pub active_tasks: Vec<String>,
    pub lifecycle: Option<String>,
    pub signal_task_ids: Vec<String>,
    pub signal_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationLineage {
    pub conversation_id: String,
    pub session_id: String,
    pub parent_conversation_id: Option<String>,
    pub root_conversation_id: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredArtifact {
    pub artifact_id: String,
    pub conversation_id: String,
    pub ts: DateTime<Utc>,
    pub text: String,
    pub kind: String,
    pub trace_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactFileLink {
    pub artifact_id: String,
    pub path: String,
    pub relation: String,
    pub source: String,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
    pub staged: bool,
    pub untracked: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCompactEvent {
    pub compact_id: String,
    pub conversation_id: String,
    pub ts: DateTime<Utc>,
    pub role: Option<ConversationRole>,
    pub text: Option<String>,
    pub tool_meta: Option<ToolMetadataLine>,
    pub source_event_ids: Vec<String>,
    pub policy_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReflectorJobStatus {
    Pending,
    Claimed,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectorLease {
    pub scope_id: String,
    pub owner_id: String,
    pub owner_pid: Option<i64>,
    pub acquired_at: DateTime<Utc>,
    pub heartbeat_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectorJob {
    pub job_id: String,
    pub active_tag: String,
    pub observation_ids: Vec<String>,
    pub conversation_ids: Vec<String>,
    pub estimated_tokens: u32,
    pub status: ReflectorJobStatus,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub attempts: u16,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LegacyImportReport {
    pub tables_scanned: usize,
    pub tables_imported: usize,
    pub rows_imported: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectWatermark {
    pub scope_key: String,
    pub last_artifact_ts: Option<DateTime<Utc>>,
    pub last_artifact_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T3BacklogJobStatus {
    Pending,
    Claimed,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct T3RuntimeLease {
    pub scope_id: String,
    pub owner_id: String,
    pub owner_pid: Option<i64>,
    pub acquired_at: DateTime<Utc>,
    pub heartbeat_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct T3BacklogJob {
    pub job_id: String,
    pub project_root: String,
    pub session_id: String,
    pub pane_id: String,
    pub active_tag: Option<String>,
    pub slice_start_id: Option<String>,
    pub slice_end_id: Option<String>,
    pub artifact_refs: Vec<String>,
    pub status: T3BacklogJobStatus,
    pub attempts: u16,
    pub last_error: Option<String>,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonRevisionState {
    Active,
    Superseded,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonEntryRevision {
    pub entry_id: String,
    pub revision: i64,
    pub state: CanonRevisionState,
    pub topic: Option<String>,
    pub summary: String,
    pub confidence_bps: u16,
    pub freshness_score: u16,
    pub supersedes_entry_id: Option<String>,
    pub evidence_refs: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeSnapshot {
    pub snapshot_id: String,
    pub scope: String,
    pub scope_key: String,
    pub payload_text: String,
    pub payload_hash: String,
    pub token_estimate: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionCheckpoint {
    pub checkpoint_id: String,
    pub conversation_id: String,
    pub session_id: String,
    pub ts: DateTime<Utc>,
    pub trigger_source: String,
    pub reason: Option<String>,
    pub summary: Option<String>,
    pub tokens_before: Option<u32>,
    pub first_kept_entry_id: Option<String>,
    pub compaction_entry_id: Option<String>,
    pub from_extension: bool,
    pub marker_event_id: Option<String>,
    pub schema_version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCompactionT0Slice {
    pub slice_id: String,
    pub slice_hash: String,
    pub schema_version: u32,
    pub conversation_id: String,
    pub session_id: String,
    pub ts: DateTime<Utc>,
    pub trigger_source: String,
    pub reason: Option<String>,
    pub summary: Option<String>,
    pub tokens_before: Option<u32>,
    pub first_kept_entry_id: Option<String>,
    pub compaction_entry_id: Option<String>,
    pub from_extension: bool,
    pub source_kind: String,
    pub source_event_ids: Vec<String>,
    pub read_files: Vec<String>,
    pub modified_files: Vec<String>,
    pub checkpoint_id: Option<String>,
    pub policy_version: String,
}

pub struct MindStore {
    conn: Connection,
}

impl MindStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn schema_version(&self) -> Result<i64, StorageError> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }

    pub fn migrate(&self) -> Result<(), StorageError> {
        let mut current = self.schema_version()?;
        if current > MIND_SCHEMA_VERSION {
            return Err(StorageError::UnsupportedSchemaVersion {
                found: current,
                supported: MIND_SCHEMA_VERSION,
            });
        }

        if current < 1 {
            let sql = include_str!("../migrations/0001_mind_schema.sql");
            self.conn.execute_batch(sql)?;
            self.conn
                .execute("PRAGMA user_version = 1", [])
                .map(|_| ())?;
            current = 1;
        }

        if current < 2 {
            let sql = include_str!("../migrations/0002_semantic_runtime.sql");
            self.conn.execute_batch(sql)?;
            self.conn
                .execute("PRAGMA user_version = 2", [])
                .map(|_| ())?;
            current = 2;
        }

        if current < 3 {
            let sql = include_str!("../migrations/0003_reflector_runtime.sql");
            self.conn.execute_batch(sql)?;
            self.conn
                .execute("PRAGMA user_version = 3", [])
                .map(|_| ())?;
            current = 3;
        }

        if current < 4 {
            let sql = include_str!("../migrations/0004_session_conversation_tree.sql");
            self.conn.execute_batch(sql)?;
            self.conn
                .execute("PRAGMA user_version = 4", [])
                .map(|_| ())?;
            current = 4;
        }

        if current < 5 {
            let sql = include_str!("../migrations/0005_project_mind_v2.sql");
            self.conn.execute_batch(sql)?;
            self.conn
                .execute("PRAGMA user_version = 5", [])
                .map(|_| ())?;
            current = 5;
        }

        if current < 6 {
            let sql = include_str!("../migrations/0006_compaction_checkpoints.sql");
            self.conn.execute_batch(sql)?;
            self.conn
                .execute("PRAGMA user_version = 6", [])
                .map(|_| ())?;
            current = 6;
        }

        if current < 7 {
            let sql = include_str!("../migrations/0007_artifact_file_links.sql");
            self.conn.execute_batch(sql)?;
            self.conn
                .execute("PRAGMA user_version = 7", [])
                .map(|_| ())?;
            current = 7;
        }

        if current < 8 {
            let sql = include_str!("../migrations/0008_compaction_slices_t0.sql");
            self.conn.execute_batch(sql)?;
            self.conn
                .execute("PRAGMA user_version = 8", [])
                .map(|_| ())?;
            current = 8;
        }

        if current < 9 {
            let sql = include_str!("../migrations/0009_detached_insight_jobs.sql");
            self.conn.execute_batch(sql)?;
            self.conn
                .execute("PRAGMA user_version = 9", [])
                .map(|_| ())?;
            current = 9;
        }

        if current < 10 {
            let sql = include_str!("../migrations/0010_detached_insight_job_hierarchy.sql");
            self.conn.execute_batch(sql)?;
            self.conn
                .execute("PRAGMA user_version = 10", [])
                .map(|_| ())?;
            current = 10;
        }

        if current < 11 {
            let sql = include_str!("../migrations/0011_detached_insight_job_results.sql");
            self.conn.execute_batch(sql)?;
            self.conn
                .execute("PRAGMA user_version = 11", [])
                .map(|_| ())?;
        }

        Ok(())
    }

    pub fn insert_raw_event(&self, event: &RawEvent) -> Result<bool, StorageError> {
        let payload_json = serde_json::to_string(&event.body)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;
        let attrs_json = serde_json::to_string(&event.attrs)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;
        let kind = match event.body {
            RawEventBody::Message(_) => "message",
            RawEventBody::ToolResult(_) => "tool_result",
            RawEventBody::TaskSignal(_) => "task_signal",
            RawEventBody::Other { .. } => "other",
        };

        let changes = self.conn.execute(
            "
            INSERT OR IGNORE INTO raw_events (
                event_id,
                conversation_id,
                agent_id,
                ts,
                kind,
                payload_json,
                attrs_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                event.event_id,
                event.conversation_id,
                event.agent_id,
                event.ts.to_rfc3339(),
                kind,
                payload_json,
                attrs_json,
            ],
        )?;

        if changes > 0 {
            self.upsert_conversation_lineage_from_event(event)?;
        }

        Ok(changes > 0)
    }

    pub fn conversation_lineage(
        &self,
        conversation_id: &str,
    ) -> Result<Option<ConversationLineage>, StorageError> {
        self.conn
            .query_row(
                "
                SELECT conversation_id, session_id, parent_conversation_id, root_conversation_id, updated_at
                FROM conversation_lineage
                WHERE conversation_id = ?1
                ",
                [conversation_id],
                |row| {
                    let updated_at = parse_timestamp(row.get::<_, String>(4)?).map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?;
                    Ok(ConversationLineage {
                        conversation_id: row.get(0)?,
                        session_id: row.get(1)?,
                        parent_conversation_id: row.get(2)?,
                        root_conversation_id: row.get(3)?,
                        updated_at,
                    })
                },
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn session_tree_conversations(
        &self,
        session_id: &str,
        seed_conversation_id: &str,
    ) -> Result<Vec<String>, StorageError> {
        let seed = self.conversation_lineage(seed_conversation_id)?;

        let (query, args): (&str, Vec<&str>) = if let Some(seed) = seed.as_ref() {
            (
                "
                SELECT conversation_id
                FROM conversation_lineage
                WHERE session_id = ?1 AND root_conversation_id = ?2
                ORDER BY conversation_id ASC
                ",
                vec![session_id, seed.root_conversation_id.as_str()],
            )
        } else {
            (
                "
                SELECT conversation_id
                FROM conversation_lineage
                WHERE session_id = ?1
                ORDER BY conversation_id ASC
                ",
                vec![session_id],
            )
        };

        let mut statement = self.conn.prepare(query)?;
        let rows =
            statement.query_map(rusqlite::params_from_iter(args.iter()), |row| row.get(0))?;
        let mut conversation_ids = Vec::new();
        for row in rows {
            conversation_ids.push(row?);
        }

        if !conversation_ids
            .iter()
            .any(|conversation_id| conversation_id == seed_conversation_id)
        {
            conversation_ids.push(seed_conversation_id.to_string());
        }
        conversation_ids.sort();
        conversation_ids.dedup();
        Ok(conversation_ids)
    }

    pub fn conversation_ids_for_session(
        &self,
        session_id: &str,
    ) -> Result<Vec<String>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT conversation_id
            FROM conversation_lineage
            WHERE session_id = ?1
            ORDER BY conversation_id ASC
            ",
        )?;
        let rows = statement.query_map([session_id], |row| row.get(0))?;

        let mut conversation_ids = Vec::new();
        for row in rows {
            conversation_ids.push(row?);
        }
        conversation_ids.sort();
        conversation_ids.dedup();
        Ok(conversation_ids)
    }

    pub fn conversation_needs_observer_run(
        &self,
        conversation_id: &str,
    ) -> Result<bool, StorageError> {
        let (latest_t0, latest_t1): (Option<String>, Option<String>) = self.conn.query_row(
            "
            SELECT
                (SELECT MAX(ts) FROM compact_events_t0 WHERE conversation_id = ?1),
                (SELECT MAX(ts) FROM observations_t1 WHERE conversation_id = ?1)
            ",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let Some(latest_t0) = latest_t0 else {
            return Ok(false);
        };
        let latest_t0 = parse_timestamp(latest_t0)?;
        let Some(latest_t1) = latest_t1 else {
            return Ok(true);
        };
        let latest_t1 = parse_timestamp(latest_t1)?;
        Ok(latest_t0 > latest_t1)
    }

    fn upsert_conversation_lineage_from_event(&self, event: &RawEvent) -> Result<(), StorageError> {
        let metadata = parse_conversation_lineage_metadata(
            &event.attrs,
            &event.conversation_id,
            &event.agent_id,
        )
        .map_err(|err| StorageError::Serialization(err.to_string()))?;

        let Some(metadata) = metadata else {
            return Ok(());
        };

        self.conn.execute(
            "
            INSERT INTO conversation_lineage (
                conversation_id,
                session_id,
                parent_conversation_id,
                root_conversation_id,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(conversation_id) DO UPDATE SET
                session_id=excluded.session_id,
                parent_conversation_id=excluded.parent_conversation_id,
                root_conversation_id=excluded.root_conversation_id,
                updated_at=excluded.updated_at
            ",
            params![
                event.conversation_id,
                metadata.session_id,
                metadata.parent_conversation_id,
                metadata.root_conversation_id,
                event.ts.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    pub fn upsert_t0_compact_event(&self, event: &T0CompactEvent) -> Result<(), StorageError> {
        let source_event_ids_json = serde_json::to_string(&event.source_event_ids)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;
        let tool_meta_json = event
            .tool_meta
            .as_ref()
            .map(|tool_meta| {
                serde_json::to_string(tool_meta)
                    .map_err(|err| StorageError::Serialization(err.to_string()))
            })
            .transpose()?;
        let role = event.role.map(role_as_str);

        self.conn.execute(
            "
            INSERT INTO compact_events_t0 (
                compact_id,
                compact_hash,
                schema_version,
                conversation_id,
                ts,
                role,
                text,
                snippet,
                source_event_ids_json,
                tool_meta_json,
                policy_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(compact_id) DO UPDATE SET
                compact_hash=excluded.compact_hash,
                schema_version=excluded.schema_version,
                conversation_id=excluded.conversation_id,
                ts=excluded.ts,
                role=excluded.role,
                text=excluded.text,
                snippet=excluded.snippet,
                source_event_ids_json=excluded.source_event_ids_json,
                tool_meta_json=excluded.tool_meta_json,
                policy_version=excluded.policy_version
            ",
            params![
                event.compact_id,
                event.compact_hash,
                i64::from(event.schema_version),
                event.conversation_id,
                event.ts.to_rfc3339(),
                role,
                event.text,
                event.snippet,
                source_event_ids_json,
                tool_meta_json,
                event.policy_version,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_checkpoint(&self, checkpoint: &IngestionCheckpoint) -> Result<(), StorageError> {
        self.conn.execute(
            "
            INSERT INTO ingestion_checkpoints (
                conversation_id,
                raw_cursor,
                t0_cursor,
                policy_version,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(conversation_id) DO UPDATE SET
                raw_cursor=excluded.raw_cursor,
                t0_cursor=excluded.t0_cursor,
                policy_version=excluded.policy_version,
                updated_at=excluded.updated_at
            ",
            params![
                checkpoint.conversation_id,
                checkpoint.raw_cursor as i64,
                checkpoint.t0_cursor as i64,
                checkpoint.policy_version,
                checkpoint.updated_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    pub fn checkpoint(
        &self,
        conversation_id: &str,
    ) -> Result<Option<IngestionCheckpoint>, StorageError> {
        let row = self
            .conn
            .query_row(
                "
                SELECT conversation_id, raw_cursor, t0_cursor, policy_version, updated_at
                FROM ingestion_checkpoints
                WHERE conversation_id = ?1
                ",
                [conversation_id],
                |row| {
                    let updated_at: String = row.get(4)?;
                    let updated_at = DateTime::parse_from_rfc3339(&updated_at)
                        .map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?
                        .with_timezone(&Utc);

                    Ok(IngestionCheckpoint {
                        conversation_id: row.get(0)?,
                        raw_cursor: row.get::<_, i64>(1)? as u64,
                        t0_cursor: row.get::<_, i64>(2)? as u64,
                        policy_version: row.get(3)?,
                        updated_at,
                    })
                },
            )
            .optional()?;

        Ok(row)
    }

    pub fn upsert_compaction_checkpoint(
        &self,
        checkpoint: &CompactionCheckpoint,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "
            INSERT INTO compaction_checkpoints (
                checkpoint_id,
                conversation_id,
                session_id,
                ts,
                trigger_source,
                reason,
                summary,
                tokens_before,
                first_kept_entry_id,
                compaction_entry_id,
                from_extension,
                marker_event_id,
                schema_version,
                created_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT(checkpoint_id) DO UPDATE SET
                conversation_id=excluded.conversation_id,
                session_id=excluded.session_id,
                ts=excluded.ts,
                trigger_source=excluded.trigger_source,
                reason=excluded.reason,
                summary=excluded.summary,
                tokens_before=excluded.tokens_before,
                first_kept_entry_id=excluded.first_kept_entry_id,
                compaction_entry_id=excluded.compaction_entry_id,
                from_extension=excluded.from_extension,
                marker_event_id=excluded.marker_event_id,
                schema_version=excluded.schema_version,
                updated_at=excluded.updated_at
            ",
            params![
                checkpoint.checkpoint_id,
                checkpoint.conversation_id,
                checkpoint.session_id,
                checkpoint.ts.to_rfc3339(),
                checkpoint.trigger_source,
                checkpoint.reason,
                checkpoint.summary,
                checkpoint.tokens_before.map(i64::from),
                checkpoint.first_kept_entry_id,
                checkpoint.compaction_entry_id,
                if checkpoint.from_extension {
                    1_i64
                } else {
                    0_i64
                },
                checkpoint.marker_event_id,
                i64::from(checkpoint.schema_version),
                checkpoint.created_at.to_rfc3339(),
                checkpoint.updated_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    pub fn compaction_checkpoints_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<CompactionCheckpoint>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT checkpoint_id, conversation_id, session_id, ts, trigger_source,
                   reason, summary, tokens_before, first_kept_entry_id, compaction_entry_id,
                   from_extension, marker_event_id, schema_version, created_at, updated_at
            FROM compaction_checkpoints
            WHERE conversation_id = ?1
            ORDER BY ts DESC, checkpoint_id DESC
            ",
        )?;

        let rows = statement.query_map([conversation_id], parse_compaction_checkpoint_row)?;
        let mut checkpoints = Vec::new();
        for row in rows {
            checkpoints.push(row?);
        }
        Ok(checkpoints)
    }

    pub fn latest_compaction_checkpoint_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Option<CompactionCheckpoint>, StorageError> {
        self.conn
            .query_row(
                "
                SELECT checkpoint_id, conversation_id, session_id, ts, trigger_source,
                       reason, summary, tokens_before, first_kept_entry_id, compaction_entry_id,
                       from_extension, marker_event_id, schema_version, created_at, updated_at
                FROM compaction_checkpoints
                WHERE conversation_id = ?1
                ORDER BY ts DESC, checkpoint_id DESC
                LIMIT 1
                ",
                [conversation_id],
                parse_compaction_checkpoint_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn compaction_checkpoint_by_id(
        &self,
        checkpoint_id: &str,
    ) -> Result<Option<CompactionCheckpoint>, StorageError> {
        self.conn
            .query_row(
                "
                SELECT checkpoint_id, conversation_id, session_id, ts, trigger_source,
                       reason, summary, tokens_before, first_kept_entry_id, compaction_entry_id,
                       from_extension, marker_event_id, schema_version, created_at, updated_at
                FROM compaction_checkpoints
                WHERE checkpoint_id = ?1
                LIMIT 1
                ",
                [checkpoint_id],
                parse_compaction_checkpoint_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn latest_compaction_checkpoint_for_session(
        &self,
        session_id: &str,
    ) -> Result<Option<CompactionCheckpoint>, StorageError> {
        self.conn
            .query_row(
                "
                SELECT checkpoint_id, conversation_id, session_id, ts, trigger_source,
                       reason, summary, tokens_before, first_kept_entry_id, compaction_entry_id,
                       from_extension, marker_event_id, schema_version, created_at, updated_at
                FROM compaction_checkpoints
                WHERE session_id = ?1
                ORDER BY ts DESC, checkpoint_id DESC
                LIMIT 1
                ",
                [session_id],
                parse_compaction_checkpoint_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn upsert_compaction_t0_slice(
        &self,
        slice: &CompactionT0Slice,
    ) -> Result<(), StorageError> {
        let source_event_ids_json = serde_json::to_string(&slice.source_event_ids)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;
        let read_files_json = serde_json::to_string(&slice.read_files)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;
        let modified_files_json = serde_json::to_string(&slice.modified_files)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;

        self.conn.execute(
            "
            INSERT INTO compaction_slices_t0 (
                slice_id,
                slice_hash,
                schema_version,
                conversation_id,
                session_id,
                ts,
                trigger_source,
                reason,
                summary,
                tokens_before,
                first_kept_entry_id,
                compaction_entry_id,
                from_extension,
                source_kind,
                source_event_ids_json,
                read_files_json,
                modified_files_json,
                checkpoint_id,
                policy_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            ON CONFLICT(slice_id) DO UPDATE SET
                slice_hash=excluded.slice_hash,
                schema_version=excluded.schema_version,
                conversation_id=excluded.conversation_id,
                session_id=excluded.session_id,
                ts=excluded.ts,
                trigger_source=excluded.trigger_source,
                reason=excluded.reason,
                summary=excluded.summary,
                tokens_before=excluded.tokens_before,
                first_kept_entry_id=excluded.first_kept_entry_id,
                compaction_entry_id=excluded.compaction_entry_id,
                from_extension=excluded.from_extension,
                source_kind=excluded.source_kind,
                source_event_ids_json=excluded.source_event_ids_json,
                read_files_json=excluded.read_files_json,
                modified_files_json=excluded.modified_files_json,
                checkpoint_id=excluded.checkpoint_id,
                policy_version=excluded.policy_version
            ",
            params![
                slice.slice_id,
                slice.slice_hash,
                i64::from(slice.schema_version),
                slice.conversation_id,
                slice.session_id,
                slice.ts.to_rfc3339(),
                slice.trigger_source,
                slice.reason,
                slice.summary,
                slice.tokens_before.map(i64::from),
                slice.first_kept_entry_id,
                slice.compaction_entry_id,
                if slice.from_extension { 1_i64 } else { 0_i64 },
                slice.source_kind,
                source_event_ids_json,
                read_files_json,
                modified_files_json,
                slice.checkpoint_id,
                slice.policy_version,
            ],
        )?;

        Ok(())
    }

    pub fn compaction_t0_slices_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<StoredCompactionT0Slice>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT slice_id, slice_hash, schema_version, conversation_id, session_id, ts,
                   trigger_source, reason, summary, tokens_before, first_kept_entry_id,
                   compaction_entry_id, from_extension, source_kind, source_event_ids_json,
                   read_files_json, modified_files_json, checkpoint_id, policy_version
            FROM compaction_slices_t0
            WHERE conversation_id = ?1
            ORDER BY ts DESC, slice_id DESC
            ",
        )?;

        let rows = statement.query_map([conversation_id], parse_compaction_t0_slice_row)?;
        let mut slices = Vec::new();
        for row in rows {
            slices.push(row?);
        }
        Ok(slices)
    }

    pub fn latest_compaction_t0_slice_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Option<StoredCompactionT0Slice>, StorageError> {
        self.conn
            .query_row(
                "
                SELECT slice_id, slice_hash, schema_version, conversation_id, session_id, ts,
                       trigger_source, reason, summary, tokens_before, first_kept_entry_id,
                       compaction_entry_id, from_extension, source_kind, source_event_ids_json,
                       read_files_json, modified_files_json, checkpoint_id, policy_version
                FROM compaction_slices_t0
                WHERE conversation_id = ?1
                ORDER BY ts DESC, slice_id DESC
                LIMIT 1
                ",
                [conversation_id],
                parse_compaction_t0_slice_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn latest_compaction_t0_slice_for_session(
        &self,
        session_id: &str,
    ) -> Result<Option<StoredCompactionT0Slice>, StorageError> {
        self.conn
            .query_row(
                "
                SELECT slice_id, slice_hash, schema_version, conversation_id, session_id, ts,
                       trigger_source, reason, summary, tokens_before, first_kept_entry_id,
                       compaction_entry_id, from_extension, source_kind, source_event_ids_json,
                       read_files_json, modified_files_json, checkpoint_id, policy_version
                FROM compaction_slices_t0
                WHERE session_id = ?1
                ORDER BY ts DESC, slice_id DESC
                LIMIT 1
                ",
                [session_id],
                parse_compaction_t0_slice_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn compaction_t0_slice_for_checkpoint(
        &self,
        checkpoint_id: &str,
    ) -> Result<Option<StoredCompactionT0Slice>, StorageError> {
        self.conn
            .query_row(
                "
                SELECT slice_id, slice_hash, schema_version, conversation_id, session_id, ts,
                       trigger_source, reason, summary, tokens_before, first_kept_entry_id,
                       compaction_entry_id, from_extension, source_kind, source_event_ids_json,
                       read_files_json, modified_files_json, checkpoint_id, policy_version
                FROM compaction_slices_t0
                WHERE checkpoint_id = ?1
                LIMIT 1
                ",
                [checkpoint_id],
                parse_compaction_t0_slice_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn append_context_state(
        &self,
        snapshot: &ConversationContextState,
    ) -> Result<(), StorageError> {
        let mut deduped_tasks = snapshot
            .active_tasks
            .iter()
            .cloned()
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect::<Vec<String>>();
        deduped_tasks.sort();
        let active_tasks_json = serde_json::to_string(&deduped_tasks)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;
        let mut deduped_signal_tasks = snapshot
            .signal_task_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect::<Vec<String>>();
        deduped_signal_tasks.sort();
        let signal_task_ids_json = serde_json::to_string(&deduped_signal_tasks)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;

        self.conn.execute(
            "
            INSERT OR REPLACE INTO conversation_context_state (
                conversation_id,
                ts,
                active_tag,
                active_tasks_json,
                lifecycle,
                signal_task_ids_json,
                signal_source
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                snapshot.conversation_id,
                snapshot.ts.to_rfc3339(),
                snapshot.active_tag,
                active_tasks_json,
                snapshot.lifecycle,
                signal_task_ids_json,
                snapshot.signal_source,
            ],
        )?;

        Ok(())
    }

    pub fn latest_context_state(
        &self,
        conversation_id: &str,
    ) -> Result<Option<ConversationContextState>, StorageError> {
        let snapshot = self
            .conn
            .query_row(
                "
                SELECT conversation_id, ts, active_tag, active_tasks_json, lifecycle, signal_task_ids_json, signal_source
                FROM conversation_context_state
                WHERE conversation_id = ?1
                ORDER BY ts DESC
                LIMIT 1
                ",
                [conversation_id],
                |row| {
                    let ts = parse_timestamp(row.get::<_, String>(1)?).map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?;
                    let active_tasks_json: String = row.get(3)?;
                    let mut active_tasks: Vec<String> = serde_json::from_str(&active_tasks_json)
                        .map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                3,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?;
                    active_tasks.sort();
                    active_tasks.dedup();

                    let signal_task_ids_json: String = row.get(5)?;
                    let mut signal_task_ids: Vec<String> =
                        serde_json::from_str(&signal_task_ids_json).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                5,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?;
                    signal_task_ids.sort();
                    signal_task_ids.dedup();

                    Ok(ConversationContextState {
                        conversation_id: row.get(0)?,
                        ts,
                        active_tag: row.get(2)?,
                        active_tasks,
                        lifecycle: row.get(4)?,
                        signal_task_ids,
                        signal_source: row.get(6)?,
                    })
                },
            )
            .optional()?;

        Ok(snapshot)
    }

    pub fn context_states(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationContextState>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT conversation_id, ts, active_tag, active_tasks_json, lifecycle, signal_task_ids_json, signal_source
            FROM conversation_context_state
            WHERE conversation_id = ?1
            ORDER BY ts ASC
            ",
        )?;

        let rows = statement.query_map([conversation_id], |row| {
            let ts = parse_timestamp(row.get::<_, String>(1)?).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;
            let active_tasks_json: String = row.get(3)?;
            let mut active_tasks: Vec<String> =
                serde_json::from_str(&active_tasks_json).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?;
            active_tasks.sort();
            active_tasks.dedup();

            let signal_task_ids_json: String = row.get(5)?;
            let mut signal_task_ids: Vec<String> = serde_json::from_str(&signal_task_ids_json)
                .map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?;
            signal_task_ids.sort();
            signal_task_ids.dedup();

            Ok(ConversationContextState {
                conversation_id: row.get(0)?,
                ts,
                active_tag: row.get(2)?,
                active_tasks,
                lifecycle: row.get(4)?,
                signal_task_ids,
                signal_source: row.get(6)?,
            })
        })?;

        let mut snapshots = Vec::new();
        for row in rows {
            snapshots.push(row?);
        }
        Ok(snapshots)
    }

    pub fn context_state_count(&self, conversation_id: &str) -> Result<i64, StorageError> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM conversation_context_state WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn insert_observation(
        &self,
        artifact_id: &str,
        conversation_id: &str,
        ts: DateTime<Utc>,
        text: &str,
        trace_ids: &[String],
    ) -> Result<(), StorageError> {
        let trace_ids_json = serde_json::to_string(trace_ids)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;
        self.conn.execute(
            "
            INSERT OR REPLACE INTO observations_t1 (
                artifact_id,
                conversation_id,
                ts,
                importance,
                text,
                trace_ids_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                artifact_id,
                conversation_id,
                ts.to_rfc3339(),
                0_i64,
                text,
                trace_ids_json
            ],
        )?;
        Ok(())
    }

    pub fn upsert_artifact_file_link(&self, link: &ArtifactFileLink) -> Result<(), StorageError> {
        self.conn.execute(
            "
            INSERT INTO artifact_file_links (
                artifact_id,
                path,
                relation,
                source,
                additions,
                deletions,
                staged,
                untracked,
                created_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(artifact_id, path, relation) DO UPDATE SET
                source = excluded.source,
                additions = excluded.additions,
                deletions = excluded.deletions,
                staged = excluded.staged,
                untracked = excluded.untracked,
                updated_at = excluded.updated_at
            ",
            params![
                link.artifact_id,
                link.path,
                link.relation,
                link.source,
                link.additions.map(i64::from),
                link.deletions.map(i64::from),
                if link.staged { 1_i64 } else { 0_i64 },
                if link.untracked { 1_i64 } else { 0_i64 },
                link.created_at.to_rfc3339(),
                link.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn artifact_file_links(
        &self,
        artifact_id: &str,
    ) -> Result<Vec<ArtifactFileLink>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT artifact_id, path, relation, source, additions, deletions, staged, untracked,
                   created_at, updated_at
            FROM artifact_file_links
            WHERE artifact_id = ?1
            ORDER BY relation ASC, path ASC
            ",
        )?;
        let rows = statement.query_map([artifact_id], parse_artifact_file_link_row)?;
        let mut links = Vec::new();
        for row in rows {
            links.push(row?);
        }
        Ok(links)
    }

    pub fn insert_reflection(
        &self,
        artifact_id: &str,
        conversation_id: &str,
        ts: DateTime<Utc>,
        text: &str,
        trace_ids: &[String],
    ) -> Result<(), StorageError> {
        let trace_ids_json = serde_json::to_string(trace_ids)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;
        self.conn.execute(
            "
            INSERT OR REPLACE INTO reflections_t2 (
                artifact_id,
                conversation_id,
                ts,
                text,
                trace_ids_json
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                artifact_id,
                conversation_id,
                ts.to_rfc3339(),
                text,
                trace_ids_json
            ],
        )?;
        Ok(())
    }

    pub fn append_trace_ids_to_artifact(
        &self,
        artifact_id: &str,
        extra_trace_ids: &[String],
    ) -> Result<bool, StorageError> {
        let Some(artifact) = self.artifact_by_id(artifact_id)? else {
            return Ok(false);
        };

        let mut merged = artifact.trace_ids;
        merged.extend(extra_trace_ids.iter().cloned());
        merged.sort();
        merged.dedup();
        let trace_ids_json = serde_json::to_string(&merged)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;

        let changed = match artifact.kind.as_str() {
            "t1" => self.conn.execute(
                "UPDATE observations_t1 SET trace_ids_json = ?2 WHERE artifact_id = ?1",
                params![artifact_id, trace_ids_json],
            )?,
            "t2" => self.conn.execute(
                "UPDATE reflections_t2 SET trace_ids_json = ?2 WHERE artifact_id = ?1",
                params![artifact_id, trace_ids_json],
            )?,
            _ => 0,
        };

        Ok(changed > 0)
    }

    pub fn artifacts_with_trace_id(
        &self,
        conversation_id: &str,
        trace_id: &str,
    ) -> Result<Vec<StoredArtifact>, StorageError> {
        let artifacts = self.artifacts_for_conversation(conversation_id)?;
        Ok(artifacts
            .into_iter()
            .filter(|artifact| artifact.trace_ids.iter().any(|id| id == trace_id))
            .collect())
    }

    pub fn upsert_semantic_provenance(
        &self,
        provenance: &SemanticProvenance,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "
            INSERT OR REPLACE INTO semantic_runtime_provenance (
                artifact_id,
                stage,
                runtime,
                provider_name,
                model_id,
                prompt_version,
                input_hash,
                output_hash,
                latency_ms,
                attempt_count,
                fallback_used,
                fallback_reason,
                failure_kind,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ",
            params![
                provenance.artifact_id,
                semantic_stage_as_str(provenance.stage),
                semantic_runtime_as_str(provenance.runtime),
                provenance.provider_name,
                provenance.model_id,
                provenance.prompt_version,
                provenance.input_hash,
                provenance.output_hash,
                provenance.latency_ms.map(|value| value as i64),
                i64::from(provenance.attempt_count),
                if provenance.fallback_used {
                    1_i64
                } else {
                    0_i64
                },
                provenance.fallback_reason,
                provenance.failure_kind.map(semantic_failure_kind_as_str),
                provenance.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn semantic_provenance_for_artifact(
        &self,
        artifact_id: &str,
    ) -> Result<Vec<SemanticProvenance>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT artifact_id, stage, runtime, provider_name, model_id, prompt_version,
                   input_hash, output_hash, latency_ms, attempt_count, fallback_used,
                   fallback_reason, failure_kind, created_at
            FROM semantic_runtime_provenance
            WHERE artifact_id = ?1
            ORDER BY attempt_count ASC
            ",
        )?;

        let rows = statement.query_map([artifact_id], |row| {
            let stage_raw: String = row.get(1)?;
            let runtime_raw: String = row.get(2)?;
            let stage = parse_semantic_stage(&stage_raw).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("invalid semantic stage: {stage_raw}"),
                    )),
                )
            })?;
            let runtime = parse_semantic_runtime(&runtime_raw).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("invalid semantic runtime: {runtime_raw}"),
                    )),
                )
            })?;

            let failure_kind = row
                .get::<_, Option<String>>(12)?
                .map(|value| {
                    parse_semantic_failure_kind(&value).ok_or_else(|| {
                        rusqlite::Error::FromSqlConversionFailure(
                            12,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("invalid semantic failure kind: {value}"),
                            )),
                        )
                    })
                })
                .transpose()?;

            let created_at = parse_timestamp(row.get::<_, String>(13)?).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    13,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;

            Ok(SemanticProvenance {
                artifact_id: row.get(0)?,
                stage,
                runtime,
                provider_name: row.get(3)?,
                model_id: row.get(4)?,
                prompt_version: row.get(5)?,
                input_hash: row.get(6)?,
                output_hash: row.get(7)?,
                latency_ms: row.get::<_, Option<i64>>(8)?.map(|value| value as u64),
                attempt_count: row.get::<_, i64>(9)? as u16,
                fallback_used: row.get::<_, i64>(10)? != 0,
                fallback_reason: row.get(11)?,
                failure_kind,
                created_at,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn try_acquire_reflector_lease(
        &self,
        scope_id: &str,
        owner_id: &str,
        owner_pid: Option<i64>,
        now: DateTime<Utc>,
        ttl_ms: u64,
    ) -> Result<bool, StorageError> {
        let expires_at = now + chrono::Duration::milliseconds(ttl_ms.min(i64::MAX as u64) as i64);
        let now_rfc3339 = now.to_rfc3339();
        let expires_rfc3339 = expires_at.to_rfc3339();

        let changes = self.conn.execute(
            "
            INSERT INTO reflector_runtime_leases (
                scope_id,
                owner_id,
                owner_pid,
                acquired_at,
                heartbeat_at,
                expires_at,
                metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, '{}')
            ON CONFLICT(scope_id) DO UPDATE SET
                owner_id=excluded.owner_id,
                owner_pid=excluded.owner_pid,
                acquired_at=CASE
                    WHEN reflector_runtime_leases.owner_id = excluded.owner_id
                    THEN reflector_runtime_leases.acquired_at
                    ELSE excluded.acquired_at
                END,
                heartbeat_at=excluded.heartbeat_at,
                expires_at=excluded.expires_at
            WHERE reflector_runtime_leases.owner_id = excluded.owner_id
               OR reflector_runtime_leases.expires_at <= excluded.acquired_at
            ",
            params![
                scope_id,
                owner_id,
                owner_pid,
                now_rfc3339,
                now_rfc3339,
                expires_rfc3339,
            ],
        )?;

        Ok(changes > 0)
    }

    pub fn heartbeat_reflector_lease(
        &self,
        scope_id: &str,
        owner_id: &str,
        now: DateTime<Utc>,
        ttl_ms: u64,
    ) -> Result<bool, StorageError> {
        let expires_at = now + chrono::Duration::milliseconds(ttl_ms.min(i64::MAX as u64) as i64);

        let changes = self.conn.execute(
            "
            UPDATE reflector_runtime_leases
            SET heartbeat_at = ?3,
                expires_at = ?4
            WHERE scope_id = ?1
              AND owner_id = ?2
            ",
            params![
                scope_id,
                owner_id,
                now.to_rfc3339(),
                expires_at.to_rfc3339(),
            ],
        )?;

        Ok(changes > 0)
    }

    pub fn release_reflector_lease(
        &self,
        scope_id: &str,
        owner_id: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "
            DELETE FROM reflector_runtime_leases
            WHERE scope_id = ?1
              AND owner_id = ?2
            ",
            params![scope_id, owner_id],
        )?;
        Ok(())
    }

    pub fn reflector_lease(&self, scope_id: &str) -> Result<Option<ReflectorLease>, StorageError> {
        let lease = self
            .conn
            .query_row(
                "
                SELECT scope_id, owner_id, owner_pid, acquired_at, heartbeat_at, expires_at
                FROM reflector_runtime_leases
                WHERE scope_id = ?1
                ",
                [scope_id],
                |row| {
                    Ok(ReflectorLease {
                        scope_id: row.get(0)?,
                        owner_id: row.get(1)?,
                        owner_pid: row.get(2)?,
                        acquired_at: parse_timestamp(row.get::<_, String>(3)?).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                3,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?,
                        heartbeat_at: parse_timestamp(row.get::<_, String>(4)?).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?,
                        expires_at: parse_timestamp(row.get::<_, String>(5)?).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                5,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?,
                    })
                },
            )
            .optional()?;

        Ok(lease)
    }

    pub fn enqueue_reflector_job(
        &self,
        active_tag: &str,
        observation_ids: &[String],
        conversation_ids: &[String],
        estimated_tokens: u32,
        now: DateTime<Utc>,
    ) -> Result<String, StorageError> {
        let mut observation_ids = observation_ids.to_vec();
        observation_ids.sort();
        observation_ids.dedup();

        let mut conversation_ids = conversation_ids.to_vec();
        conversation_ids.sort();
        conversation_ids.dedup();

        let payload_hash =
            canonical_payload_hash(&(active_tag, &observation_ids, &conversation_ids))
                .map_err(|err| StorageError::Serialization(err.to_string()))?;
        let job_id = format!("rfj:{}", &payload_hash[..16]);

        self.conn.execute(
            "
            INSERT OR IGNORE INTO reflector_jobs_t2 (
                job_id,
                active_tag,
                observation_ids_json,
                conversation_ids_json,
                estimated_tokens,
                status,
                claimed_by,
                claimed_at,
                attempts,
                last_error,
                created_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, NULL, ?7, ?7)
            ",
            params![
                job_id,
                active_tag,
                serde_json::to_string(&observation_ids)
                    .map_err(|err| StorageError::Serialization(err.to_string()))?,
                serde_json::to_string(&conversation_ids)
                    .map_err(|err| StorageError::Serialization(err.to_string()))?,
                i64::from(estimated_tokens),
                reflector_job_status_as_str(ReflectorJobStatus::Pending),
                now.to_rfc3339(),
            ],
        )?;

        Ok(job_id)
    }

    pub fn enqueue_t3_backlog_job(
        &self,
        project_root: &str,
        session_id: &str,
        pane_id: &str,
        active_tag: Option<&str>,
        slice_start_id: Option<&str>,
        slice_end_id: Option<&str>,
        artifact_refs: &[String],
        now: DateTime<Utc>,
    ) -> Result<(String, bool), StorageError> {
        let mut artifact_refs = artifact_refs.to_vec();
        artifact_refs.sort();
        artifact_refs.dedup();

        let payload_hash = canonical_payload_hash(&(
            project_root,
            session_id,
            pane_id,
            active_tag,
            slice_start_id,
            slice_end_id,
            &artifact_refs,
        ))
        .map_err(|err| StorageError::Serialization(err.to_string()))?;
        let job_id = format!("t3j:{}", &payload_hash[..16]);

        let changes = self.conn.execute(
            "
            INSERT OR IGNORE INTO t3_backlog_jobs (
                job_id,
                project_root,
                session_id,
                pane_id,
                active_tag,
                slice_start_id,
                slice_end_id,
                artifact_refs_json,
                status,
                attempts,
                last_error,
                claimed_by,
                claimed_at,
                created_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, NULL, NULL, NULL, ?10, ?10)
            ",
            params![
                job_id,
                project_root,
                session_id,
                pane_id,
                active_tag,
                slice_start_id,
                slice_end_id,
                serde_json::to_string(&artifact_refs)
                    .map_err(|err| StorageError::Serialization(err.to_string()))?,
                t3_backlog_job_status_as_str(T3BacklogJobStatus::Pending),
                now.to_rfc3339(),
            ],
        )?;

        Ok((job_id, changes > 0))
    }

    pub fn try_acquire_t3_runtime_lease(
        &self,
        scope_id: &str,
        owner_id: &str,
        owner_pid: Option<i64>,
        now: DateTime<Utc>,
        ttl_ms: u64,
    ) -> Result<bool, StorageError> {
        let ttl_ms = ttl_ms.max(1).min(i64::MAX as u64) as i64;
        let expires_at = now + chrono::Duration::milliseconds(ttl_ms);

        let changes = self.conn.execute(
            "
            INSERT INTO t3_runtime_leases (
                scope_id,
                owner_id,
                owner_pid,
                acquired_at,
                heartbeat_at,
                expires_at
            ) VALUES (?1, ?2, ?3, ?4, ?4, ?5)
            ON CONFLICT(scope_id) DO UPDATE SET
                owner_id = excluded.owner_id,
                owner_pid = excluded.owner_pid,
                acquired_at = CASE
                    WHEN t3_runtime_leases.owner_id = excluded.owner_id
                    THEN t3_runtime_leases.acquired_at
                    ELSE excluded.acquired_at
                END,
                heartbeat_at = excluded.heartbeat_at,
                expires_at = excluded.expires_at
            WHERE t3_runtime_leases.owner_id = excluded.owner_id
               OR t3_runtime_leases.expires_at <= excluded.acquired_at
            ",
            params![
                scope_id,
                owner_id,
                owner_pid,
                now.to_rfc3339(),
                expires_at.to_rfc3339(),
            ],
        )?;

        Ok(changes > 0)
    }

    pub fn heartbeat_t3_runtime_lease(
        &self,
        scope_id: &str,
        owner_id: &str,
        now: DateTime<Utc>,
        ttl_ms: u64,
    ) -> Result<bool, StorageError> {
        let ttl_ms = ttl_ms.max(1).min(i64::MAX as u64) as i64;
        let expires_at = now + chrono::Duration::milliseconds(ttl_ms);

        let changes = self.conn.execute(
            "
            UPDATE t3_runtime_leases
            SET heartbeat_at = ?3,
                expires_at = ?4
            WHERE scope_id = ?1
              AND owner_id = ?2
              AND expires_at >= ?3
            ",
            params![
                scope_id,
                owner_id,
                now.to_rfc3339(),
                expires_at.to_rfc3339(),
            ],
        )?;

        Ok(changes > 0)
    }

    pub fn release_t3_runtime_lease(
        &self,
        scope_id: &str,
        owner_id: &str,
    ) -> Result<bool, StorageError> {
        let changes = self.conn.execute(
            "
            DELETE FROM t3_runtime_leases
            WHERE scope_id = ?1
              AND owner_id = ?2
            ",
            params![scope_id, owner_id],
        )?;
        Ok(changes > 0)
    }

    pub fn t3_runtime_lease(&self, scope_id: &str) -> Result<Option<T3RuntimeLease>, StorageError> {
        let lease = self
            .conn
            .query_row(
                "
                SELECT scope_id, owner_id, owner_pid, acquired_at, heartbeat_at, expires_at
                FROM t3_runtime_leases
                WHERE scope_id = ?1
                ",
                [scope_id],
                |row| {
                    Ok(T3RuntimeLease {
                        scope_id: row.get(0)?,
                        owner_id: row.get(1)?,
                        owner_pid: row.get(2)?,
                        acquired_at: parse_timestamp(row.get::<_, String>(3)?).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                3,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?,
                        heartbeat_at: parse_timestamp(row.get::<_, String>(4)?).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?,
                        expires_at: parse_timestamp(row.get::<_, String>(5)?).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                5,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?,
                    })
                },
            )
            .optional()?;

        Ok(lease)
    }

    pub fn claim_next_t3_backlog_job(
        &self,
        scope_id: &str,
        owner_id: &str,
        now: DateTime<Utc>,
        stale_claim_after_ms: i64,
    ) -> Result<Option<T3BacklogJob>, StorageError> {
        let lease = self.t3_runtime_lease(scope_id)?;
        let Some(lease) = lease else {
            return Ok(None);
        };
        if lease.owner_id != owner_id || lease.expires_at < now {
            return Ok(None);
        }

        let stale_cutoff = now - chrono::Duration::milliseconds(stale_claim_after_ms.max(0));

        for _ in 0..4 {
            let candidate: Option<String> = self
                .conn
                .query_row(
                    "
                    SELECT job_id
                    FROM t3_backlog_jobs
                    WHERE status = ?1
                       OR (status = ?2 AND claimed_at IS NOT NULL AND claimed_at <= ?3)
                    ORDER BY CASE status WHEN ?1 THEN 0 ELSE 1 END, created_at ASC, job_id ASC
                    LIMIT 1
                    ",
                    params![
                        t3_backlog_job_status_as_str(T3BacklogJobStatus::Pending),
                        t3_backlog_job_status_as_str(T3BacklogJobStatus::Claimed),
                        stale_cutoff.to_rfc3339(),
                    ],
                    |row| row.get(0),
                )
                .optional()?;

            let Some(job_id) = candidate else {
                return Ok(None);
            };

            let changes = self.conn.execute(
                "
                UPDATE t3_backlog_jobs
                SET status = ?4,
                    claimed_by = ?2,
                    claimed_at = ?3,
                    attempts = attempts + 1,
                    updated_at = ?3
                WHERE job_id = ?1
                  AND (
                    status = ?5
                    OR (status = ?6 AND claimed_at IS NOT NULL AND claimed_at <= ?7)
                  )
                ",
                params![
                    job_id,
                    owner_id,
                    now.to_rfc3339(),
                    t3_backlog_job_status_as_str(T3BacklogJobStatus::Claimed),
                    t3_backlog_job_status_as_str(T3BacklogJobStatus::Pending),
                    t3_backlog_job_status_as_str(T3BacklogJobStatus::Claimed),
                    stale_cutoff.to_rfc3339(),
                ],
            )?;

            if changes == 0 {
                continue;
            }

            return self.t3_backlog_job_by_id(&job_id);
        }

        Ok(None)
    }

    pub fn complete_t3_backlog_job(
        &self,
        job_id: &str,
        owner_id: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, StorageError> {
        let changes = self.conn.execute(
            "
            UPDATE t3_backlog_jobs
            SET status = ?4,
                updated_at = ?3
            WHERE job_id = ?1
              AND claimed_by = ?2
              AND status = ?5
            ",
            params![
                job_id,
                owner_id,
                now.to_rfc3339(),
                t3_backlog_job_status_as_str(T3BacklogJobStatus::Completed),
                t3_backlog_job_status_as_str(T3BacklogJobStatus::Claimed),
            ],
        )?;

        Ok(changes > 0)
    }

    pub fn fail_t3_backlog_job(
        &self,
        job_id: &str,
        owner_id: &str,
        error: &str,
        now: DateTime<Utc>,
        requeue: bool,
        max_attempts: u16,
    ) -> Result<bool, StorageError> {
        let changes = self.conn.execute(
            "
            UPDATE t3_backlog_jobs
            SET status = CASE
                    WHEN ?9 = 1 AND attempts < ?10 THEN ?4
                    ELSE ?5
                END,
                claimed_by = CASE
                    WHEN ?9 = 1 AND attempts < ?10 THEN NULL
                    ELSE ?2
                END,
                claimed_at = CASE
                    WHEN ?9 = 1 AND attempts < ?10 THEN NULL
                    ELSE ?6
                END,
                last_error = ?3,
                updated_at = ?6
            WHERE job_id = ?1
              AND status = ?7
              AND claimed_by = ?8
            ",
            params![
                job_id,
                owner_id,
                error,
                t3_backlog_job_status_as_str(T3BacklogJobStatus::Pending),
                t3_backlog_job_status_as_str(T3BacklogJobStatus::Failed),
                now.to_rfc3339(),
                t3_backlog_job_status_as_str(T3BacklogJobStatus::Claimed),
                owner_id,
                if requeue { 1_i64 } else { 0_i64 },
                i64::from(max_attempts.max(1)),
            ],
        )?;

        Ok(changes > 0)
    }

    pub fn t3_backlog_job_by_id(&self, job_id: &str) -> Result<Option<T3BacklogJob>, StorageError> {
        let job = self
            .conn
            .query_row(
                "
                SELECT job_id, project_root, session_id, pane_id, active_tag,
                       slice_start_id, slice_end_id, artifact_refs_json,
                       status, attempts, last_error, claimed_by, claimed_at,
                       created_at, updated_at
                FROM t3_backlog_jobs
                WHERE job_id = ?1
                ",
                [job_id],
                parse_t3_backlog_job_row,
            )
            .optional()?;

        Ok(job)
    }

    pub fn t3_backlog_jobs_for_project_root(
        &self,
        project_root: &str,
    ) -> Result<Vec<T3BacklogJob>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT job_id, project_root, session_id, pane_id, active_tag,
                   slice_start_id, slice_end_id, artifact_refs_json,
                   status, attempts, last_error, claimed_by, claimed_at,
                   created_at, updated_at
            FROM t3_backlog_jobs
            WHERE project_root = ?1
            ORDER BY created_at DESC, job_id DESC
            ",
        )?;

        let rows = statement.query_map([project_root], parse_t3_backlog_job_row)?;
        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row?);
        }
        Ok(jobs)
    }

    pub fn project_watermark(
        &self,
        scope_key: &str,
    ) -> Result<Option<ProjectWatermark>, StorageError> {
        self.conn
            .query_row(
                "
                SELECT scope_key, last_artifact_ts, last_artifact_id, updated_at
                FROM project_watermarks
                WHERE scope_key = ?1
                ",
                [scope_key],
                |row| {
                    let last_artifact_ts = row
                        .get::<_, Option<String>>(1)?
                        .map(parse_timestamp)
                        .transpose()
                        .map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                1,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?;
                    let updated_at = parse_timestamp(row.get::<_, String>(3)?).map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?;
                    Ok(ProjectWatermark {
                        scope_key: row.get(0)?,
                        last_artifact_ts,
                        last_artifact_id: row.get(2)?,
                        updated_at,
                    })
                },
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn advance_project_watermark(
        &self,
        scope_key: &str,
        last_artifact_ts: Option<DateTime<Utc>>,
        last_artifact_id: Option<&str>,
        updated_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "
            INSERT INTO project_watermarks (
                scope_key,
                last_artifact_ts,
                last_artifact_id,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(scope_key) DO UPDATE SET
                last_artifact_ts = excluded.last_artifact_ts,
                last_artifact_id = excluded.last_artifact_id,
                updated_at = excluded.updated_at
            ",
            params![
                scope_key,
                last_artifact_ts.map(|value| value.to_rfc3339()),
                last_artifact_id,
                updated_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    pub fn latest_canon_revision(
        &self,
        entry_id: &str,
    ) -> Result<Option<CanonEntryRevision>, StorageError> {
        self.conn
            .query_row(
                "
                SELECT entry_id, revision, state, topic, summary, confidence_bps, freshness_score,
                       supersedes_entry_id, evidence_refs_json, created_at
                FROM project_canon_revisions
                WHERE entry_id = ?1
                ORDER BY revision DESC
                LIMIT 1
                ",
                [entry_id],
                parse_canon_entry_revision_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn upsert_canon_entry_revision(
        &self,
        entry_id: &str,
        topic: Option<&str>,
        summary: &str,
        confidence_bps: u16,
        freshness_score: u16,
        supersedes_entry_id: Option<&str>,
        evidence_refs: &[String],
        created_at: DateTime<Utc>,
    ) -> Result<CanonEntryRevision, StorageError> {
        let mut evidence_refs = evidence_refs.to_vec();
        evidence_refs.sort();
        evidence_refs.dedup();

        let latest = self.latest_canon_revision(entry_id)?;
        if let Some(latest) = latest.as_ref() {
            if latest.state == CanonRevisionState::Active
                && latest.topic.as_deref() == topic
                && latest.summary == summary
                && latest.confidence_bps == confidence_bps
                && latest.freshness_score == freshness_score
                && latest.supersedes_entry_id.as_deref() == supersedes_entry_id
                && latest.evidence_refs == evidence_refs
            {
                return Ok(latest.clone());
            }
        }

        let revision = latest.as_ref().map(|row| row.revision + 1).unwrap_or(1);
        let supersedes_entry_id = supersedes_entry_id
            .map(|value| value.to_string())
            .or_else(|| latest.as_ref().map(|_| entry_id.to_string()));

        self.conn.execute(
            "
            INSERT INTO project_canon_revisions (
                entry_id,
                revision,
                state,
                topic,
                summary,
                confidence_bps,
                freshness_score,
                supersedes_entry_id,
                evidence_refs_json,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                entry_id,
                revision,
                canon_revision_state_as_str(CanonRevisionState::Active),
                topic,
                summary,
                i64::from(confidence_bps),
                i64::from(freshness_score),
                supersedes_entry_id,
                serde_json::to_string(&evidence_refs)
                    .map_err(|err| StorageError::Serialization(err.to_string()))?,
                created_at.to_rfc3339(),
            ],
        )?;

        if revision > 1 {
            self.conn.execute(
                "
                UPDATE project_canon_revisions
                SET state = ?4
                WHERE entry_id = ?1
                  AND revision < ?2
                  AND state = ?3
                ",
                params![
                    entry_id,
                    revision,
                    canon_revision_state_as_str(CanonRevisionState::Active),
                    canon_revision_state_as_str(CanonRevisionState::Superseded),
                ],
            )?;
        }

        if let Some(superseded_entry) = supersedes_entry_id.as_deref() {
            if superseded_entry != entry_id {
                self.conn.execute(
                    "
                    UPDATE project_canon_revisions
                    SET state = ?2
                    WHERE entry_id = ?1
                      AND state = ?3
                    ",
                    params![
                        superseded_entry,
                        canon_revision_state_as_str(CanonRevisionState::Superseded),
                        canon_revision_state_as_str(CanonRevisionState::Active),
                    ],
                )?;
            }
        }

        self.latest_canon_revision(entry_id)?.ok_or_else(|| {
            StorageError::Serialization("canon revision insert did not produce a row".to_string())
        })
    }

    pub fn canon_entries_by_state(
        &self,
        state: CanonRevisionState,
        topic: Option<&str>,
    ) -> Result<Vec<CanonEntryRevision>, StorageError> {
        let mut statement = if topic.is_some() {
            self.conn.prepare(
                "
                SELECT entry_id, revision, state, topic, summary, confidence_bps, freshness_score,
                       supersedes_entry_id, evidence_refs_json, created_at
                FROM project_canon_revisions
                WHERE state = ?1 AND topic = ?2
                ORDER BY topic ASC, confidence_bps DESC, freshness_score DESC, created_at DESC,
                         entry_id ASC, revision DESC
                ",
            )?
        } else {
            self.conn.prepare(
                "
                SELECT entry_id, revision, state, topic, summary, confidence_bps, freshness_score,
                       supersedes_entry_id, evidence_refs_json, created_at
                FROM project_canon_revisions
                WHERE state = ?1
                ORDER BY topic ASC, confidence_bps DESC, freshness_score DESC, created_at DESC,
                         entry_id ASC, revision DESC
                ",
            )?
        };

        let rows = if let Some(topic) = topic {
            statement.query_map(
                params![canon_revision_state_as_str(state), topic],
                parse_canon_entry_revision_row,
            )?
        } else {
            statement.query_map(
                params![canon_revision_state_as_str(state)],
                parse_canon_entry_revision_row,
            )?
        };

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn active_canon_entries(
        &self,
        topic: Option<&str>,
    ) -> Result<Vec<CanonEntryRevision>, StorageError> {
        self.canon_entries_by_state(CanonRevisionState::Active, topic)
    }

    pub fn canon_entry_revisions(
        &self,
        entry_id: &str,
    ) -> Result<Vec<CanonEntryRevision>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT entry_id, revision, state, topic, summary, confidence_bps, freshness_score,
                   supersedes_entry_id, evidence_refs_json, created_at
            FROM project_canon_revisions
            WHERE entry_id = ?1
            ORDER BY revision DESC
            ",
        )?;

        let rows = statement.query_map([entry_id], parse_canon_entry_revision_row)?;
        let mut revisions = Vec::new();
        for row in rows {
            revisions.push(row?);
        }
        Ok(revisions)
    }

    pub fn mark_active_canon_entries_stale(
        &self,
        topic: Option<&str>,
        stale_before_or_equal: DateTime<Utc>,
        keep_entry_ids: &[String],
    ) -> Result<usize, StorageError> {
        let keep = keep_entry_ids.iter().cloned().collect::<BTreeSet<_>>();
        let active = self.active_canon_entries(topic)?;
        let mut marked = 0usize;

        for entry in active {
            if keep.contains(&entry.entry_id) || entry.created_at > stale_before_or_equal {
                continue;
            }

            let changes = self.conn.execute(
                "
                UPDATE project_canon_revisions
                SET state = ?3
                WHERE entry_id = ?1
                  AND revision = ?2
                  AND state = ?4
                ",
                params![
                    entry.entry_id,
                    entry.revision,
                    canon_revision_state_as_str(CanonRevisionState::Stale),
                    canon_revision_state_as_str(CanonRevisionState::Active),
                ],
            )?;
            marked += changes;
        }

        Ok(marked)
    }

    pub fn upsert_handshake_snapshot(
        &self,
        scope: &str,
        scope_key: &str,
        payload_text: &str,
        payload_hash: &str,
        token_estimate: u32,
        created_at: DateTime<Utc>,
    ) -> Result<(String, bool), StorageError> {
        let snapshot_id = format!(
            "hs:{}",
            &canonical_payload_hash(&(scope, scope_key, payload_hash))
                .map_err(|err| StorageError::Serialization(err.to_string()))?[..16]
        );

        let changes = self.conn.execute(
            "
            INSERT OR IGNORE INTO handshake_snapshots (
                snapshot_id,
                scope,
                scope_key,
                payload_text,
                payload_hash,
                token_estimate,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                snapshot_id,
                scope,
                scope_key,
                payload_text,
                payload_hash,
                i64::from(token_estimate),
                created_at.to_rfc3339(),
            ],
        )?;

        Ok((snapshot_id, changes > 0))
    }

    pub fn latest_handshake_snapshot(
        &self,
        scope: &str,
        scope_key: &str,
    ) -> Result<Option<HandshakeSnapshot>, StorageError> {
        self.conn
            .query_row(
                "
                SELECT snapshot_id, scope, scope_key, payload_text, payload_hash,
                       token_estimate, created_at
                FROM handshake_snapshots
                WHERE scope = ?1
                  AND scope_key = ?2
                ORDER BY created_at DESC, snapshot_id DESC
                LIMIT 1
                ",
                params![scope, scope_key],
                parse_handshake_snapshot_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn claim_next_reflector_job(
        &self,
        scope_id: &str,
        owner_id: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<ReflectorJob>, StorageError> {
        let lease = self.reflector_lease(scope_id)?;
        let Some(lease) = lease else {
            return Ok(None);
        };
        if lease.owner_id != owner_id || lease.expires_at < now {
            return Ok(None);
        }

        for _ in 0..4 {
            let candidate: Option<String> = self
                .conn
                .query_row(
                    "
                    SELECT job_id
                    FROM reflector_jobs_t2
                    WHERE status = ?1
                    ORDER BY created_at ASC, job_id ASC
                    LIMIT 1
                    ",
                    [reflector_job_status_as_str(ReflectorJobStatus::Pending)],
                    |row| row.get(0),
                )
                .optional()?;

            let Some(job_id) = candidate else {
                return Ok(None);
            };

            let changes = self.conn.execute(
                "
                UPDATE reflector_jobs_t2
                SET status = ?4,
                    claimed_by = ?2,
                    claimed_at = ?3,
                    attempts = attempts + 1,
                    updated_at = ?3
                WHERE job_id = ?1
                  AND status = ?5
                ",
                params![
                    job_id,
                    owner_id,
                    now.to_rfc3339(),
                    reflector_job_status_as_str(ReflectorJobStatus::Claimed),
                    reflector_job_status_as_str(ReflectorJobStatus::Pending),
                ],
            )?;

            if changes == 0 {
                continue;
            }

            let job = self.reflector_job_by_id(&job_id)?;
            return Ok(job);
        }

        Ok(None)
    }

    pub fn complete_reflector_job(
        &self,
        job_id: &str,
        owner_id: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, StorageError> {
        let changes = self.conn.execute(
            "
            UPDATE reflector_jobs_t2
            SET status = ?4,
                updated_at = ?3
            WHERE job_id = ?1
              AND claimed_by = ?2
              AND status = ?5
            ",
            params![
                job_id,
                owner_id,
                now.to_rfc3339(),
                reflector_job_status_as_str(ReflectorJobStatus::Completed),
                reflector_job_status_as_str(ReflectorJobStatus::Claimed),
            ],
        )?;

        Ok(changes > 0)
    }

    pub fn fail_reflector_job(
        &self,
        job_id: &str,
        owner_id: &str,
        error: &str,
        now: DateTime<Utc>,
        requeue: bool,
    ) -> Result<bool, StorageError> {
        let (status, claimed_by, claimed_at) = if requeue {
            (
                reflector_job_status_as_str(ReflectorJobStatus::Pending),
                None::<String>,
                None::<String>,
            )
        } else {
            (
                reflector_job_status_as_str(ReflectorJobStatus::Failed),
                Some(owner_id.to_string()),
                Some(now.to_rfc3339()),
            )
        };

        let changes = self.conn.execute(
            "
            UPDATE reflector_jobs_t2
            SET status = ?4,
                claimed_by = ?5,
                claimed_at = ?6,
                last_error = ?3,
                updated_at = ?2
            WHERE job_id = ?1
              AND status = ?7
              AND claimed_by = ?8
            ",
            params![
                job_id,
                now.to_rfc3339(),
                error,
                status,
                claimed_by,
                claimed_at,
                reflector_job_status_as_str(ReflectorJobStatus::Claimed),
                owner_id,
            ],
        )?;

        Ok(changes > 0)
    }

    pub fn pending_reflector_jobs(&self) -> Result<i64, StorageError> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM reflector_jobs_t2 WHERE status = ?1",
            [reflector_job_status_as_str(ReflectorJobStatus::Pending)],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn pending_t3_backlog_jobs(&self) -> Result<i64, StorageError> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM t3_backlog_jobs WHERE status = ?1",
            [t3_backlog_job_status_as_str(T3BacklogJobStatus::Pending)],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn upsert_detached_insight_job(
        &self,
        owner_plane: &str,
        worker_kind: Option<&str>,
        job: &InsightDetachedJob,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "
            INSERT INTO detached_insight_jobs (
                job_id, parent_job_id, owner_plane, worker_kind, mode, status, agent, team, chain_name,
                created_at_ms, started_at_ms, finished_at_ms, current_step_index, step_count,
                output_excerpt, stdout_excerpt, stderr_excerpt, error_text, fallback_used, step_results_json, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19, ?20, ?21
            )
            ON CONFLICT(job_id) DO UPDATE SET
                parent_job_id = excluded.parent_job_id,
                owner_plane = excluded.owner_plane,
                worker_kind = excluded.worker_kind,
                mode = excluded.mode,
                status = excluded.status,
                agent = excluded.agent,
                team = excluded.team,
                chain_name = excluded.chain_name,
                created_at_ms = excluded.created_at_ms,
                started_at_ms = excluded.started_at_ms,
                finished_at_ms = excluded.finished_at_ms,
                current_step_index = excluded.current_step_index,
                step_count = excluded.step_count,
                output_excerpt = excluded.output_excerpt,
                stdout_excerpt = excluded.stdout_excerpt,
                stderr_excerpt = excluded.stderr_excerpt,
                error_text = excluded.error_text,
                fallback_used = excluded.fallback_used,
                step_results_json = excluded.step_results_json,
                updated_at = excluded.updated_at
            ",
            params![
                job.job_id,
                job.parent_job_id,
                owner_plane.trim(),
                worker_kind.map(str::trim).filter(|value| !value.is_empty()),
                detached_mode_as_str(job.mode),
                detached_job_status_as_str(job.status),
                job.agent,
                job.team,
                job.chain,
                job.created_at_ms,
                job.started_at_ms,
                job.finished_at_ms,
                job.current_step_index.map(|value| value as i64),
                job.step_count.map(|value| value as i64),
                job.output_excerpt,
                job.stdout_excerpt,
                job.stderr_excerpt,
                job.error,
                if job.fallback_used { 1 } else { 0 },
                serde_json::to_string(&job.step_results)
                    .map_err(|err| StorageError::Serialization(err.to_string()))?,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn detached_insight_jobs(
        &self,
        owner_plane: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<InsightDetachedJob>, StorageError> {
        let mut sql = String::from(
            "SELECT job_id, parent_job_id, mode, status, agent, team, chain_name, created_at_ms, started_at_ms, finished_at_ms, current_step_index, step_count, output_excerpt, stdout_excerpt, stderr_excerpt, error_text, fallback_used, step_results_json FROM detached_insight_jobs",
        );
        let mut args = Vec::<String>::new();
        if let Some(owner_plane) = owner_plane.map(str::trim).filter(|value| !value.is_empty()) {
            sql.push_str(" WHERE owner_plane = ?1");
            args.push(owner_plane.to_string());
        }
        sql.push_str(" ORDER BY created_at_ms DESC, job_id DESC");
        if let Some(limit) = limit.filter(|limit| *limit > 0) {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        let mut statement = self.conn.prepare(&sql)?;
        let rows = statement.query_map(rusqlite::params_from_iter(args.iter()), |row| {
            parse_detached_insight_job_row(row)
        })?;
        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row?);
        }
        Ok(jobs)
    }

    pub fn mark_detached_insight_jobs_stale(
        &self,
        owner_plane: &str,
        reason: &str,
    ) -> Result<usize, StorageError> {
        let changed = self.conn.execute(
            "
            UPDATE detached_insight_jobs
            SET status = ?1,
                error_text = CASE
                    WHEN error_text IS NULL OR error_text = '' THEN ?2
                    ELSE error_text
                END,
                finished_at_ms = COALESCE(finished_at_ms, created_at_ms),
                updated_at = ?3
            WHERE owner_plane = ?4
              AND status IN (?5, ?6)
            ",
            params![
                detached_job_status_as_str(InsightDetachedJobStatus::Stale),
                reason,
                Utc::now().to_rfc3339(),
                owner_plane.trim(),
                detached_job_status_as_str(InsightDetachedJobStatus::Queued),
                detached_job_status_as_str(InsightDetachedJobStatus::Running),
            ],
        )?;
        Ok(changed)
    }

    pub fn reflector_job_by_id(&self, job_id: &str) -> Result<Option<ReflectorJob>, StorageError> {
        let job = self
            .conn
            .query_row(
                "
                SELECT job_id, active_tag, observation_ids_json, conversation_ids_json,
                       estimated_tokens, status, claimed_by, claimed_at, attempts,
                       last_error, created_at, updated_at
                FROM reflector_jobs_t2
                WHERE job_id = ?1
                ",
                [job_id],
                |row| parse_reflector_job_row(row),
            )
            .optional()?;

        Ok(job)
    }

    pub fn import_legacy_store(
        &self,
        legacy_path: impl AsRef<Path>,
    ) -> Result<LegacyImportReport, StorageError> {
        let legacy_path = legacy_path.as_ref();
        if !legacy_path.exists() {
            return Ok(LegacyImportReport::default());
        }

        let legacy_path = legacy_path.to_string_lossy().to_string();
        self.conn
            .execute("ATTACH DATABASE ?1 AS legacy_mind", [legacy_path.as_str()])?;

        let import_result = (|| {
            let mut report = LegacyImportReport::default();
            for spec in LEGACY_IMPORT_SPECS {
                report.tables_scanned += 1;
                if !self.attached_table_exists("legacy_mind", spec.table)? {
                    continue;
                }

                let statement = format!(
                    "INSERT OR IGNORE INTO {table} ({columns}) SELECT {columns} FROM legacy_mind.{table}",
                    table = spec.table,
                    columns = spec.columns,
                );
                let imported = self.conn.execute(&statement, [])?;
                if imported > 0 {
                    report.tables_imported += 1;
                    report.rows_imported += imported;
                }
            }
            Ok(report)
        })();

        let detach_result = self.conn.execute_batch("DETACH DATABASE legacy_mind");

        match (import_result, detach_result) {
            (Ok(report), Ok(())) => Ok(report),
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(StorageError::from(err)),
        }
    }

    fn attached_table_exists(&self, schema: &str, table_name: &str) -> Result<bool, StorageError> {
        let query = format!(
            "SELECT EXISTS(SELECT 1 FROM {schema}.sqlite_master WHERE type = 'table' AND name = ?1)",
            schema = schema,
        );
        let exists: i64 = self
            .conn
            .query_row(&query, [table_name], |row| row.get(0))?;
        Ok(exists != 0)
    }

    pub fn artifacts_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<StoredArtifact>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT artifact_id, conversation_id, ts, text, trace_ids_json, 't1' AS kind
            FROM observations_t1
            WHERE conversation_id = ?1
            UNION ALL
            SELECT artifact_id, conversation_id, ts, text, trace_ids_json, 't2' AS kind
            FROM reflections_t2
            WHERE conversation_id = ?1
            ORDER BY ts ASC, artifact_id ASC
            ",
        )?;

        let rows = statement.query_map([conversation_id], |row| {
            let ts = parse_timestamp(row.get::<_, String>(2)?).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;
            let trace_ids_json: String = row.get(4)?;
            let mut trace_ids: Vec<String> =
                serde_json::from_str(&trace_ids_json).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?;
            trace_ids.sort();
            trace_ids.dedup();

            Ok(StoredArtifact {
                artifact_id: row.get(0)?,
                conversation_id: row.get(1)?,
                ts,
                text: row.get(3)?,
                trace_ids,
                kind: row.get(5)?,
            })
        })?;

        let mut artifacts = Vec::new();
        for row in rows {
            artifacts.push(row?);
        }
        Ok(artifacts)
    }

    pub fn artifact_by_id(
        &self,
        artifact_id: &str,
    ) -> Result<Option<StoredArtifact>, StorageError> {
        self.conn
            .query_row(
                "
                SELECT artifact_id, conversation_id, ts, text, trace_ids_json, 't1' AS kind
                FROM observations_t1
                WHERE artifact_id = ?1
                UNION ALL
                SELECT artifact_id, conversation_id, ts, text, trace_ids_json, 't2' AS kind
                FROM reflections_t2
                WHERE artifact_id = ?1
                ORDER BY kind ASC
                LIMIT 1
                ",
                [artifact_id],
                |row| {
                    let ts = parse_timestamp(row.get::<_, String>(2)?).map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?;
                    let trace_ids_json: String = row.get(4)?;
                    let mut trace_ids: Vec<String> = serde_json::from_str(&trace_ids_json)
                        .map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?;
                    trace_ids.sort();
                    trace_ids.dedup();

                    Ok(StoredArtifact {
                        artifact_id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        ts,
                        text: row.get(3)?,
                        trace_ids,
                        kind: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn raw_event_count(&self, conversation_id: &str) -> Result<i64, StorageError> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM raw_events WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn t0_event_count(&self, conversation_id: &str) -> Result<i64, StorageError> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM compact_events_t0 WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn t0_compact_hashes(&self, conversation_id: &str) -> Result<Vec<String>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT compact_hash
            FROM compact_events_t0
            WHERE conversation_id = ?1
            ORDER BY ts, compact_id
            ",
        )?;

        let rows = statement.query_map([conversation_id], |row| row.get(0))?;
        let mut hashes = Vec::new();
        for row in rows {
            hashes.push(row?);
        }
        Ok(hashes)
    }

    pub fn t0_events_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<StoredCompactEvent>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT compact_id, conversation_id, ts, role, text, tool_meta_json, source_event_ids_json, policy_version
            FROM compact_events_t0
            WHERE conversation_id = ?1
            ORDER BY ts ASC, compact_id ASC
            ",
        )?;

        let rows = statement.query_map([conversation_id], |row| {
            let ts = parse_timestamp(row.get::<_, String>(2)?).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;

            let role = row
                .get::<_, Option<String>>(3)?
                .and_then(|value| parse_role(&value));
            let tool_meta_json: Option<String> = row.get(5)?;
            let tool_meta = if let Some(tool_meta_json) = tool_meta_json {
                Some(serde_json::from_str(&tool_meta_json).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?)
            } else {
                None
            };
            let source_ids_json: String = row.get(6)?;
            let mut source_event_ids: Vec<String> = serde_json::from_str(&source_ids_json)
                .map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        6,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?;
            source_event_ids.sort();
            source_event_ids.dedup();

            Ok(StoredCompactEvent {
                compact_id: row.get(0)?,
                conversation_id: row.get(1)?,
                ts,
                role,
                text: row.get(4)?,
                tool_meta,
                source_event_ids,
                policy_version: row.get(7)?,
            })
        })?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    pub fn upsert_artifact_task_link(&self, link: &ArtifactTaskLink) -> Result<(), StorageError> {
        let evidence_event_ids_json = serde_json::to_string(&link.evidence_event_ids)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;

        self.conn.execute(
            "
            INSERT INTO artifact_task_links (
                artifact_id,
                task_id,
                relation,
                confidence_bps,
                source,
                evidence_event_ids_json,
                start_ts,
                end_ts
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(artifact_id, task_id, relation) DO UPDATE SET
                confidence_bps=excluded.confidence_bps,
                source=excluded.source,
                evidence_event_ids_json=excluded.evidence_event_ids_json,
                start_ts=excluded.start_ts,
                end_ts=excluded.end_ts
            ",
            params![
                link.artifact_id,
                link.task_id,
                relation_as_str(link.relation),
                i64::from(link.confidence_bps),
                link.source,
                evidence_event_ids_json,
                link.start_ts.to_rfc3339(),
                link.end_ts.map(|value| value.to_rfc3339()),
            ],
        )?;

        Ok(())
    }

    pub fn artifact_task_links_for_artifact(
        &self,
        artifact_id: &str,
    ) -> Result<Vec<ArtifactTaskLink>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT artifact_id, task_id, relation, confidence_bps, source, evidence_event_ids_json, start_ts, end_ts
            FROM artifact_task_links
            WHERE artifact_id = ?1
            ORDER BY task_id ASC, relation ASC
            ",
        )?;

        let rows = statement.query_map([artifact_id], |row| {
            let relation = parse_relation(&row.get::<_, String>(2)?).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid relation",
                    )),
                )
            })?;
            let evidence_event_ids_json: String = row.get(5)?;
            let mut evidence_event_ids: Vec<String> =
                serde_json::from_str(&evidence_event_ids_json).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?;
            evidence_event_ids.sort();
            evidence_event_ids.dedup();
            let start_ts = parse_timestamp(row.get::<_, String>(6)?).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;
            let end_ts = row
                .get::<_, Option<String>>(7)?
                .map(parse_timestamp)
                .transpose()
                .map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        7,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?;

            Ok(ArtifactTaskLink {
                artifact_id: row.get(0)?,
                task_id: row.get(1)?,
                relation,
                confidence_bps: row.get::<_, i64>(3)? as u16,
                source: row.get(4)?,
                evidence_event_ids,
                start_ts,
                end_ts,
            })
        })?;

        let mut links = Vec::new();
        for row in rows {
            links.push(row?);
        }
        Ok(links)
    }

    pub fn replace_segment_route(&self, route: &SegmentRoute) -> Result<(), StorageError> {
        route
            .validate()
            .map_err(|err| StorageError::Serialization(err.to_string()))?;

        self.conn.execute(
            "DELETE FROM segment_routes WHERE artifact_id = ?1",
            [&route.artifact_id],
        )?;

        self.insert_segment_candidate(
            &route.artifact_id,
            &route.primary,
            route.routed_by,
            &route.reason,
            route.overridden_by.as_deref(),
            "primary",
        )?;

        for (index, candidate) in route.secondary.iter().enumerate() {
            let rank = format!("secondary:{}", index + 1);
            self.insert_segment_candidate(
                &route.artifact_id,
                candidate,
                route.routed_by,
                &route.reason,
                route.overridden_by.as_deref(),
                &rank,
            )?;
        }

        Ok(())
    }

    pub fn segment_route_for_artifact(
        &self,
        artifact_id: &str,
    ) -> Result<Option<SegmentRoute>, StorageError> {
        let mut statement = self.conn.prepare(
            "
            SELECT segment_id, confidence_bps, routed_by, reason, overridden_by
            FROM segment_routes
            WHERE artifact_id = ?1
            ORDER BY confidence_bps DESC, segment_id ASC
            ",
        )?;

        let rows = statement.query_map([artifact_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;

        let mut entries = Vec::new();
        for row in rows {
            let (segment_id, confidence_bps_i64, routed_by_raw, reason, overridden_by) = row?;
            if !(0..=10_000).contains(&confidence_bps_i64) {
                return Err(StorageError::Serialization(format!(
                    "invalid segment confidence: {confidence_bps_i64}"
                )));
            }
            let routed_by = parse_route_origin(&routed_by_raw).ok_or_else(|| {
                StorageError::Serialization(format!("invalid route origin: {routed_by_raw}"))
            })?;
            entries.push((
                SegmentCandidate {
                    segment_id,
                    confidence_bps: confidence_bps_i64 as u16,
                },
                routed_by,
                reason.unwrap_or_default(),
                overridden_by,
            ));
        }

        let Some((primary, routed_by, reason, overridden_by)) = entries.first().cloned() else {
            return Ok(None);
        };

        let secondary = entries
            .into_iter()
            .skip(1)
            .map(|(candidate, _, _, _)| candidate)
            .collect::<Vec<_>>();

        Ok(Some(SegmentRoute {
            artifact_id: artifact_id.to_string(),
            primary,
            secondary,
            routed_by,
            reason: strip_route_rank_suffix(&reason),
            overridden_by,
        }))
    }

    fn insert_segment_candidate(
        &self,
        artifact_id: &str,
        candidate: &SegmentCandidate,
        routed_by: RouteOrigin,
        reason: &str,
        overridden_by: Option<&str>,
        rank: &str,
    ) -> Result<(), StorageError> {
        let reason = format!("{} | rank={rank}", reason.trim());
        self.conn.execute(
            "
            INSERT INTO segment_routes (
                artifact_id,
                segment_id,
                confidence_bps,
                routed_by,
                reason,
                overridden_by
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(artifact_id, segment_id) DO UPDATE SET
                confidence_bps=excluded.confidence_bps,
                routed_by=excluded.routed_by,
                reason=excluded.reason,
                overridden_by=excluded.overridden_by
            ",
            params![
                artifact_id,
                candidate.segment_id,
                i64::from(candidate.confidence_bps),
                route_origin_as_str(routed_by),
                reason,
                overridden_by,
            ],
        )?;
        Ok(())
    }

    pub fn has_raw_event(&self, event_id: &str) -> Result<bool, StorageError> {
        let found = self
            .conn
            .query_row(
                "SELECT 1 FROM raw_events WHERE event_id = ?1 LIMIT 1",
                [event_id],
                |_| Ok(()),
            )
            .optional()?;
        Ok(found.is_some())
    }

    pub fn raw_event_by_id(&self, event_id: &str) -> Result<Option<RawEvent>, StorageError> {
        self.conn
            .query_row(
                "
                SELECT event_id, conversation_id, agent_id, ts, payload_json, attrs_json
                FROM raw_events
                WHERE event_id = ?1
                LIMIT 1
                ",
                [event_id],
                |row| {
                    let ts = parse_timestamp(row.get::<_, String>(3)?).map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?;
                    let body: RawEventBody = serde_json::from_str(&row.get::<_, String>(4)?)
                        .map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?;
                    let attrs = serde_json::from_str::<
                        std::collections::BTreeMap<String, serde_json::Value>,
                    >(&row.get::<_, String>(5)?)
                    .map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            5,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?;

                    Ok(RawEvent {
                        event_id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        agent_id: row.get(2)?,
                        ts,
                        body,
                        attrs,
                    })
                },
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn compact_source_event_ids(&self, compact_id: &str) -> Result<Vec<String>, StorageError> {
        let source_json: Option<String> = self
            .conn
            .query_row(
                "SELECT source_event_ids_json FROM compact_events_t0 WHERE compact_id = ?1",
                [compact_id],
                |row| row.get(0),
            )
            .optional()?;

        let Some(source_json) = source_json else {
            return Ok(Vec::new());
        };

        serde_json::from_str(&source_json)
            .map_err(|err| StorageError::Serialization(err.to_string()))
    }

    pub fn table_exists(&self, table_name: &str) -> Result<bool, StorageError> {
        let exists = self
            .conn
            .query_row(
                "
                SELECT 1
                FROM sqlite_master
                WHERE type='table' AND name = ?1
                LIMIT 1
                ",
                [table_name],
                |_| Ok(()),
            )
            .optional()?;
        Ok(exists.is_some())
    }
}

fn role_as_str(role: aoc_core::mind_contracts::ConversationRole) -> &'static str {
    match role {
        aoc_core::mind_contracts::ConversationRole::System => "system",
        aoc_core::mind_contracts::ConversationRole::User => "user",
        aoc_core::mind_contracts::ConversationRole::Assistant => "assistant",
        aoc_core::mind_contracts::ConversationRole::Tool => "tool",
    }
}

fn parse_role(value: &str) -> Option<ConversationRole> {
    match value {
        "system" => Some(ConversationRole::System),
        "user" => Some(ConversationRole::User),
        "assistant" => Some(ConversationRole::Assistant),
        "tool" => Some(ConversationRole::Tool),
        _ => None,
    }
}

fn relation_as_str(relation: ArtifactTaskRelation) -> &'static str {
    match relation {
        ArtifactTaskRelation::Active => "active",
        ArtifactTaskRelation::WorkedOn => "worked_on",
        ArtifactTaskRelation::Mentioned => "mentioned",
        ArtifactTaskRelation::Completed => "completed",
    }
}

fn parse_relation(value: &str) -> Option<ArtifactTaskRelation> {
    match value {
        "active" => Some(ArtifactTaskRelation::Active),
        "worked_on" => Some(ArtifactTaskRelation::WorkedOn),
        "mentioned" => Some(ArtifactTaskRelation::Mentioned),
        "completed" => Some(ArtifactTaskRelation::Completed),
        _ => None,
    }
}

fn route_origin_as_str(origin: RouteOrigin) -> &'static str {
    match origin {
        RouteOrigin::Taskmaster => "taskmaster",
        RouteOrigin::Heuristic => "heuristic",
        RouteOrigin::ManualOverride => "manual_override",
    }
}

fn parse_route_origin(value: &str) -> Option<RouteOrigin> {
    match value {
        "taskmaster" => Some(RouteOrigin::Taskmaster),
        "heuristic" => Some(RouteOrigin::Heuristic),
        "manual_override" => Some(RouteOrigin::ManualOverride),
        _ => None,
    }
}

fn semantic_stage_as_str(stage: SemanticStage) -> &'static str {
    match stage {
        SemanticStage::T1Observer => "t1_observer",
        SemanticStage::T2Reflector => "t2_reflector",
    }
}

fn parse_semantic_stage(value: &str) -> Option<SemanticStage> {
    match value {
        "t1_observer" => Some(SemanticStage::T1Observer),
        "t2_reflector" => Some(SemanticStage::T2Reflector),
        _ => None,
    }
}

fn semantic_runtime_as_str(runtime: SemanticRuntime) -> &'static str {
    runtime.as_str()
}

fn parse_semantic_runtime(value: &str) -> Option<SemanticRuntime> {
    match value {
        "deterministic" => Some(SemanticRuntime::Deterministic),
        "pi-semantic" => Some(SemanticRuntime::PiSemantic),
        "external-semantic" => Some(SemanticRuntime::ExternalSemantic),
        _ => None,
    }
}

fn semantic_failure_kind_as_str(kind: SemanticFailureKind) -> &'static str {
    kind.as_str()
}

fn parse_semantic_failure_kind(value: &str) -> Option<SemanticFailureKind> {
    match value {
        "timeout" => Some(SemanticFailureKind::Timeout),
        "invalid_output" => Some(SemanticFailureKind::InvalidOutput),
        "budget_exceeded" => Some(SemanticFailureKind::BudgetExceeded),
        "provider_error" => Some(SemanticFailureKind::ProviderError),
        "lock_conflict" => Some(SemanticFailureKind::LockConflict),
        _ => None,
    }
}

struct LegacyImportSpec {
    table: &'static str,
    columns: &'static str,
}

const LEGACY_IMPORT_SPECS: &[LegacyImportSpec] = &[
    LegacyImportSpec {
        table: "raw_events",
        columns: "event_id, conversation_id, agent_id, ts, kind, payload_json, attrs_json",
    },
    LegacyImportSpec {
        table: "compact_events_t0",
        columns: "compact_id, compact_hash, schema_version, conversation_id, ts, role, text, snippet, source_event_ids_json, tool_meta_json, policy_version",
    },
    LegacyImportSpec {
        table: "observations_t1",
        columns: "artifact_id, conversation_id, ts, importance, text, trace_ids_json",
    },
    LegacyImportSpec {
        table: "reflections_t2",
        columns: "artifact_id, conversation_id, ts, text, trace_ids_json",
    },
    LegacyImportSpec {
        table: "artifact_task_links",
        columns: "artifact_id, task_id, relation, confidence_bps, source, evidence_event_ids_json, start_ts, end_ts",
    },
    LegacyImportSpec {
        table: "conversation_context_state",
        columns: "conversation_id, ts, active_tag, active_tasks_json, lifecycle, signal_task_ids_json, signal_source",
    },
    LegacyImportSpec {
        table: "segment_routes",
        columns: "artifact_id, segment_id, confidence_bps, routed_by, reason, overridden_by",
    },
    LegacyImportSpec {
        table: "aoc_mem_decisions",
        columns: "decision_id, ts, project_id, segment_id, text, supersedes_id",
    },
    LegacyImportSpec {
        table: "ingestion_checkpoints",
        columns: "conversation_id, raw_cursor, t0_cursor, policy_version, updated_at",
    },
    LegacyImportSpec {
        table: "semantic_runtime_provenance",
        columns: "artifact_id, stage, runtime, provider_name, model_id, prompt_version, input_hash, output_hash, latency_ms, attempt_count, fallback_used, fallback_reason, failure_kind, created_at",
    },
    LegacyImportSpec {
        table: "compaction_checkpoints",
        columns: "checkpoint_id, conversation_id, session_id, ts, trigger_source, reason, summary, tokens_before, first_kept_entry_id, compaction_entry_id, from_extension, marker_event_id, schema_version, created_at, updated_at",
    },
    LegacyImportSpec {
        table: "compaction_slices_t0",
        columns: "slice_id, slice_hash, schema_version, conversation_id, session_id, ts, trigger_source, reason, summary, tokens_before, first_kept_entry_id, compaction_entry_id, from_extension, source_kind, source_event_ids_json, read_files_json, modified_files_json, checkpoint_id, policy_version",
    },
    LegacyImportSpec {
        table: "reflector_runtime_leases",
        columns: "scope_id, owner_id, owner_pid, acquired_at, heartbeat_at, expires_at, metadata_json",
    },
    LegacyImportSpec {
        table: "reflector_jobs_t2",
        columns: "job_id, active_tag, observation_ids_json, conversation_ids_json, estimated_tokens, status, claimed_by, claimed_at, attempts, last_error, created_at, updated_at",
    },
    LegacyImportSpec {
        table: "conversation_lineage",
        columns:
            "conversation_id, session_id, parent_conversation_id, root_conversation_id, updated_at",
    },
];

fn reflector_job_status_as_str(status: ReflectorJobStatus) -> &'static str {
    match status {
        ReflectorJobStatus::Pending => "pending",
        ReflectorJobStatus::Claimed => "claimed",
        ReflectorJobStatus::Completed => "completed",
        ReflectorJobStatus::Failed => "failed",
    }
}

fn t3_backlog_job_status_as_str(status: T3BacklogJobStatus) -> &'static str {
    match status {
        T3BacklogJobStatus::Pending => "pending",
        T3BacklogJobStatus::Claimed => "claimed",
        T3BacklogJobStatus::Completed => "completed",
        T3BacklogJobStatus::Failed => "failed",
    }
}

fn parse_reflector_job_status(value: &str) -> Option<ReflectorJobStatus> {
    match value {
        "pending" => Some(ReflectorJobStatus::Pending),
        "claimed" => Some(ReflectorJobStatus::Claimed),
        "completed" => Some(ReflectorJobStatus::Completed),
        "failed" => Some(ReflectorJobStatus::Failed),
        _ => None,
    }
}

fn parse_t3_backlog_job_status(value: &str) -> Option<T3BacklogJobStatus> {
    match value {
        "pending" => Some(T3BacklogJobStatus::Pending),
        "claimed" => Some(T3BacklogJobStatus::Claimed),
        "completed" => Some(T3BacklogJobStatus::Completed),
        "failed" => Some(T3BacklogJobStatus::Failed),
        _ => None,
    }
}

fn detached_mode_as_str(mode: InsightDetachedMode) -> &'static str {
    match mode {
        InsightDetachedMode::Dispatch => "dispatch",
        InsightDetachedMode::Chain => "chain",
        InsightDetachedMode::Parallel => "parallel",
    }
}

fn parse_detached_mode(value: &str) -> Option<InsightDetachedMode> {
    match value.trim() {
        "dispatch" => Some(InsightDetachedMode::Dispatch),
        "chain" => Some(InsightDetachedMode::Chain),
        "parallel" => Some(InsightDetachedMode::Parallel),
        _ => None,
    }
}

fn detached_job_status_as_str(status: InsightDetachedJobStatus) -> &'static str {
    match status {
        InsightDetachedJobStatus::Queued => "queued",
        InsightDetachedJobStatus::Running => "running",
        InsightDetachedJobStatus::Success => "success",
        InsightDetachedJobStatus::Fallback => "fallback",
        InsightDetachedJobStatus::Error => "error",
        InsightDetachedJobStatus::Cancelled => "cancelled",
        InsightDetachedJobStatus::Stale => "stale",
    }
}

fn parse_detached_job_status(value: &str) -> Option<InsightDetachedJobStatus> {
    match value.trim() {
        "queued" => Some(InsightDetachedJobStatus::Queued),
        "running" => Some(InsightDetachedJobStatus::Running),
        "success" => Some(InsightDetachedJobStatus::Success),
        "fallback" => Some(InsightDetachedJobStatus::Fallback),
        "error" => Some(InsightDetachedJobStatus::Error),
        "cancelled" => Some(InsightDetachedJobStatus::Cancelled),
        "stale" => Some(InsightDetachedJobStatus::Stale),
        _ => None,
    }
}

fn canon_revision_state_as_str(state: CanonRevisionState) -> &'static str {
    match state {
        CanonRevisionState::Active => "active",
        CanonRevisionState::Superseded => "superseded",
        CanonRevisionState::Stale => "stale",
    }
}

fn parse_detached_insight_job_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<InsightDetachedJob> {
    let mode_raw: String = row.get(2)?;
    let mode = parse_detached_mode(&mode_raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid detached insight mode: {mode_raw}"),
            )),
        )
    })?;
    let status_raw: String = row.get(3)?;
    let status = parse_detached_job_status(&status_raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid detached insight status: {status_raw}"),
            )),
        )
    })?;
    let step_results_json = row.get::<_, Option<String>>(17)?;
    let step_results = step_results_json
        .as_deref()
        .map(|json| serde_json::from_str::<Vec<InsightDispatchStepResult>>(json))
        .transpose()
        .map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(17, rusqlite::types::Type::Text, Box::new(err))
        })?
        .unwrap_or_default();
    Ok(InsightDetachedJob {
        job_id: row.get(0)?,
        parent_job_id: row.get(1)?,
        mode,
        status,
        agent: row.get(4)?,
        team: row.get(5)?,
        chain: row.get(6)?,
        created_at_ms: row.get(7)?,
        started_at_ms: row.get(8)?,
        finished_at_ms: row.get(9)?,
        current_step_index: row.get::<_, Option<i64>>(10)?.map(|value| value as usize),
        step_count: row.get::<_, Option<i64>>(11)?.map(|value| value as usize),
        output_excerpt: row.get(12)?,
        stdout_excerpt: row.get(13)?,
        stderr_excerpt: row.get(14)?,
        error: row.get(15)?,
        fallback_used: row.get::<_, i64>(16)? != 0,
        step_results,
    })
}

fn parse_canon_revision_state(value: &str) -> Option<CanonRevisionState> {
    match value {
        "active" => Some(CanonRevisionState::Active),
        "superseded" => Some(CanonRevisionState::Superseded),
        "stale" => Some(CanonRevisionState::Stale),
        _ => None,
    }
}

fn parse_canon_entry_revision_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonEntryRevision> {
    let state_raw: String = row.get(2)?;
    let state = parse_canon_revision_state(&state_raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid canon state: {state_raw}"),
            )),
        )
    })?;

    let evidence_refs_json: String = row.get(8)?;
    let mut evidence_refs: Vec<String> =
        serde_json::from_str(&evidence_refs_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(err))
        })?;
    evidence_refs.sort();
    evidence_refs.dedup();

    let created_at = parse_timestamp(row.get::<_, String>(9)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(CanonEntryRevision {
        entry_id: row.get(0)?,
        revision: row.get(1)?,
        state,
        topic: row.get(3)?,
        summary: row.get(4)?,
        confidence_bps: row.get::<_, i64>(5)? as u16,
        freshness_score: row.get::<_, i64>(6)? as u16,
        supersedes_entry_id: row.get(7)?,
        evidence_refs,
        created_at,
    })
}

fn parse_handshake_snapshot_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HandshakeSnapshot> {
    let created_at = parse_timestamp(row.get::<_, String>(6)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(HandshakeSnapshot {
        snapshot_id: row.get(0)?,
        scope: row.get(1)?,
        scope_key: row.get(2)?,
        payload_text: row.get(3)?,
        payload_hash: row.get(4)?,
        token_estimate: row.get::<_, i64>(5)? as u32,
        created_at,
    })
}

fn parse_artifact_file_link_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactFileLink> {
    let created_at = parse_timestamp(row.get::<_, String>(8)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let updated_at = parse_timestamp(row.get::<_, String>(9)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(ArtifactFileLink {
        artifact_id: row.get(0)?,
        path: row.get(1)?,
        relation: row.get(2)?,
        source: row.get(3)?,
        additions: row
            .get::<_, Option<i64>>(4)?
            .map(|value| value.max(0) as u32),
        deletions: row
            .get::<_, Option<i64>>(5)?
            .map(|value| value.max(0) as u32),
        staged: row.get::<_, i64>(6)? != 0,
        untracked: row.get::<_, i64>(7)? != 0,
        created_at,
        updated_at,
    })
}

fn parse_compaction_t0_slice_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredCompactionT0Slice> {
    let ts = parse_timestamp(row.get::<_, String>(5)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let source_event_ids_json: String = row.get(14)?;
    let mut source_event_ids: Vec<String> =
        serde_json::from_str(&source_event_ids_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                14,
                rusqlite::types::Type::Text,
                Box::new(err),
            )
        })?;
    source_event_ids.sort();
    source_event_ids.dedup();

    let read_files_json: String = row.get(15)?;
    let mut read_files: Vec<String> = serde_json::from_str(&read_files_json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(15, rusqlite::types::Type::Text, Box::new(err))
    })?;
    read_files.sort();
    read_files.dedup();

    let modified_files_json: String = row.get(16)?;
    let mut modified_files: Vec<String> =
        serde_json::from_str(&modified_files_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                16,
                rusqlite::types::Type::Text,
                Box::new(err),
            )
        })?;
    modified_files.sort();
    modified_files.dedup();

    Ok(StoredCompactionT0Slice {
        slice_id: row.get(0)?,
        slice_hash: row.get(1)?,
        schema_version: row.get::<_, i64>(2)?.max(0) as u32,
        conversation_id: row.get(3)?,
        session_id: row.get(4)?,
        ts,
        trigger_source: row.get(6)?,
        reason: row.get(7)?,
        summary: row.get(8)?,
        tokens_before: row
            .get::<_, Option<i64>>(9)?
            .map(|value| value.max(0) as u32),
        first_kept_entry_id: row.get(10)?,
        compaction_entry_id: row.get(11)?,
        from_extension: row.get::<_, i64>(12)? != 0,
        source_kind: row.get(13)?,
        source_event_ids,
        read_files,
        modified_files,
        checkpoint_id: row.get(17)?,
        policy_version: row.get(18)?,
    })
}

fn parse_compaction_checkpoint_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CompactionCheckpoint> {
    let ts = parse_timestamp(row.get::<_, String>(3)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let created_at = parse_timestamp(row.get::<_, String>(13)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(13, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let updated_at = parse_timestamp(row.get::<_, String>(14)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(14, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(CompactionCheckpoint {
        checkpoint_id: row.get(0)?,
        conversation_id: row.get(1)?,
        session_id: row.get(2)?,
        ts,
        trigger_source: row.get(4)?,
        reason: row.get(5)?,
        summary: row.get(6)?,
        tokens_before: row.get::<_, Option<i64>>(7)?.map(|value| value as u32),
        first_kept_entry_id: row.get(8)?,
        compaction_entry_id: row.get(9)?,
        from_extension: row.get::<_, i64>(10)? != 0,
        marker_event_id: row.get(11)?,
        schema_version: row.get::<_, i64>(12)? as u32,
        created_at,
        updated_at,
    })
}

fn parse_t3_backlog_job_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<T3BacklogJob> {
    let artifact_refs_json: String = row.get(7)?;
    let mut artifact_refs: Vec<String> =
        serde_json::from_str(&artifact_refs_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(err))
        })?;
    artifact_refs.sort();
    artifact_refs.dedup();

    let status_raw: String = row.get(8)?;
    let status = parse_t3_backlog_job_status(&status_raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            8,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid t3 backlog status: {status_raw}"),
            )),
        )
    })?;

    let claimed_at = row
        .get::<_, Option<String>>(12)?
        .map(parse_timestamp)
        .transpose()
        .map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                12,
                rusqlite::types::Type::Text,
                Box::new(err),
            )
        })?;

    let created_at = parse_timestamp(row.get::<_, String>(13)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(13, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let updated_at = parse_timestamp(row.get::<_, String>(14)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(14, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(T3BacklogJob {
        job_id: row.get(0)?,
        project_root: row.get(1)?,
        session_id: row.get(2)?,
        pane_id: row.get(3)?,
        active_tag: row.get(4)?,
        slice_start_id: row.get(5)?,
        slice_end_id: row.get(6)?,
        artifact_refs,
        status,
        attempts: row.get::<_, i64>(9)? as u16,
        last_error: row.get(10)?,
        claimed_by: row.get(11)?,
        claimed_at,
        created_at,
        updated_at,
    })
}

fn parse_reflector_job_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReflectorJob> {
    let observation_ids_json: String = row.get(2)?;
    let mut observation_ids: Vec<String> =
        serde_json::from_str(&observation_ids_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(err))
        })?;
    observation_ids.sort();
    observation_ids.dedup();

    let conversation_ids_json: String = row.get(3)?;
    let mut conversation_ids: Vec<String> =
        serde_json::from_str(&conversation_ids_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(err))
        })?;
    conversation_ids.sort();
    conversation_ids.dedup();

    let status_raw: String = row.get(5)?;
    let status = parse_reflector_job_status(&status_raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            5,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid reflector job status: {status_raw}"),
            )),
        )
    })?;

    let claimed_at = row
        .get::<_, Option<String>>(7)?
        .map(parse_timestamp)
        .transpose()
        .map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(err))
        })?;

    let created_at = parse_timestamp(row.get::<_, String>(10)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let updated_at = parse_timestamp(row.get::<_, String>(11)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(ReflectorJob {
        job_id: row.get(0)?,
        active_tag: row.get(1)?,
        observation_ids,
        conversation_ids,
        estimated_tokens: row.get::<_, i64>(4)? as u32,
        status,
        claimed_by: row.get(6)?,
        claimed_at,
        attempts: row.get::<_, i64>(8)? as u16,
        last_error: row.get(9)?,
        created_at,
        updated_at,
    })
}

fn strip_route_rank_suffix(reason: &str) -> String {
    reason
        .split(" | rank=")
        .next()
        .unwrap_or(reason)
        .trim()
        .to_string()
}

fn parse_timestamp(value: String) -> Result<DateTime<Utc>, StorageError> {
    DateTime::parse_from_rfc3339(&value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|err| StorageError::Timestamp(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aoc_core::mind_contracts::{
        build_compaction_t0_slice, compact_raw_event_to_t0, ConversationRole, MessageEvent,
        RawEvent, RawEventBody, T0CompactionPolicy, ToolExecutionStatus, ToolResultEvent,
    };
    use chrono::TimeZone;
    use rusqlite::{params, Connection};
    use tempfile::NamedTempFile;

    fn ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 23, 14, 0, 0)
            .single()
            .expect("valid timestamp")
    }

    fn sample_message_event(event_id: &str, conversation_id: &str) -> RawEvent {
        RawEvent {
            event_id: event_id.to_string(),
            conversation_id: conversation_id.to_string(),
            agent_id: "agent-1".to_string(),
            ts: ts(),
            body: RawEventBody::Message(MessageEvent {
                role: ConversationRole::User,
                text: "build contracts".to_string(),
            }),
            attrs: Default::default(),
        }
    }

    fn sample_tool_event(event_id: &str, conversation_id: &str) -> RawEvent {
        RawEvent {
            event_id: event_id.to_string(),
            conversation_id: conversation_id.to_string(),
            agent_id: "agent-1".to_string(),
            ts: ts(),
            body: RawEventBody::ToolResult(ToolResultEvent {
                tool_name: "bash".to_string(),
                status: ToolExecutionStatus::Success,
                latency_ms: Some(45),
                exit_code: Some(0),
                output: Some("output body".to_string()),
                redacted: false,
            }),
            attrs: Default::default(),
        }
    }

    #[test]
    fn migration_creates_mind_tables() {
        let db = MindStore::open_in_memory().expect("open db");

        for table in [
            "raw_events",
            "compact_events_t0",
            "observations_t1",
            "reflections_t2",
            "artifact_task_links",
            "conversation_context_state",
            "segment_routes",
            "semantic_runtime_provenance",
            "reflector_runtime_leases",
            "reflector_jobs_t2",
            "conversation_lineage",
            "aoc_mem_decisions",
            "ingestion_checkpoints",
            "t3_backlog_jobs",
            "t3_runtime_leases",
            "project_canon_revisions",
            "handshake_snapshots",
            "project_watermarks",
            "compaction_slices_t0",
            "detached_insight_jobs",
        ] {
            assert!(db.table_exists(table).expect("table check"));
        }

        assert_eq!(
            db.schema_version().expect("schema version"),
            MIND_SCHEMA_VERSION
        );
    }

    #[test]
    fn query_roundtrip_for_raw_and_t0_and_checkpoint() {
        let db = MindStore::open_in_memory().expect("open db");
        let raw = sample_message_event("evt-1", "conv-1");

        assert!(db.insert_raw_event(&raw).expect("insert raw"));
        assert!(!db.insert_raw_event(&raw).expect("idempotent insert raw"));

        let compact = compact_raw_event_to_t0(&raw, &T0CompactionPolicy::default())
            .expect("compact ok")
            .expect("message should compact");
        db.upsert_t0_compact_event(&compact)
            .expect("upsert compact");

        let checkpoint = IngestionCheckpoint {
            conversation_id: "conv-1".to_string(),
            raw_cursor: 44,
            t0_cursor: 44,
            policy_version: "t0.v1".to_string(),
            updated_at: ts(),
        };
        db.upsert_checkpoint(&checkpoint)
            .expect("upsert checkpoint");

        assert_eq!(db.raw_event_count("conv-1").expect("count raw"), 1);
        assert_eq!(db.t0_event_count("conv-1").expect("count t0"), 1);

        let loaded_checkpoint = db
            .checkpoint("conv-1")
            .expect("checkpoint query")
            .expect("checkpoint present");
        assert_eq!(loaded_checkpoint.raw_cursor, 44);
        assert_eq!(loaded_checkpoint.t0_cursor, 44);
        assert_eq!(loaded_checkpoint.policy_version, "t0.v1");
    }

    #[test]
    fn replay_stability_keeps_same_t0_hash_for_same_policy() {
        let file = NamedTempFile::new().expect("temp db");
        let db = MindStore::open(file.path()).expect("open db");
        let raw = sample_tool_event("evt-2", "conv-2");

        let mut policy = T0CompactionPolicy::default();
        policy.tool_snippet_allowlist.insert("bash".to_string(), 6);

        db.insert_raw_event(&raw).expect("insert raw");
        let compact_a = compact_raw_event_to_t0(&raw, &policy)
            .expect("compact")
            .expect("tool should compact");
        db.upsert_t0_compact_event(&compact_a).expect("upsert a");

        let compact_b = compact_raw_event_to_t0(&raw, &policy)
            .expect("compact")
            .expect("tool should compact");
        db.upsert_t0_compact_event(&compact_b).expect("upsert b");

        let hashes = db.t0_compact_hashes("conv-2").expect("hashes");
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes[0], compact_a.compact_hash);
        assert_eq!(hashes[0], compact_b.compact_hash);
    }

    #[test]
    fn provenance_links_from_t0_back_to_raw_events() {
        let db = MindStore::open_in_memory().expect("open db");
        let raw = sample_tool_event("evt-3", "conv-3");
        db.insert_raw_event(&raw).expect("insert raw");

        let compact = compact_raw_event_to_t0(&raw, &T0CompactionPolicy::default())
            .expect("compact")
            .expect("tool should compact");
        db.upsert_t0_compact_event(&compact)
            .expect("upsert compact");

        let source_ids = db
            .compact_source_event_ids(&compact.compact_id)
            .expect("source ids");
        assert_eq!(source_ids, vec!["evt-3".to_string()]);

        for source_id in source_ids {
            assert!(db.has_raw_event(&source_id).expect("raw event exists"));
        }
    }

    #[test]
    fn context_state_roundtrip_preserves_sorted_task_set() {
        let db = MindStore::open_in_memory().expect("open db");
        db.append_context_state(&ConversationContextState {
            conversation_id: "conv-4".to_string(),
            ts: ts(),
            active_tag: Some("mind".to_string()),
            active_tasks: vec!["102".to_string(), "101".to_string(), "101".to_string()],
            lifecycle: Some("in-progress".to_string()),
            signal_task_ids: vec!["101".to_string(), "101".to_string()],
            signal_source: "task_summary".to_string(),
        })
        .expect("append context state");

        let snapshot = db
            .latest_context_state("conv-4")
            .expect("load latest")
            .expect("snapshot exists");

        assert_eq!(snapshot.active_tag.as_deref(), Some("mind"));
        assert_eq!(
            snapshot.active_tasks,
            vec!["101".to_string(), "102".to_string()]
        );
        assert_eq!(snapshot.lifecycle.as_deref(), Some("in-progress"));
        assert_eq!(snapshot.signal_task_ids, vec!["101".to_string()]);
        assert_eq!(snapshot.signal_source, "task_summary");
        assert_eq!(db.context_state_count("conv-4").expect("count"), 1);
    }

    #[test]
    fn raw_event_lineage_tracks_session_and_branch_relationships() {
        let db = MindStore::open_in_memory().expect("open db");

        let root = RawEvent {
            event_id: "evt-root".to_string(),
            conversation_id: "conv-root".to_string(),
            agent_id: "session-a::12".to_string(),
            ts: ts(),
            body: RawEventBody::Message(MessageEvent {
                role: ConversationRole::User,
                text: "root conversation".to_string(),
            }),
            attrs: Default::default(),
        };
        let mut branch_attrs = std::collections::BTreeMap::new();
        branch_attrs.insert(
            "session_id".to_string(),
            serde_json::Value::String("session-a".to_string()),
        );
        branch_attrs.insert(
            "parent_conversation_id".to_string(),
            serde_json::Value::String("conv-root".to_string()),
        );
        branch_attrs.insert(
            "root_conversation_id".to_string(),
            serde_json::Value::String("conv-root".to_string()),
        );
        let branch = RawEvent {
            event_id: "evt-branch".to_string(),
            conversation_id: "conv-branch".to_string(),
            agent_id: "session-a::12".to_string(),
            ts: ts() + chrono::Duration::seconds(1),
            body: RawEventBody::Message(MessageEvent {
                role: ConversationRole::User,
                text: "branch conversation".to_string(),
            }),
            attrs: branch_attrs,
        };

        db.insert_raw_event(&root).expect("insert root raw");
        db.insert_raw_event(&branch).expect("insert branch raw");

        let root_lineage = db
            .conversation_lineage("conv-root")
            .expect("load root lineage")
            .expect("root lineage exists");
        assert_eq!(root_lineage.session_id, "session-a");
        assert_eq!(root_lineage.parent_conversation_id, None);
        assert_eq!(root_lineage.root_conversation_id, "conv-root");

        let branch_lineage = db
            .conversation_lineage("conv-branch")
            .expect("load branch lineage")
            .expect("branch lineage exists");
        assert_eq!(branch_lineage.session_id, "session-a");
        assert_eq!(
            branch_lineage.parent_conversation_id.as_deref(),
            Some("conv-root")
        );
        assert_eq!(branch_lineage.root_conversation_id, "conv-root");

        let session_tree = db
            .session_tree_conversations("session-a", "conv-root")
            .expect("session tree");
        assert_eq!(
            session_tree,
            vec!["conv-branch".to_string(), "conv-root".to_string()]
        );
    }

    #[test]
    fn raw_event_lineage_rejects_partial_branch_metadata() {
        let db = MindStore::open_in_memory().expect("open db");

        let mut attrs = std::collections::BTreeMap::new();
        attrs.insert(
            "parent_conversation_id".to_string(),
            serde_json::Value::String("conv-root".to_string()),
        );

        let event = RawEvent {
            event_id: "evt-invalid-branch".to_string(),
            conversation_id: "conv-branch".to_string(),
            agent_id: "session-a::12".to_string(),
            ts: ts(),
            body: RawEventBody::Message(MessageEvent {
                role: ConversationRole::User,
                text: "invalid branch metadata".to_string(),
            }),
            attrs,
        };

        let err = db.insert_raw_event(&event).expect_err("insert must fail");
        assert!(matches!(err, StorageError::Serialization(_)));
    }

    #[test]
    fn conversation_needs_observer_run_compares_latest_t0_and_t1() {
        let db = MindStore::open_in_memory().expect("open db");

        let raw = sample_message_event("evt-observer", "conv-observer");
        let compact = compact_raw_event_to_t0(&raw, &T0CompactionPolicy::default())
            .expect("compact")
            .expect("kept");
        db.upsert_t0_compact_event(&compact).expect("insert t0");

        assert!(db
            .conversation_needs_observer_run("conv-observer")
            .expect("needs run before t1"));

        db.insert_observation(
            "obs:conv-observer:1",
            "conv-observer",
            compact.ts,
            "t1 observer output",
            &[compact.compact_id.clone()],
        )
        .expect("insert t1");

        assert!(!db
            .conversation_needs_observer_run("conv-observer")
            .expect("does not need run after t1"));
    }

    #[test]
    fn semantic_provenance_roundtrip_preserves_runtime_and_failure_metadata() {
        let db = MindStore::open_in_memory().expect("open db");

        db.upsert_semantic_provenance(&SemanticProvenance {
            artifact_id: "obs:1".to_string(),
            stage: SemanticStage::T1Observer,
            runtime: SemanticRuntime::PiSemantic,
            provider_name: Some("pi".to_string()),
            model_id: Some("small-background".to_string()),
            prompt_version: "observer.v1".to_string(),
            input_hash: "in-hash".to_string(),
            output_hash: Some("out-hash".to_string()),
            latency_ms: Some(123),
            attempt_count: 1,
            fallback_used: false,
            fallback_reason: None,
            failure_kind: None,
            created_at: ts(),
        })
        .expect("insert semantic provenance");

        db.upsert_semantic_provenance(&SemanticProvenance {
            artifact_id: "obs:1".to_string(),
            stage: SemanticStage::T1Observer,
            runtime: SemanticRuntime::Deterministic,
            provider_name: None,
            model_id: None,
            prompt_version: "observer.v1".to_string(),
            input_hash: "in-hash".to_string(),
            output_hash: None,
            latency_ms: None,
            attempt_count: 2,
            fallback_used: true,
            fallback_reason: Some("provider timeout".to_string()),
            failure_kind: Some(SemanticFailureKind::Timeout),
            created_at: ts(),
        })
        .expect("insert fallback provenance");

        let rows = db
            .semantic_provenance_for_artifact("obs:1")
            .expect("load provenance");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].runtime, SemanticRuntime::PiSemantic);
        assert_eq!(rows[1].runtime, SemanticRuntime::Deterministic);
        assert!(rows[1].fallback_used);
        assert_eq!(rows[1].failure_kind, Some(SemanticFailureKind::Timeout));
    }

    #[test]
    fn segment_route_roundtrip_preserves_primary_secondary_and_origin() {
        let db = MindStore::open_in_memory().expect("open db");

        let route = SegmentRoute {
            artifact_id: "obs-42".to_string(),
            primary: SegmentCandidate {
                segment_id: "mind".to_string(),
                confidence_bps: 9_400,
            },
            secondary: vec![
                SegmentCandidate {
                    segment_id: "global".to_string(),
                    confidence_bps: 6_000,
                },
                SegmentCandidate {
                    segment_id: "uncertain".to_string(),
                    confidence_bps: 5_000,
                },
            ],
            routed_by: RouteOrigin::Taskmaster,
            reason: "taskmaster_tag_map:tag=mind->segment=mind".to_string(),
            overridden_by: None,
        };

        db.replace_segment_route(&route).expect("store route");

        let loaded = db
            .segment_route_for_artifact("obs-42")
            .expect("load route")
            .expect("route present");

        assert_eq!(loaded.artifact_id, route.artifact_id);
        assert_eq!(loaded.primary.segment_id, "mind");
        assert_eq!(loaded.secondary.len(), 2);
        assert_eq!(loaded.secondary[0].segment_id, "global");
        assert_eq!(loaded.secondary[1].segment_id, "uncertain");
        assert_eq!(loaded.routed_by, RouteOrigin::Taskmaster);
        assert_eq!(loaded.reason, route.reason);
    }

    #[test]
    fn replace_segment_route_removes_stale_secondary_rows() {
        let db = MindStore::open_in_memory().expect("open db");

        db.replace_segment_route(&SegmentRoute {
            artifact_id: "obs-43".to_string(),
            primary: SegmentCandidate {
                segment_id: "mind".to_string(),
                confidence_bps: 9_000,
            },
            secondary: vec![SegmentCandidate {
                segment_id: "global".to_string(),
                confidence_bps: 5_500,
            }],
            routed_by: RouteOrigin::Heuristic,
            reason: "first pass".to_string(),
            overridden_by: None,
        })
        .expect("first route");

        db.replace_segment_route(&SegmentRoute {
            artifact_id: "obs-43".to_string(),
            primary: SegmentCandidate {
                segment_id: "frontend".to_string(),
                confidence_bps: 9_700,
            },
            secondary: Vec::new(),
            routed_by: RouteOrigin::ManualOverride,
            reason: "override patch".to_string(),
            overridden_by: Some("patch-1".to_string()),
        })
        .expect("second route");

        let loaded = db
            .segment_route_for_artifact("obs-43")
            .expect("load")
            .expect("exists");
        assert_eq!(loaded.primary.segment_id, "frontend");
        assert!(loaded.secondary.is_empty());
        assert_eq!(loaded.routed_by, RouteOrigin::ManualOverride);
        assert_eq!(loaded.overridden_by.as_deref(), Some("patch-1"));
    }

    #[test]
    fn reflector_lease_allows_single_owner_and_stale_takeover() {
        let db = MindStore::open_in_memory().expect("open db");
        let now = ts();

        assert!(db
            .try_acquire_reflector_lease("scope-a", "owner-a", Some(111), now, 1_000)
            .expect("acquire a"));

        assert!(!db
            .try_acquire_reflector_lease(
                "scope-a",
                "owner-b",
                Some(222),
                now + chrono::Duration::milliseconds(500),
                1_000,
            )
            .expect("owner-b blocked"));

        assert!(db
            .try_acquire_reflector_lease(
                "scope-a",
                "owner-b",
                Some(222),
                now + chrono::Duration::milliseconds(1_500),
                1_000,
            )
            .expect("owner-b takeover"));

        let lease = db
            .reflector_lease("scope-a")
            .expect("lease query")
            .expect("lease present");
        assert_eq!(lease.owner_id, "owner-b");
        assert_eq!(lease.owner_pid, Some(222));
    }

    #[test]
    fn reflector_job_claim_complete_and_failure_requeue_roundtrip() {
        let db = MindStore::open_in_memory().expect("open db");
        let now = ts();

        db.try_acquire_reflector_lease("scope-a", "owner-a", Some(101), now, 5_000)
            .expect("acquire lease");

        let job_id = db
            .enqueue_reflector_job(
                "mind",
                &["obs:2".to_string(), "obs:1".to_string()],
                &["conv-1".to_string()],
                120,
                now,
            )
            .expect("enqueue job");

        let claimed = db
            .claim_next_reflector_job(
                "scope-a",
                "owner-a",
                now + chrono::Duration::milliseconds(1),
            )
            .expect("claim")
            .expect("job present");
        assert_eq!(claimed.job_id, job_id);
        assert_eq!(claimed.status, ReflectorJobStatus::Claimed);
        assert_eq!(claimed.attempts, 1);

        assert!(db
            .fail_reflector_job(
                &job_id,
                "owner-a",
                "temporary timeout",
                now + chrono::Duration::milliseconds(2),
                true,
            )
            .expect("requeue"));
        assert_eq!(db.pending_reflector_jobs().expect("pending"), 1);

        let claimed_again = db
            .claim_next_reflector_job(
                "scope-a",
                "owner-a",
                now + chrono::Duration::milliseconds(3),
            )
            .expect("claim again")
            .expect("job present");
        assert_eq!(claimed_again.attempts, 2);

        assert!(db
            .complete_reflector_job(&job_id, "owner-a", now + chrono::Duration::milliseconds(4),)
            .expect("complete"));

        let completed = db
            .reflector_job_by_id(&job_id)
            .expect("load job")
            .expect("job exists");
        assert_eq!(completed.status, ReflectorJobStatus::Completed);
        assert_eq!(completed.attempts, 2);
    }

    #[test]
    fn t3_runtime_lease_allows_single_owner_and_stale_takeover() {
        let db = MindStore::open_in_memory().expect("open db");
        let now = ts();

        assert!(db
            .try_acquire_t3_runtime_lease("project:/repo", "owner-a", Some(111), now, 1_000)
            .expect("acquire a"));

        assert!(!db
            .try_acquire_t3_runtime_lease(
                "project:/repo",
                "owner-b",
                Some(222),
                now + chrono::Duration::milliseconds(500),
                1_000,
            )
            .expect("owner-b blocked"));

        assert!(db
            .try_acquire_t3_runtime_lease(
                "project:/repo",
                "owner-b",
                Some(222),
                now + chrono::Duration::milliseconds(1_500),
                1_000,
            )
            .expect("owner-b takeover"));

        let lease = db
            .t3_runtime_lease("project:/repo")
            .expect("lease query")
            .expect("lease present");
        assert_eq!(lease.owner_id, "owner-b");
        assert_eq!(lease.owner_pid, Some(222));
    }

    #[test]
    fn t3_backlog_job_claim_complete_and_failure_requeue_roundtrip() {
        let db = MindStore::open_in_memory().expect("open db");
        let now = ts();

        db.try_acquire_t3_runtime_lease("project:/repo", "owner-a", Some(101), now, 5_000)
            .expect("acquire lease");

        let refs = vec!["obs:2".to_string(), "obs:1".to_string()];
        let (job_id, inserted) = db
            .enqueue_t3_backlog_job(
                "/repo",
                "session-a",
                "12",
                Some("mind"),
                Some("obs:1"),
                Some("ref:1"),
                &refs,
                now,
            )
            .expect("enqueue t3");
        assert!(inserted);

        let claimed = db
            .claim_next_t3_backlog_job(
                "project:/repo",
                "owner-a",
                now + chrono::Duration::milliseconds(1),
                1_000,
            )
            .expect("claim")
            .expect("job present");
        assert_eq!(claimed.job_id, job_id);
        assert_eq!(claimed.status, T3BacklogJobStatus::Claimed);
        assert_eq!(claimed.attempts, 1);

        assert!(db
            .fail_t3_backlog_job(
                &job_id,
                "owner-a",
                "temporary timeout",
                now + chrono::Duration::milliseconds(2),
                true,
                3,
            )
            .expect("requeue"));
        assert_eq!(db.pending_t3_backlog_jobs().expect("pending"), 1);

        let claimed_again = db
            .claim_next_t3_backlog_job(
                "project:/repo",
                "owner-a",
                now + chrono::Duration::milliseconds(3),
                1_000,
            )
            .expect("claim again")
            .expect("job present");
        assert_eq!(claimed_again.attempts, 2);

        assert!(db
            .complete_t3_backlog_job(&job_id, "owner-a", now + chrono::Duration::milliseconds(4),)
            .expect("complete"));

        let completed = db
            .t3_backlog_job_by_id(&job_id)
            .expect("load job")
            .expect("job exists");
        assert_eq!(completed.status, T3BacklogJobStatus::Completed);
        assert_eq!(completed.attempts, 2);
    }

    #[test]
    fn compaction_checkpoint_round_trip_and_latest_lookup() {
        let db = MindStore::open_in_memory().expect("open db");
        let now = ts();
        let checkpoint = CompactionCheckpoint {
            checkpoint_id: "cmpchk:conv-1:compact-1".to_string(),
            conversation_id: "conv-1".to_string(),
            session_id: "session-1".to_string(),
            ts: now,
            trigger_source: "pi_compact".to_string(),
            reason: Some("pi compaction".to_string()),
            summary: Some("Compacted setup and implementation notes.".to_string()),
            tokens_before: Some(12_345),
            first_kept_entry_id: Some("entry-42".to_string()),
            compaction_entry_id: Some("compact-1".to_string()),
            from_extension: true,
            marker_event_id: Some("evt-compaction-conv-1-compact-1".to_string()),
            schema_version: 1,
            created_at: now,
            updated_at: now,
        };

        db.upsert_compaction_checkpoint(&checkpoint)
            .expect("insert compaction checkpoint");

        let loaded = db
            .latest_compaction_checkpoint_for_conversation("conv-1")
            .expect("load latest checkpoint")
            .expect("checkpoint exists");
        assert_eq!(loaded, checkpoint);

        let by_session = db
            .latest_compaction_checkpoint_for_session("session-1")
            .expect("load latest checkpoint for session")
            .expect("session checkpoint exists");
        assert_eq!(by_session.checkpoint_id, checkpoint.checkpoint_id);

        let later = CompactionCheckpoint {
            reason: Some("pi compaction retry".to_string()),
            updated_at: now + chrono::Duration::seconds(5),
            ..checkpoint.clone()
        };
        db.upsert_compaction_checkpoint(&later)
            .expect("update compaction checkpoint");

        let rows = db
            .compaction_checkpoints_for_conversation("conv-1")
            .expect("list checkpoints");
        assert_eq!(rows.len(), 1, "checkpoint upsert should not duplicate rows");
        assert_eq!(rows[0].reason.as_deref(), Some("pi compaction retry"));
    }

    #[test]
    fn compaction_t0_slice_round_trip_and_latest_lookup() {
        let db = MindStore::open_in_memory().expect("open db");
        let now = ts();

        let slice = build_compaction_t0_slice(
            "conv-1",
            "session-1",
            now,
            "pi_compact_import",
            Some("pi_session_import"),
            Some("checkpoint summary"),
            Some(123),
            Some("entry-9"),
            Some("compact-1"),
            true,
            "pi_compaction_checkpoint",
            &["pi:c1".to_string()],
            &["src/lib.rs".to_string()],
            &["README.md".to_string(), "src/main.rs".to_string()],
            Some("cmpchk:conv-1:compact-1"),
            "t0.compaction.v1",
        )
        .expect("build slice");
        db.upsert_compaction_t0_slice(&slice)
            .expect("insert compaction slice");

        let loaded = db
            .latest_compaction_t0_slice_for_conversation("conv-1")
            .expect("latest by conversation")
            .expect("slice exists");
        assert_eq!(loaded.slice_id, slice.slice_id);
        assert_eq!(loaded.compaction_entry_id.as_deref(), Some("compact-1"));
        assert_eq!(loaded.read_files, vec!["src/lib.rs".to_string()]);

        let by_session = db
            .latest_compaction_t0_slice_for_session("session-1")
            .expect("latest by session")
            .expect("session slice exists");
        assert_eq!(by_session.slice_id, slice.slice_id);

        let by_checkpoint = db
            .compaction_t0_slice_for_checkpoint("cmpchk:conv-1:compact-1")
            .expect("slice by checkpoint")
            .expect("checkpoint slice exists");
        assert_eq!(by_checkpoint.slice_hash, slice.slice_hash);

        let updated = build_compaction_t0_slice(
            "conv-1",
            "session-1",
            now,
            "pi_compact_import",
            Some("pi_session_import"),
            Some("checkpoint summary updated"),
            Some(456),
            Some("entry-10"),
            Some("compact-1"),
            true,
            "pi_compaction_checkpoint",
            &["pi:c1".to_string(), "pi:c1".to_string()],
            &["src/lib.rs".to_string(), "src/lib.rs".to_string()],
            &["src/main.rs".to_string()],
            Some("cmpchk:conv-1:compact-1"),
            "t0.compaction.v1",
        )
        .expect("build updated slice");
        db.upsert_compaction_t0_slice(&updated)
            .expect("update compaction slice");

        let rows = db
            .compaction_t0_slices_for_conversation("conv-1")
            .expect("list slices");
        assert_eq!(rows.len(), 1, "slice upsert should not duplicate rows");
        assert_eq!(
            rows[0].summary.as_deref(),
            Some("checkpoint summary updated")
        );
        assert_eq!(rows[0].tokens_before, Some(456));
    }

    #[test]
    fn append_trace_ids_to_artifact_updates_t1_and_t2_and_supports_lookup() {
        let db = MindStore::open_in_memory().expect("open db");
        let now = ts();

        db.insert_observation("obs:1", "conv-trace", now, "t1", &["t0:1".to_string()])
            .expect("insert obs");
        db.insert_reflection("ref:1", "conv-trace", now, "t2", &["obs:1".to_string()])
            .expect("insert ref");

        assert!(db
            .append_trace_ids_to_artifact(
                "obs:1",
                &[
                    "t0slice:conv-trace:c1".to_string(),
                    "t0slice:conv-trace:c1".to_string()
                ],
            )
            .expect("append obs traces"));
        assert!(db
            .append_trace_ids_to_artifact(
                "ref:1",
                &["t0slice:conv-trace:c1".to_string(), "obs:1".to_string()],
            )
            .expect("append ref traces"));

        let obs = db
            .artifact_by_id("obs:1")
            .expect("load obs")
            .expect("obs exists");
        assert!(obs
            .trace_ids
            .iter()
            .any(|trace_id| trace_id == "t0slice:conv-trace:c1"));

        let ref_artifact = db
            .artifact_by_id("ref:1")
            .expect("load ref")
            .expect("ref exists");
        assert!(ref_artifact
            .trace_ids
            .iter()
            .any(|trace_id| trace_id == "t0slice:conv-trace:c1"));

        let linked = db
            .artifacts_with_trace_id("conv-trace", "t0slice:conv-trace:c1")
            .expect("lookup trace-linked artifacts");
        assert_eq!(linked.len(), 2);
    }

    #[test]
    fn artifact_file_links_round_trip() {
        let db = MindStore::open_in_memory().expect("open db");
        let now = ts();

        let link = ArtifactFileLink {
            artifact_id: "obs:1".to_string(),
            path: "src/lib.rs".to_string(),
            relation: "modified".to_string(),
            source: "pi_compaction_git_diff".to_string(),
            additions: Some(12),
            deletions: Some(3),
            staged: true,
            untracked: false,
            created_at: now,
            updated_at: now,
        };

        db.upsert_artifact_file_link(&link)
            .expect("insert artifact file link");

        let loaded = db
            .artifact_file_links("obs:1")
            .expect("load artifact file links");
        assert_eq!(loaded, vec![link.clone()]);

        db.upsert_artifact_file_link(&ArtifactFileLink {
            staged: false,
            untracked: true,
            updated_at: now + chrono::Duration::seconds(2),
            ..link.clone()
        })
        .expect("update artifact file link");

        let updated = db
            .artifact_file_links("obs:1")
            .expect("load updated artifact file links");
        assert_eq!(updated.len(), 1);
        assert!(!updated[0].staged);
        assert!(updated[0].untracked);
    }

    #[test]
    fn detached_insight_jobs_round_trip_and_stale_active_rows() {
        let db = MindStore::open_in_memory().expect("open db");

        db.upsert_detached_insight_job(
            "delegated",
            Some("specialist"),
            &InsightDetachedJob {
                job_id: "detached-1".to_string(),
                parent_job_id: None,
                mode: InsightDetachedMode::Dispatch,
                status: InsightDetachedJobStatus::Queued,
                agent: Some("insight-t1-observer".to_string()),
                team: None,
                chain: None,
                created_at_ms: 10,
                started_at_ms: None,
                finished_at_ms: None,
                current_step_index: None,
                step_count: Some(1),
                output_excerpt: Some("queued".to_string()),
                stdout_excerpt: Some("queued stdout".to_string()),
                stderr_excerpt: None,
                error: None,
                fallback_used: false,
                step_results: vec![InsightDispatchStepResult {
                    agent: "insight-t1-observer".to_string(),
                    status: "queued".to_string(),
                    output_excerpt: Some("queued".to_string()),
                    stdout_excerpt: Some("queued stdout".to_string()),
                    stderr_excerpt: None,
                    error: None,
                }],
            },
        )
        .expect("insert detached job");

        db.upsert_detached_insight_job(
            "delegated",
            Some("chain_step"),
            &InsightDetachedJob {
                job_id: "detached-2".to_string(),
                parent_job_id: Some("detached-parent".to_string()),
                mode: InsightDetachedMode::Chain,
                status: InsightDetachedJobStatus::Running,
                agent: None,
                team: None,
                chain: Some("insight-handoff".to_string()),
                created_at_ms: 20,
                started_at_ms: Some(25),
                finished_at_ms: None,
                current_step_index: Some(1),
                step_count: Some(2),
                output_excerpt: Some("running".to_string()),
                stdout_excerpt: Some("running stdout".to_string()),
                stderr_excerpt: Some("running stderr".to_string()),
                error: None,
                fallback_used: false,
                step_results: vec![InsightDispatchStepResult {
                    agent: "insight-t2-reflector".to_string(),
                    status: "running".to_string(),
                    output_excerpt: Some("running".to_string()),
                    stdout_excerpt: Some("running stdout".to_string()),
                    stderr_excerpt: Some("running stderr".to_string()),
                    error: None,
                }],
            },
        )
        .expect("insert running detached job");

        let jobs = db
            .detached_insight_jobs(Some("delegated"), Some(10))
            .expect("load jobs");
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].job_id, "detached-2");
        assert_eq!(jobs[0].parent_job_id.as_deref(), Some("detached-parent"));
        assert_eq!(jobs[0].stdout_excerpt.as_deref(), Some("running stdout"));
        assert_eq!(jobs[0].stderr_excerpt.as_deref(), Some("running stderr"));
        assert_eq!(jobs[0].step_results.len(), 1);
        assert_eq!(jobs[0].step_results[0].agent, "insight-t2-reflector");
        assert_eq!(jobs[1].job_id, "detached-1");
        assert!(jobs[1].parent_job_id.is_none());
        assert_eq!(jobs[1].stdout_excerpt.as_deref(), Some("queued stdout"));
        assert_eq!(jobs[1].step_results.len(), 1);

        let changed = db
            .mark_detached_insight_jobs_stale("delegated", "restart observed")
            .expect("mark stale");
        assert_eq!(changed, 2);
        let jobs = db
            .detached_insight_jobs(Some("delegated"), Some(10))
            .expect("load stale jobs");
        assert!(jobs
            .iter()
            .all(|job| job.status == InsightDetachedJobStatus::Stale));
        assert!(jobs.iter().all(|job| job.error.as_deref().is_some()));
    }

    #[test]
    fn import_legacy_store_copies_v1_to_v4_tables_without_duplicates() {
        let legacy_file = NamedTempFile::new().expect("legacy temp db");
        let legacy_conn = Connection::open(legacy_file.path()).expect("open legacy db");
        legacy_conn
            .execute_batch(include_str!("../migrations/0001_mind_schema.sql"))
            .expect("apply migration 1");
        legacy_conn
            .execute_batch(include_str!("../migrations/0002_semantic_runtime.sql"))
            .expect("apply migration 2");
        legacy_conn
            .execute_batch(include_str!("../migrations/0003_reflector_runtime.sql"))
            .expect("apply migration 3");
        legacy_conn
            .execute_batch(include_str!(
                "../migrations/0004_session_conversation_tree.sql"
            ))
            .expect("apply migration 4");
        legacy_conn
            .execute("PRAGMA user_version = 4", [])
            .expect("set legacy schema version");

        legacy_conn
            .execute(
                "
                INSERT INTO raw_events (event_id, conversation_id, agent_id, ts, kind, payload_json, attrs_json)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ",
                params![
                    "evt-legacy-1",
                    "conv-legacy",
                    "session-legacy::12",
                    ts().to_rfc3339(),
                    "message",
                    serde_json::json!({"kind":"message","role":"user","text":"hello"}).to_string(),
                    "{}",
                ],
            )
            .expect("seed legacy raw event");

        let db = MindStore::open_in_memory().expect("open db");
        let first = db
            .import_legacy_store(legacy_file.path())
            .expect("import legacy data");
        assert!(first.tables_imported >= 1);
        assert!(first.rows_imported >= 1);
        assert_eq!(db.raw_event_count("conv-legacy").expect("count raw"), 1);

        let second = db
            .import_legacy_store(legacy_file.path())
            .expect("re-import legacy data");
        assert_eq!(second.rows_imported, 0);
        assert_eq!(db.raw_event_count("conv-legacy").expect("count raw"), 1);
    }

    #[test]
    fn conversation_ids_for_session_lists_known_conversations() {
        let db = MindStore::open_in_memory().expect("open db");

        let root = RawEvent {
            event_id: "evt-root-session-list".to_string(),
            conversation_id: "conv-root-session-list".to_string(),
            agent_id: "session-list::12".to_string(),
            ts: ts(),
            body: RawEventBody::Message(MessageEvent {
                role: ConversationRole::User,
                text: "root".to_string(),
            }),
            attrs: Default::default(),
        };
        let mut branch_attrs = std::collections::BTreeMap::new();
        branch_attrs.insert(
            "session_id".to_string(),
            serde_json::Value::String("session-list".to_string()),
        );
        branch_attrs.insert(
            "parent_conversation_id".to_string(),
            serde_json::Value::String("conv-root-session-list".to_string()),
        );
        branch_attrs.insert(
            "root_conversation_id".to_string(),
            serde_json::Value::String("conv-root-session-list".to_string()),
        );
        let branch = RawEvent {
            event_id: "evt-branch-session-list".to_string(),
            conversation_id: "conv-branch-session-list".to_string(),
            agent_id: "session-list::12".to_string(),
            ts: ts() + chrono::Duration::seconds(1),
            body: RawEventBody::Message(MessageEvent {
                role: ConversationRole::User,
                text: "branch".to_string(),
            }),
            attrs: branch_attrs,
        };

        db.insert_raw_event(&root).expect("insert root");
        db.insert_raw_event(&branch).expect("insert branch");

        let conversations = db
            .conversation_ids_for_session("session-list")
            .expect("query session conversations");
        assert_eq!(
            conversations,
            vec![
                "conv-branch-session-list".to_string(),
                "conv-root-session-list".to_string(),
            ]
        );
    }

    #[test]
    fn t3_backlog_enqueue_and_watermark_roundtrip_are_idempotent() {
        let db = MindStore::open_in_memory().expect("open db");
        let now = ts();

        let refs = vec!["obs:2".to_string(), "obs:1".to_string()];
        let (job_id, inserted) = db
            .enqueue_t3_backlog_job(
                "/repo",
                "session-a",
                "12",
                Some("mind"),
                Some("obs:1"),
                Some("ref:1"),
                &refs,
                now,
            )
            .expect("enqueue t3 job");
        assert!(inserted);

        let (_same_job_id, inserted_again) = db
            .enqueue_t3_backlog_job(
                "/repo",
                "session-a",
                "12",
                Some("mind"),
                Some("obs:1"),
                Some("ref:1"),
                &refs,
                now,
            )
            .expect("enqueue t3 job second time");
        assert!(!inserted_again);

        let pending: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM t3_backlog_jobs WHERE job_id = ?1",
                [job_id],
                |row| row.get(0),
            )
            .expect("pending count");
        assert_eq!(pending, 1);

        db.advance_project_watermark(
            "session:session-a:pane:12",
            Some(now),
            Some("ref:1"),
            now + chrono::Duration::seconds(1),
        )
        .expect("advance watermark");

        let watermark = db
            .project_watermark("session:session-a:pane:12")
            .expect("read watermark")
            .expect("watermark present");
        assert_eq!(watermark.last_artifact_id.as_deref(), Some("ref:1"));
        assert_eq!(watermark.last_artifact_ts, Some(now));
    }

    #[test]
    fn canon_revision_upsert_is_idempotent_and_supersedes_previous_active_revision() {
        let db = MindStore::open_in_memory().expect("open db");
        let now = ts();
        let entry_id = "canon:abc123";

        let first = db
            .upsert_canon_entry_revision(
                entry_id,
                Some("mind"),
                "Initial project canon summary",
                7_200,
                9_400,
                None,
                &["obs:2".to_string(), "obs:1".to_string()],
                now,
            )
            .expect("insert rev1");
        assert_eq!(first.revision, 1);
        assert_eq!(first.state, CanonRevisionState::Active);
        assert_eq!(
            first.evidence_refs,
            vec!["obs:1".to_string(), "obs:2".to_string()]
        );

        let same = db
            .upsert_canon_entry_revision(
                entry_id,
                Some("mind"),
                "Initial project canon summary",
                7_200,
                9_400,
                None,
                &["obs:1".to_string(), "obs:2".to_string()],
                now,
            )
            .expect("idempotent upsert");
        assert_eq!(same.revision, 1);

        let second = db
            .upsert_canon_entry_revision(
                entry_id,
                Some("mind"),
                "Updated project canon summary",
                7_900,
                9_600,
                None,
                &["obs:3".to_string(), "obs:2".to_string()],
                now + chrono::Duration::seconds(1),
            )
            .expect("insert rev2");
        assert_eq!(second.revision, 2);
        assert_eq!(second.state, CanonRevisionState::Active);

        let history = db
            .canon_entry_revisions(entry_id)
            .expect("history query should succeed");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].revision, 2);
        assert_eq!(history[0].state, CanonRevisionState::Active);
        assert_eq!(history[1].revision, 1);
        assert_eq!(history[1].state, CanonRevisionState::Superseded);

        let active = db
            .active_canon_entries(Some("mind"))
            .expect("active query should succeed");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].entry_id, entry_id);
        assert_eq!(active[0].revision, 2);
    }

    #[test]
    fn mark_active_canon_entries_stale_marks_old_untouched_entries_only() {
        let db = MindStore::open_in_memory().expect("open db");
        let now = ts();

        db.upsert_canon_entry_revision(
            "canon:old-a",
            Some("mind"),
            "Older canon entry A",
            7_000,
            8_000,
            None,
            &["obs:1".to_string()],
            now - chrono::Duration::days(30),
        )
        .expect("insert old a");
        db.upsert_canon_entry_revision(
            "canon:old-b",
            Some("mind"),
            "Older canon entry B",
            7_100,
            8_100,
            None,
            &["obs:2".to_string()],
            now - chrono::Duration::days(30),
        )
        .expect("insert old b");
        db.upsert_canon_entry_revision(
            "canon:new-c",
            Some("mind"),
            "Recent canon entry C",
            7_200,
            8_800,
            None,
            &["obs:3".to_string()],
            now - chrono::Duration::days(2),
        )
        .expect("insert new c");

        let marked = db
            .mark_active_canon_entries_stale(
                Some("mind"),
                now - chrono::Duration::days(14),
                &["canon:old-b".to_string()],
            )
            .expect("stale update");
        assert_eq!(marked, 1);

        let stale = db
            .canon_entries_by_state(CanonRevisionState::Stale, Some("mind"))
            .expect("stale query");
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].entry_id, "canon:old-a");

        let active = db.active_canon_entries(Some("mind")).expect("active query");
        assert_eq!(active.len(), 2);
        assert!(active.iter().any(|entry| entry.entry_id == "canon:old-b"));
        assert!(active.iter().any(|entry| entry.entry_id == "canon:new-c"));
    }

    #[test]
    fn handshake_snapshot_upsert_is_idempotent_and_latest_lookup_returns_newest() {
        let db = MindStore::open_in_memory().expect("open db");
        let now = ts();

        let (snapshot_id, inserted) = db
            .upsert_handshake_snapshot(
                "project",
                "project:/repo",
                "# Handshake\n\n- one\n",
                "hash-one",
                120,
                now,
            )
            .expect("insert snapshot");
        assert!(inserted);

        let (_same_id, inserted_again) = db
            .upsert_handshake_snapshot(
                "project",
                "project:/repo",
                "# Handshake\n\n- one\n",
                "hash-one",
                120,
                now,
            )
            .expect("idempotent snapshot");
        assert!(!inserted_again);

        let latest = db
            .latest_handshake_snapshot("project", "project:/repo")
            .expect("latest query")
            .expect("latest exists");
        assert_eq!(latest.snapshot_id, snapshot_id);
        assert_eq!(latest.payload_hash, "hash-one");
        assert_eq!(latest.token_estimate, 120);

        db.upsert_handshake_snapshot(
            "project",
            "project:/repo",
            "# Handshake\n\n- two\n",
            "hash-two",
            160,
            now + chrono::Duration::seconds(1),
        )
        .expect("insert newer snapshot");

        let latest = db
            .latest_handshake_snapshot("project", "project:/repo")
            .expect("latest query")
            .expect("latest exists");
        assert_eq!(latest.payload_hash, "hash-two");
        assert_eq!(latest.token_estimate, 160);
    }
}
