---
description: Deep-dive a selected subsystem with implementation details, tradeoffs, and exercises.
agent: teach
---
Run a targeted teach-mode deep dive.

Input:
- Use the `/teach-dive` argument as subsystem name or number.
- Accept either labels or numeric choices from `/teach-full`:
  1) ingestion/parsing/chunking
  2) indexing/embeddings/retrieval
  3) deep agent orchestration + filesystem tools
  4) API/state/queue
  5) frontend/observability/tests
  6) cross-cutting risks and roadmap gaps

If input is missing or ambiguous:
- Show the numbered list above and ask for one choice.
- Do not proceed until one subsystem is selected.

Workflow:
1. Read `.aoc/insight/current.md` when available to continue context.
2. Refresh context sources relevant to the target subsystem:
   - `.aoc/context.md`
   - `aoc-mem read`
   - `aoc-stm read-current` (fallback `aoc-stm read`)
   - `aoc-task list`
3. Explore target subsystem deeply; use parallel sub-exploration when it improves coverage.
4. Produce output with exactly these sections:
   - concept in plain English
   - how this repo implements it (file refs)
   - tradeoffs and alternatives
   - debugging checklist
   - 2-3 hands-on exercises
5. Persist insight artifacts:
   - Save the deep-dive note to `.aoc/insight/sessions/<UTC-timestamp>-teach-dive-<subsystem-slug>.md`
   - Update `.aoc/insight/current.md` with dive summary and suggested next step
   - Append key insights to `.aoc/insight/insights.md` with confidence and evidence
   - Append one index entry to `.aoc/insight/index.md`

Rules:
- Do not edit repository code unless the user explicitly asks for a guided change.
- If a guided change is requested, explain impact first, then implement only what was requested.
- Label uncertain findings clearly.
- Return only the deep-dive content (no orchestration/meta wrapper text).

End checkpoint (must ask this before stopping):
Would you like to continue deeper, switch subsystem, or run a guided change?
1) Continue deeper in this subsystem
2) Switch subsystem
3) Run a guided change

Stop and wait for user reply.
