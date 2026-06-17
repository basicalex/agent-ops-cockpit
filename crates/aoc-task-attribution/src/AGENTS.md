# Repository Guidelines

Scope: `crates/aoc-task-attribution/src`

## Local Contracts
- Preserve artifact-task link meaning: `Active`, `Mentioned`, `WorkedOn`, completion-backfilled `WorkedOn`, and `Completed` keep their confidence order/source strings; duplicate `(task_id, relation)` drafts merge via `LinkDraft::key`/`upsert_draft` with highest confidence/source and unioned sorted evidence.
- Keep attribution inputs narrow and evidence-backed: task IDs may come only from active context states, artifact text, and t0 compact events inside `AttributionConfig`'s mention window; evidence IDs retain `ctx:`, `artifact:*:text`, or `t0:` prefixes.

## Verification
- `cargo test --manifest-path crates/Cargo.toml -p aoc-task-attribution --lib`

## Do Not
- Do not add fuzzy/global task matching, widen mention windows, or change completion backfill/Completed relation timing without focused attribution tests.
- Do not bypass sorted `BTreeSet` evidence collection before `ArtifactTaskLink::new`.

## Update When
- Confidence constants, `TaskAttributionEngine::attribute_conversation`, completion/mention extraction, draft merge keys, or store upsert flow change.
