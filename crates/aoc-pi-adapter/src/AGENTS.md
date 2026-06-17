# Repository Guidelines

Scope: `crates/aoc-pi-adapter/src`

## Local Contracts
- Require a newline-terminated JSON session header and derive conversation_id as pi:<session_id>; keep missing-id fallback based on the session file path stable.
- Ingest only complete newline-delimited entries after header/checkpoint cursor; never ingest the header, defer partial trailing lines, skip corrupt complete lines, and reset to header_end_cursor on truncation.
- Never persist bash/tool output from Pi sessions into Mind; ToolResultEvent.output stays None and redacted while source output remains only in the Pi session/artifact file.
- Preserve Pi source attrs and lineage attrs on raw events: session/file/conversation/import ids, entry id/type/parent, cwd/version/parent session when present, and LINEAGE_ATTRS_KEY with root conversation id.
- Compaction imports must keep checkpoint/T0 slice rebuildability: marker/source event links, entry ids, source/read/modified files, tokens, first-kept entry, and pi_compaction_checkpoint source remain round-trippable.

## Verification
- `cargo test --manifest-path crates/aoc-pi-adapter/Cargo.toml`
