use aoc_core::mind_contracts::{
    canonical_payload_hash, parse_conversation_lineage_metadata, ArtifactTaskLink,
    ArtifactTaskRelation, ConversationRole, RawEvent, RawEventBody, RouteOrigin, SegmentCandidate,
    SegmentRoute, SemanticFailureKind, SemanticProvenance, SemanticRuntime, SemanticStage,
    T0CompactEvent, ToolMetadataLine,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeSet;
use std::path::Path;
use thiserror::Error;

pub const MIND_SCHEMA_VERSION: i64 = 5;

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

fn parse_reflector_job_status(value: &str) -> Option<ReflectorJobStatus> {
    match value {
        "pending" => Some(ReflectorJobStatus::Pending),
        "claimed" => Some(ReflectorJobStatus::Claimed),
        "completed" => Some(ReflectorJobStatus::Completed),
        "failed" => Some(ReflectorJobStatus::Failed),
        _ => None,
    }
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
        compact_raw_event_to_t0, ConversationRole, MessageEvent, RawEvent, RawEventBody,
        T0CompactionPolicy, ToolExecutionStatus, ToolResultEvent,
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
}
