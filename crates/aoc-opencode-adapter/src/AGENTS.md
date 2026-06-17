# Repository Guidelines

Scope: `crates/aoc-opencode-adapter/src`

## Local Contracts
- Conversation files are append-only but truncation-tolerant: checkpoint raw byte cursors, defer incomplete trailing lines, skip corrupt complete lines, and advance only to consumed complete records.
- Never persist raw tool output into Mind; sanitize RawEvent before insert and keep tool-result output redacted through compaction/normalization.
- Event identity and fallback timestamps stay deterministic: prefer event_id/id, otherwise hash conversation_id + line_offset + canonical JSON; use line-offset fallback timestamps only when source timestamps are missing/invalid.
- Maintain lineage compatibility across mind_lineage, lineage, conversation_lineage, payload lineage, and legacy parent/root key spellings; emit canonical lineage attrs when session_id is present.
- Task attribution must resume from latest_context_state and update on tm/aoc-task lifecycle signals across initial and resumed ingest.

## Verification
- `cargo test --manifest-path crates/aoc-opencode-adapter/Cargo.toml`
