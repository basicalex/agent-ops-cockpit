# Compaction-Derived T0 Slice Contract (Task 151.1)

## Purpose
Define the first-class normalized T0 representation for Pi compaction checkpoints so live and imported compaction events become replayable, provenance-preserving Mind substrate instead of only durable markers.

This contract is intentionally additive:
- keep existing `compact_events_t0` for message/tool-result compaction output,
- add a dedicated compaction-slice representation for Pi-native compaction boundaries,
- preserve rebuildability from raw events + `compaction_checkpoints` + imported Pi details.

## Why a dedicated T0 slice is needed
Current T0 compaction in `compact_raw_event_to_t0(...)` only handles:
- `RawEventBody::Message`
- `RawEventBody::ToolResult`

Pi compaction checkpoints are different. They represent:
- a session continuity boundary,
- a summary of context compression,
- kept/discarded frontier metadata,
- structured detail payloads like `readFiles` / `modifiedFiles`,
- a durable provenance anchor for later T1/T2/T3 and recovery.

So they should not be forced into the same row shape as a compacted chat utterance or tool output.

## Existing canonical sources
A compaction-derived T0 slice may be produced from either:

### 1. Live path
- Pi `session_compact`
- extension -> `mind_compaction_checkpoint`
- wrapper runtime persists:
  - raw marker event
  - `compaction_checkpoints`
  - current evidence links / observer artifacts

### 2. Import/replay path
- Pi session JSONL importer (`aoc-pi-adapter`)
- imported `compaction` entries
- imported source raw event
- imported `compaction_checkpoints`
- imported detail payload attrs (`readFiles`, `modifiedFiles`)

The T0 slice contract must normalize both sources into the same first-class representation.

## T0 slice role in the layer model
Compaction-derived T0 slices belong to **T0** because they are:
- replayable,
- session/provenance preserving,
- not yet semantic synthesis,
- suitable as downstream substrate for T1 observers.

They are not themselves T1 observations.

## Core design principles

### 1. One slice per compaction boundary
A compaction-derived T0 slice represents one Pi compaction checkpoint.

### 2. Idempotent by compaction identity
When `compaction_entry_id` exists, it is the primary dedupe key within a conversation/session.

### 3. Source-path neutral
The resulting slice should be the same whether derived from:
- live wrapper checkpoint handling, or
- imported Pi session history.

### 4. Preserve enough structure for replay/rebuild
The slice must retain enough normalized structure that downstream logic does not need to scrape arbitrary raw JSON blobs for ordinary use.

### 5. Do not bloat T1 prose
Structured detail payloads remain structured evidence/provenance, not observer summary text.

## Recommended storage model
Add a dedicated table for compaction-derived T0 slices rather than overloading `compact_events_t0`.

Recommended table name:
- `compaction_slices_t0`

Rationale:
- the existing `compact_events_t0` schema is optimized for compacted message/tool rows,
- compaction checkpoints carry different fields and semantics,
- a separate table keeps query surfaces and replay logic clear.

## Recommended stored record shape
Suggested normalized record:

- `slice_id: String`
- `slice_hash: String`
- `schema_version: u32`
- `conversation_id: String`
- `session_id: String`
- `ts: DateTime<Utc>`
- `trigger_source: String`
- `reason: Option<String>`
- `summary: Option<String>`
- `tokens_before: Option<u32>`
- `first_kept_entry_id: Option<String>`
- `compaction_entry_id: Option<String>`
- `from_extension: bool`
- `source_kind: String`
- `source_event_ids: Vec<String>`
- `read_files: Vec<String>`
- `modified_files: Vec<String>`
- `checkpoint_id: Option<String>`
- `policy_version: String`

### Field meaning

#### `slice_id`
Stable slice identifier.

Preference order:
1. `t0slice:<conversation_id>:<compaction_entry_id>` when present
2. deterministic hash-derived fallback from conversation/session/timestamp/source ids

#### `slice_hash`
Canonical content hash of the normalized slice body for replay verification and idempotent rebuild checks.

#### `schema_version`
Version of the slice schema, separate from raw-event or checkpoint schema if needed.

#### `conversation_id`
Mind conversation identity.
- live path: current runtime conversation id
- import path: `pi:<session_id>` per current importer contract

#### `session_id`
Required explicit session identifier.

#### `ts`
Compaction boundary timestamp.

#### `trigger_source`
Normalized source label, likely one of:
- `pi_compact`
- `pi_compact_import`

This should preserve whether it originated live or via importer while still representing the same semantic kind.

#### `reason`
Optional textual reason such as:
- manual compaction
- threshold-suggested compact
- import recovery

#### `summary`
Pi compaction summary text.

#### `tokens_before`
Pre-compaction token count when available.

#### `first_kept_entry_id`
Pi compaction frontier marker.

#### `compaction_entry_id`
Primary Pi compaction identity when available.

#### `from_extension`
Whether the compaction was extension-hook originated (`fromHook`/live extension context).

#### `source_kind`
Normalized source class, e.g.:
- `pi_compaction_checkpoint`

This distinguishes the slice from ordinary message/tool T0 rows.

#### `source_event_ids`
Backing raw event ids for provenance and replay.
Should include:
- live raw marker event id when available
- imported source raw event id when available

#### `read_files`
Normalized `details.readFiles` paths.

#### `modified_files`
Normalized `details.modifiedFiles` paths.

#### `checkpoint_id`
Foreign-key-style reference to `compaction_checkpoints.checkpoint_id` when known.

#### `policy_version`
Normalization policy version for rebuild compatibility, distinct from ordinary utterance compaction policy if helpful.

## Provenance contract
Each compaction-derived T0 slice must preserve enough provenance to answer:
- which conversation/session did this belong to?
- which raw event(s) or checkpoint produced it?
- was it created live or via importer?
- what compaction boundary did it correspond to?
- what kept-entry frontier and file details were associated with it?

Minimum provenance links:
- `conversation_id`
- `session_id`
- `checkpoint_id`
- `compaction_entry_id`
- `source_event_ids`

## Data-source precedence rules
When building a slice, prefer data in this order:

### Identity
1. `compaction_checkpoints.compaction_entry_id`
2. raw source payload `id`
3. deterministic hash fallback

### Timestamp
1. checkpoint `ts`
2. source raw event `ts`

### Summary/frontier/token metadata
1. checkpoint normalized fields
2. source raw payload attrs/payload

### Detail payloads (`readFiles`, `modifiedFiles`)
1. source raw event attrs (`pi_detail_*`)
2. raw payload `details.*`
3. empty list

This keeps live/imported normalization consistent.

## Normalization contract

### Input set for normalization
A normalization routine should accept one or more of:
- `CompactionCheckpoint`
- source `RawEvent`/raw marker
- optional imported/live detail attrs

### Output
One normalized compaction-derived T0 slice.

### Idempotency rule
Upsert by:
- `slice_id`

And verify semantic stability through:
- `slice_hash`

### Replay rule
If derived state is wiped, rebuilding from:
- raw events
- `compaction_checkpoints`

should reproduce equivalent slices.

## Relationship to existing `compact_events_t0`
Do **not** replace or mutate the current message/tool-result T0 model.

Instead:
- `compact_events_t0` continues to represent compacted conversational/tool substrate,
- `compaction_slices_t0` represents compaction-boundary substrate.

Downstream observer/runtime logic may later consume both as T0 inputs.

## Relationship to evidence
Compaction-derived T0 slices should be able to link to structured evidence, but the slice itself should only store normalized file lists directly.

Immediate minimal requirement:
- retain `read_files`
- retain `modified_files`
- allow downstream linkage from slice -> evidence/artifact references later

This is enough for Task `151.3` to connect slices to:
- `artifact_file_links`
- downstream T1 outputs
- future provenance queries

## Relationship to Mission Control / health surfacing
Once slices exist, Mission Control and related status surfaces should be able to report:
- latest checkpoint
- whether it has a normalized T0 slice
- whether T1 has consumed it yet
- replay/rebuild eligibility

So the slice becomes the key bridge between:
- checkpoint durability,
- replayability,
- observer readiness.

## Recommended implementation boundaries

### `aoc-core`
Add the core contract type for the normalized compaction-derived slice.
For example:
- `CompactionT0Slice`
- plus deterministic hash/id helpers

### `aoc-storage`
Add:
- migration for `compaction_slices_t0`
- row type
- upsert/query helpers

### producer/runtime side
Add normalization from:
- live checkpoint path
- imported checkpoint path

Likely near existing wrapper/Mind runtime code and/or importer integration points.

## Minimal initial query surface
Recommended initial queries:
- `upsert_compaction_t0_slice(...)`
- `compaction_t0_slices_for_conversation(...)`
- `latest_compaction_t0_slice_for_conversation(...)`
- `latest_compaction_t0_slice_for_session(...)`
- maybe `compaction_t0_slice_for_checkpoint(...)`

## Policy/version guidance
Use a dedicated normalization policy label, for example:
- `t0.compaction.v1`

This avoids conflating ordinary message/tool utterance compaction policy with compaction-boundary slice normalization.

## Initial implementation scope for Task 151

### 151.1 (this contract)
Define:
- stored slice shape
- provenance rules
- source precedence rules
- idempotency rules

### 151.2
Implement normalization from:
- live checkpoints
- imported checkpoints
- raw detail attrs/payloads

### 151.3
Link slices to:
- evidence
- downstream artifacts
- observer consumption flow

### 151.4
Add replay/rebuild tests proving slices can be rederived safely.

## Explicit non-goals for this slice
This contract does not yet require:
- full graph projection
- direct task mutation
- rich diff payload storage
- commit references
- branch-aware synthetic conversation splitting
- replacing existing T0 compact utterance rows

## Completion criteria for 151.1
This subtask is complete when we have:
1. a clear dedicated first-class compaction-derived T0 slice shape,
2. a decision to store it separately from ordinary `compact_events_t0` rows,
3. provenance and data-precedence rules for live/imported normalization,
4. a stable basis for storage migration and normalization implementation.
