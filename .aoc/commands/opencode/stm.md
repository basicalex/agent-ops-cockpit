---
description: Capture concise short-term handoff memory in .aoc/stm/current.md
agent: build
---
Create or update `.aoc/stm/current.md` as a compact handoff snapshot for the current work.

Use this structure:
- Objective + active task/subtask IDs (and tag)
- Done
- In progress
- Blockers / risks
- Files touched
- Last command outcomes (tests/lint/build)
- Open decisions / assumptions
- Next 3-5 concrete steps

Rules:
- Keep it short, actionable, and specific.
- Do not duplicate long-term memory entries; only include ephemeral execution state.
- If there is a durable architectural decision, explicitly note: "Promote to aoc-mem".

When finished, remind me I can run `aoc-stm` (default `--last`) to archive and continue in a fresh tab/session with STM injected.
