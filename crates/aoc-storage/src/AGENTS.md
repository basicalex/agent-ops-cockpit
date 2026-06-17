# Repository Guidelines

Scope: `crates/aoc-storage/src`

## Local Contracts
- Schema changes must be monotonic and versioned: bump MIND_SCHEMA_VERSION, extend MindStore::migrate in order, set PRAGMA user_version, record migrations where current paths do, and keep explicit SELECT lists/parsers/round-trip tests synchronized.
- Storage boundaries must reject unredacted secrets: raw events use raw_event_contains_unredacted_secret, and text-bearing durable surfaces use ensure_no_secrets_in_text/optional variants before INSERT/UPSERT.
- Reflector/T3 leases and job claims remain owner- and expiry-gated: acquisition replaces only same-owner or expired leases, and claim_next_* returns None unless owner_id matches and expires_at >= now.
- Segment-route persistence preserves replacement semantics: delete old rows before replacement, load ordered by confidence then segment id, error on invalid confidence/origin, and strip storage rank suffixes from public reasons.
- Compaction checkpoint/T0 slice storage must preserve idempotent upserts, conversation-scoped compaction_entry_id, latest lookups by conversation/session/checkpoint, and round-trippable slice hashes/source/read/modified/token/first-kept fields.

## Verification
- `cargo test -p aoc-storage --lib`
