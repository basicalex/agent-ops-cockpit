- [2026-02-23 11:37] Mind implementation handoff (T0/T1/T2 rollout)

Scope completed:
- Updated mind tasks 101,102,103,105,107,108 to encode T0 compact transcript policy.
- Updated PRD at .taskmaster/docs/prds/aoc-mind_prd.md with new T0 feature, dual-lane model, parser-budget rules, roadmap tasks, test strategy, data model, decisions, risks, and open questions.
- mind tag now linked to PRD (.taskmaster/docs/prds/aoc-mind_prd.md).

Locked strategy:
- Dual-lane ingestion: raw events remain authoritative/provenance; T0 compact events are deterministic derived lane for T1/T2.
- T0 defaults: keep system/user/assistant content; strip bulky tool outputs; retain one-line tool metadata (tool, success/fail, latency, exit code if present, output size).
- T0 allows policy-versioned allowlisted snippets for selected tools.
- T1 policy: one conversation per pass only; if T0 for one conversation <= ~28k target (32k hard cap), single-pass T1; else chunk only within same conversation; no cross-conversation mixing.
- Provider policy: local deterministic baseline authoritative; Zen optional background enhancer for T2 first; strict timeout/retry/budget/redaction and fail-open fallback.

Current task graph status (mind):
- Next task is 101 (no deps).
- Then 102+103 can proceed; 104->105->106/107->109/108->110.

Files changed in this session for this plan:
- .taskmaster/tasks/tasks.json
- .taskmaster/docs/prds/aoc-mind_prd.md

Implementation guardrails for next agent:
- Keep rollout additive and default-safe; do not break existing AOC flows.
- Start with contracts/tests first (task 101), then storage (102) and ingestion+T0 derivation/checkpoints (103).
- Maintain deterministic fixtures/golden tests for compaction and parser-budget behavior.
- Avoid mandatory provider dependencies in hot paths.
