use aoc_core::mind_contracts::{
    ArtifactTaskLink, ArtifactTaskRelation, ConversationRole, RawEvent, RawEventBody, RouteOrigin,
    SegmentCandidate, SegmentRoute, SemanticFailureKind, SemanticProvenance, SemanticRuntime,
    SemanticStage, T0CompactEvent, ToolMetadataLine,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeSet;
use std::path::Path;
use thiserror::Error;

pub const MIND_SCHEMA_VERSION: i64 = 2;

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

        Ok(changes > 0)
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
            "aoc_mem_decisions",
            "ingestion_checkpoints",
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
}
