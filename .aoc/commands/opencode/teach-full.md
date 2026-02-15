---
description: Run a full teach-mode architecture scan, then pause for user-selected deep dive.
agent: teach
---
Run an interactive teach-mode full scan of this repository.

Rules:
- Prioritize explanation and mapping over implementation changes.
- Do not edit repo code during this flow.
- Keep teaching artifacts local-only under `.aoc/insight/`.

Workflow:
1. Initialize insight workspace (create if missing):
   - `.aoc/insight/README.md`
   - `.aoc/insight/current.md`
   - `.aoc/insight/index.md`
   - `.aoc/insight/insights.md`
   - `.aoc/insight/sessions/`
2. Read AOC context sources in this order:
   - `.aoc/context.md`
   - `aoc-mem read`
   - `aoc-stm read-current` (fallback to `aoc-stm read` when current is empty)
   - `aoc-task list`
   - `.taskmaster/docs/prd.txt` (fallback `.taskmaster/docs/prd.md`)
3. Dispatch parallel exploration tracks across these major subsystems:
   - ingestion/parsing/chunking
   - indexing/embeddings/retrieval
   - deep agent + fs tools
   - API/state/queue
   - frontend/observability/tests
4. Merge findings into a structured teaching report with these sections:
   - system mental model
   - subsystem map
   - key files per subsystem
   - current status (done vs fragile/missing)
   - top risks
5. Save outputs:
   - Full report to `.aoc/insight/sessions/<UTC-timestamp>-teach-full.md`
   - Update `.aoc/insight/current.md` with latest scan summary and next choices
   - Append concise insights to `.aoc/insight/insights.md` with evidence and confidence
   - Append one index entry to `.aoc/insight/index.md`

Output requirements:
- Keep explanations practical and mentor-like.
- Include file references for every subsystem claim.
- Explicitly label missing/unclear areas instead of guessing.
- Return only the teaching report content (no orchestration/meta wrapper text).

PAUSE checkpoint (must end exactly like this):
Choose a subsystem to dive deeper (reply with number).
1) Ingestion / parsing / chunking
2) Indexing / embeddings / retrieval
3) Deep agent orchestration + filesystem tools
4) API, state, and queue flows
5) Frontend, observability, and tests
6) Cross-cutting risks and roadmap gaps

Stop and wait for user reply.
