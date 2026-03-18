# Pi Session Importer Contract (Task 155.1)

## Purpose
Define the minimal, correct importer contract for reading Pi-native session history into AOC Mind as replayable substrate for bootstrap, backfill, and recovery.

This contract is intentionally **source-substrate first**:
- **Pi session JSONL** remains the authoritative session-history source.
- **AOC Mind SQLite** remains the canonical derived semantic/project memory store.
- The importer should preserve enough structure and provenance to support later T0/T1/T2/T3 processing without forcing a graph-DB-first design.

## Inputs confirmed from Pi docs
Installed Pi docs confirm:
- Sessions are stored as **JSONL files** under `~/.pi/agent/sessions/`.
- Session files are **tree-structured** via `id` / `parentId`.
- Header contains session metadata including `id`, `cwd`, optional `parentSession`, and `version`.
- Relevant entry types include:
  - `message`
  - `compaction`
  - `branch_summary`
  - `custom`
  - `model_change`
  - `thinking_level_change`
- Compaction and branch-summary entries can carry `details`, with default useful fields:
  - `details.readFiles: string[]`
  - `details.modifiedFiles: string[]`

## Existing repo constraints
Current Mind storage/runtime already provides:
- `raw_events`
- T0 compact events via `compact_raw_event_to_t0(...)`
- `ingestion_checkpoints`
- `compaction_checkpoints`
- `artifact_file_links`
- conversation lineage attrs on raw events

Current limitation:
- `compact_raw_event_to_t0(...)` only promotes `RawEventBody::Message` and `RawEventBody::ToolResult`.
- Compaction/branch-summary/session-shape records therefore need either:
  - raw marker + dedicated side tables now, and/or
  - a new first-class normalization path for compaction-derived T0 slices in Task 151.

## Import scope for phase 1
The importer should handle:
1. Session header metadata
2. Message entries
3. Compaction entries
4. Branch-summary entries
5. Important metadata-only entries (`model_change`, `thinking_level_change`, `custom`)
6. Relevant structured detail payloads (`readFiles`, `modifiedFiles`)

The importer should **not** attempt in phase 1 to:
- replace wrapper live ingestion,
- fully reconstruct semantic artifacts directly,
- invent a separate graph store,
- fully split Pi session trees into multiple branch-specific semantic conversations unless needed later.

## Import unit and identity model

### Source unit
One Pi `.jsonl` file is the importer's base source unit.

### Canonical source identifiers
For each imported file, preserve:
- `session_file_path`
- Pi header `id` as `pi_session_id` when present
- Pi header `version`
- Pi header `cwd`
- Pi header `parentSession` when present

### Mind identity contract (phase 1)
Use:
- `session_id = <pi header id>` when present, otherwise deterministic file-derived fallback
- `conversation_id = pi:<session_id>` for the imported session file as a whole

Rationale:
- This keeps the initial importer simple and replay-safe.
- It avoids prematurely inventing synthetic branch conversations.
- It still preserves tree semantics in raw payload/attrs for later branch-aware projections.

### Branch/tree provenance
For every imported entry, preserve in raw payload attrs when available:
- Pi entry `id`
- Pi entry `parentId`
- Pi entry `type`
- `session_file_path`
- `pi_session_id`
- `pi_session_version`
- `cwd`
- `parentSession`

Future work may project branch-aware synthetic conversation views from this preserved tree metadata.

## Normalization contract

### 1. Session header
Pi header line should not become a T0 message.

It should be used to:
- derive `session_id`
- derive default `conversation_id`
- derive importer lineage attrs
- seed importer/reconciler metadata

Optional future enhancement:
- persist a session-source record/query surface if importer operations need richer source inspection.

### 2. Message entries (`type: "message"`)
Pi `message` entries should normalize into `RawEvent` as follows.

#### User / assistant textual messages
Map to:
- `RawEventBody::Message`

Fields:
- `event_id` = deterministic from Pi entry `id` when present; fallback to canonical hash
- `ts` = Pi entry timestamp or message timestamp if needed
- `conversation_id` = `pi:<session_id>`
- `agent_id` = importer/runtime agent id
- `attrs` = source/session/tree provenance

#### Content extraction
For `user` and `assistant` messages:
- concatenate `text` blocks in order
- omit binary payloads from the normalized text body
- if useful later, retain a source marker in attrs indicating non-text blocks were present

#### Assistant thinking blocks
Do **not** treat raw reasoning blocks as first-class semantic text by default.
Recommended phase-1 rule:
- exclude `thinking` blocks from the normalized `MessageEvent.text`
- preserve the original Pi entry payload in `RawEventBody::Other` only if needed for source fidelity, or keep it solely in raw attrs/payload snapshot if the importer stores the full source payload there

#### Tool-call blocks inside assistant messages
Tool-call blocks should not become standalone T0 text.
Preserve them as source provenance in the raw payload/attrs for future traversal.

### 3. Tool results within message entries (`role: "toolResult"`)
Map to:
- `RawEventBody::ToolResult`

Fields:
- `tool_name` from `toolName`
- `status` from `isError`
- `output` from concatenated text content when present
- `redacted = false` unless importer learns otherwise from source metadata
- `exit_code` / `latency_ms` when derivable, otherwise `None`

This keeps compatibility with existing `compact_raw_event_to_t0(...)`.

### 4. Bash execution messages (`role: "bashExecution"`)
Normalize as tool-like execution records by mapping to:
- `RawEventBody::ToolResult`

Recommended mapping:
- `tool_name = "bash_execution"`
- `status = success/failure` based on `exitCode` and cancellation state
- `output = output`
- `exit_code = exitCode`
- `redacted = excludeFromContext` should **not** be treated as content redaction; preserve the original flag in attrs instead
- attrs should include:
  - `command`
  - `cancelled`
  - `truncated`
  - `fullOutputPath`
  - `excludeFromContext`

Rationale:
- Existing T0 compaction already knows how to compact `ToolResult` events.
- Bash execution is semantically closer to tool execution than to free text.

### 5. Compaction entries (`type: "compaction"`)
Always preserve the source entry.

Phase-1 normalization:
- insert a `RawEventBody::Other` source record containing the compaction payload and provenance
- upsert `CompactionCheckpoint`

Required checkpoint fields:
- `conversation_id`
- `session_id`
- `ts`
- `trigger_source = "pi_compact"`
- `summary`
- `tokens_before`
- `first_kept_entry_id`
- `compaction_entry_id`
- `from_extension` from `fromHook`
- source marker event id if available

Structured details to preserve:
- `details.readFiles`
- `details.modifiedFiles`

Task 151 will promote these checkpoints/source records into first-class compaction-derived T0 slices.

### 6. Branch-summary entries (`type: "branch_summary"`)
Always preserve the source entry.

Phase-1 normalization:
- insert a `RawEventBody::Other` source record containing:
  - `summary`
  - `fromId`
  - `fromHook`
  - `details`
  - source/session/tree provenance

Branch summaries should not automatically become T0 text in phase 1.
They are preserved as structured substrate for later branch-aware retrieval/projection.

### 7. Custom entries / custom messages
#### `type: "custom"`
Preserve as:
- `RawEventBody::Other`

Use cases:
- extension state persistence
- future importer-specific provenance

#### `role: "custom"` inside message entries
If display/content is meaningful text, importer may preserve a textual summary in `RawEventBody::Other` plus attrs:
- `customType`
- `display`
- `details`

Do not treat custom messages as ordinary user/assistant semantic turns by default.

### 8. Model/thinking-level changes
Normalize as:
- `RawEventBody::Other`

Reason:
- useful for provenance/debugging
- not primary semantic substrate for T0/T1 in phase 1

## Structured detail preservation contract
The importer must preserve Pi-native structured details even when they are not yet promoted into standalone tables.

Minimum preserved fields:
- `compaction.details.readFiles`
- `compaction.details.modifiedFiles`
- `branch_summary.details.readFiles`
- `branch_summary.details.modifiedFiles`

Phase-1 storage rule:
- keep them in raw payload snapshots / attrs on imported source events
- make them available for later Task 151 normalization and future evidence-link creation

Preferred later direction:
- convert these detail fields into first-class evidence links without bloating T1 prose

## Reconciler/checkpoint contract
The importer should be incremental and idempotent.

### Checkpoint key
Phase-1 can reuse the existing `IngestionCheckpoint` model keyed by `conversation_id = pi:<session_id>`.

### Cursor model
Because Pi sessions are JSONL files, use byte-offset cursors:
- `raw_cursor`
- `t0_cursor`

### Reset behavior
If the file shrinks or is replaced:
- detect cursor > file length
- reset to zero
- mark the run as reset/reconciled

### Duplicate safety
Use deterministic ids so repeated imports do not duplicate:
- raw events
- compaction checkpoints
- later T0 slices

Recommended event id preference order:
1. Pi entry `id`
2. deterministic canonical hash with source file path + line offset + normalized payload

## Recommended implementation location
The importer should follow the existing adapter pattern used by:
- `crates/aoc-opencode-adapter`

Recommended new surface:
- a Pi-specific adapter crate or module with an API parallel to `OpenCodeIngestor`

Suggested responsibilities:
- read/scan Pi session files
- normalize to `RawEvent`
- upsert `CompactionCheckpoint` for compaction entries
- persist importer checkpoints
- leave Task 151 to create first-class compaction-derived T0 slices

## Gaps vs current OpenCode ingestor
The existing OpenCode ingestor already provides the reusable pattern for:
- incremental JSONL scanning
- deterministic raw event ids
- `RawEvent` insertion
- T0 compaction from message/tool-result bodies
- ingestion checkpoints

New Pi-specific work needed:
1. parse Pi session header
2. parse Pi tree entry types
3. map Pi message roles/content blocks correctly
4. upsert compaction checkpoints from imported `compaction` entries
5. preserve branch-summary payloads and details
6. carry session/tree provenance in attrs
7. expose imported detail payloads for later evidence normalization

## Explicit phase-1 decisions
- **Do not replace Mind SQLite with Pi-native storage.**
- **Do not invent a graph DB layer.** Use relational tables plus provenance/query projection later.
- **Do not over-promote branch summaries into semantic text.** Preserve first; project later.
- **Do not block developer workflow.** Importer is for bootstrap/backfill/recovery, not a hard runtime dependency for session compaction.
- **Do preserve enough raw/session provenance** so future branch-aware projections remain possible.

## Deliverables for Task 155.1
This subtask is complete when we have:
1. confirmed Pi session file location and entry types,
2. defined the phase-1 identity model (`session_id`, `conversation_id`, provenance attrs),
3. defined normalization rules per entry/message type,
4. defined incremental checkpoint/idempotency behavior,
5. identified the delta vs `aoc-opencode-adapter`.

## Immediate follow-on work
- Task `155.2`: implement importer checkpointing and session-file reconciliation
- Task `155.3`: normalize Pi messages, compaction entries, and branch summaries into Mind substrate
- Task `151`: promote imported compaction checkpoints into first-class compaction-derived T0 slices
