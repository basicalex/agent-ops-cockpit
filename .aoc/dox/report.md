# AOC DOX Report

- Schema: `aoc.dox.v1`
- Directories scanned: 507
- CodeGraph available: true
- Budget status: `Ok`
- Candidates: create=17, update=0, reject=490

Critic-approved create proposals materialized in `.aoc/dox/candidates.json`: `.omp/extensions`, `bin`, `crates/aoc-agent-wrap-rs/src`, `crates/aoc-cli`, `crates/aoc-core/src`, `crates/aoc-hub-rs/src`, `crates/aoc-installer/src`, `crates/aoc-mind/src`, `crates/aoc-mind/src/bin`, `crates/aoc-mission-control/src`, `crates/aoc-opencode-adapter/src`, `crates/aoc-pi-adapter/src`, `crates/aoc-segment-routing/src`, `crates/aoc-storage/src`, `crates/aoc-task-attribution/src`, `crates/aoc-taskmaster/src`, and `crates/aoc-yazi-mermaid/src`.

Excluded critic/scout rejects remain rejected: `bin/__pycache__` and `crates/aoc-control/src`.

Local `AGENTS.md` files are not written by `aoc dox map`; use `aoc dox apply --dry-run` before any apply.
