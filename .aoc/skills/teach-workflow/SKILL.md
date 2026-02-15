---
name: teach-workflow
description: Run guided teach-mode scans, dives, and local insight logging.
---

## When to use
- Onboard a developer to this repository.
- Build a subsystem mental model before making changes.
- Create a guided learning path with checkpoints.

## Core commands
- `/teach-full` for broad architecture scan + pause checkpoint.
- `/teach-dive <subsystem>` for targeted deep dives.

## Standard flow
1. Ensure context is loaded from:
   - `.aoc/context.md`
   - `aoc-mem read`
   - `aoc-stm read-current` (fallback `aoc-stm read`)
   - `aoc-task list`
   - `.taskmaster/docs/prd.txt` (fallback `.taskmaster/docs/prd.md`)
2. For full scan, explore subsystem tracks in parallel.
3. Synthesize a teaching report with:
   - system mental model
   - subsystem map
   - key files per subsystem
   - current status (done vs fragile/missing)
   - top risks
4. End with numbered checkpoint options and wait for user choice.
5. For deep dive, return:
   - concept in plain English
   - repo implementation with file refs
   - tradeoffs and alternatives
   - debugging checklist
   - 2-3 hands-on exercises

## Insight logging (local-only)
Maintain a local insight workspace in `.aoc/insight/`:
- `current.md`: active teaching state and pending choices
- `insights.md`: append-only insight log
- `index.md`: compact history index
- `sessions/*.md`: timestamped full scan and deep-dive snapshots

Do not change `.gitignore` for this workspace; keep it local-only.

## Insight entry format
- timestamp (UTC)
- subsystem
- insight
- evidence (file refs)
- confidence (`high|medium|low`)
- suggested action
- promote to memory (`yes|no`)

## Guardrails
- Default to read-only exploration.
- Do not edit code unless explicitly requested.
- Never edit `.aoc/memory.md` directly.
- Never edit `.taskmaster/tasks/tasks.json` directly.
- Promote durable decisions with `aoc-mem add` only when asked.
