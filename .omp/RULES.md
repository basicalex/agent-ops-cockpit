# agent-ops-cockpit rules

Curated from the project mnemopi bank on 2026-08-06 (`/optimize-mnemopi`).

- The AOC startup capsule (`aoc-omp-context` / `aoc-handshake`) is injected at launch — never re-derive or re-read it, and check aoc-handshake before loading broad project memory.
- `/commit` is not Git staging — never treat it like `git add`; never commit dirty unrelated work alongside your changes.
- Never run `aoc dox apply --yes` unless the user explicitly asked for it.
- Never test installer flows with live installs — use isolated temp/PATH fixtures and constrained download flows.
- Transcription phrase biasing must include the core spellings: `Oh My Pi`, `OMP`, `Agent Ops Cockpit`, `AOC`; load the system lexicon when present.
