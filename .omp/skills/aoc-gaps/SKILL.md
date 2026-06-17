---
name: aoc-gaps
description: Audit implementation and conceptual gaps by comparing targeted code inspection, Taskmaster tasks/specs, AOC memory/STM decisions, VCS state, and an optional operator focus. Use for broad project gap reviews or directed planning such as `/skill:aoc-gaps mission-control observability`.
allowed-tools: Bash(tm:*), Bash(aoc-task:*), Bash(aoc-mem:*), Bash(aoc-stm:*), Bash(git:*), Bash(python3:*), Bash(jq:*), Bash(rg:*), Bash(find:*), Bash(test:*)
---

# AOC Gaps

Use this skill to find gaps between project intent and implementation reality, then produce a concrete plan to move the project forward.

Invocation forms:

```text
/skill:aoc-gaps
/skill:aoc-gaps mission-control observability
/skill:aoc-gaps voyager onboarding
```

Arguments after the command are the **direction**. No quotes are needed in Pi chat. If no direction is provided, run a broad repo-level gap audit.

## Sources of truth

Compare these layers, escalating only as needed:

1. **Implemented code reality**
   - Use the `aoc_codegraph` tool first when `.codegraph/` exists; otherwise use targeted file/symbol inspection.
   - `git status --short` and recent commits when relevant.
2. **Planned intent**
   - `tm tag current`
   - `tm list --tag <tag>`
   - `aoc-task tag spec show --tag <tag>`
   - `aoc-task spec show <id> --tag <tag>` for task-specific grounding when needed.
3. **Decisions and continuity**
   - `aoc-mem search <direction>` for durable decisions.
   - `aoc-stm status` / `aoc-stm resume` only when active handoff/current continuation matters.
4. **Operator direction**
   - Treat command arguments as authoritative scope unless repo/task evidence conflicts.

## Workflow

### 1. Resolve scope

- Capture direction from user arguments.
- If direction is empty, set scope to `broad`.
- Run:

```bash
tm tag current
git status --short
```

Do not invent code-backed conclusions without local evidence.

### 2. Inspect implementation narrowly

For a directed audit, query `aoc_codegraph` first for `status`, `search`, `context`, `callers`, `callees`, `impact`, or `affected` evidence tied to the direction terms. If `aoc_codegraph` reports CodeGraph missing, stale, uninitialized, or unavailable, fall back to focused searches and bounded file reads. Keep excerpts small.

### 3. Load task/spec intent

Run:

```bash
tag=$(tm tag current)
tm list --tag "$tag"
aoc-task tag spec show --tag "$tag"
```

If specific tasks match the direction, inspect them:

```bash
tm show <id> --tag "$tag"
aoc-task spec show <id> --tag "$tag"
```

### 4. Pull focused memory only when useful

For directed audits:

```bash
aoc-mem search "<direction>"
```

For broad audits, use at most a few focused searches based on the code/task hotspots discovered. Do not load broad memory dumps.

### 5. Compare and classify gaps

Classify findings as:

- **Planned but missing** — task/spec intent exists but no graph/code evidence.
- **Implemented but unplanned** — code exists with no task/spec/provenance.
- **Spec stale** — implementation has moved beyond the documented intent.
- **Decision drift** — memory/STM decisions conflict with current code or tasks.
- **Operational gap** — tests, docs, install/runtime flows, observability, or safety are missing.
- **Conceptual gap** — user/product concept lacks a concrete task/spec/code path.

### 6. Output operational plan

Use this format:

```markdown
# Gap Audit: <direction|broad>

## Current reality
- Graph: present/missing/stale, nodes/edges/layers summary
- Active tag: <tag>
- Relevant code areas: ...
- Relevant task/spec areas: ...

## Aligned
- ...

## Gaps
1. <gap title>
   - Type: planned-missing | implemented-unplanned | spec-stale | decision-drift | operational | conceptual
   - Evidence: code/graph/task/spec/memory references
   - Impact: why it matters
   - Close with: concrete code/doc/test/spec/task action

## Recommended next tasks
- `tm add ...`
- spec/doc update suggestions
- test/check suggestions

## Suggested execution order
1. ...
2. ...
3. ...
```

Keep the plan concise enough to execute. Prefer 3-7 high-signal gaps over a huge inventory.

## Guardrails

- Do not mark tasks complete.
- Do not edit files unless the operator asks to implement the plan.
- Do not read `.aoc/memory.md`, `.aoc/stm/current.md`, or `.taskmaster/tasks/tasks.json` directly; use AOC CLI commands.
- State evidence quality clearly; separate observed code facts from inferred gaps.
