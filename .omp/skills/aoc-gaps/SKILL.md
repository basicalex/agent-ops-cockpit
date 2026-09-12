---
name: aoc-gaps
description: Audit implementation and conceptual gaps through targeted code inspection, VCS state, and an optional operator focus. Use for broad project gap reviews or directed planning such as `/skill:aoc-gaps mission-control observability`.
allowed-tools: Bash(git:*), Bash(python3:*), Bash(jq:*), Bash(rg:*), Bash(find:*), Bash(test:*)
---

# AOC Gaps

Use this skill to find gaps between project intent and implementation reality, then produce a concrete plan to move the project forward.

Invocation forms:

```text
/skill:aoc-gaps
/skill:aoc-gaps mission-control observability
/skill:aoc-gaps acme onboarding
```

Arguments after the command are the **direction**. No quotes are needed in Pi chat. If no direction is provided, run a broad repo-level gap audit.

## Sources of truth

Compare these layers, escalating only as needed:

1. **Implemented code reality**
   - Use the `aoc_codegraph` tool first when `.codegraph/` exists; otherwise use targeted file/symbol inspection.
   - `git status --short` and recent commits when relevant.
2. **Operator direction**
   - Treat command arguments as authoritative scope unless repository evidence conflicts.

## Workflow

### 1. Resolve scope

- Capture direction from user arguments.
- If direction is empty, set scope to `broad`.
- Run `git status --short`.

Do not invent code-backed conclusions without local evidence.

### 2. Inspect implementation narrowly

For a directed audit, query `aoc_codegraph` first for `status`, `search`, `context`, `callers`, `callees`, `impact`, or `affected` evidence tied to the direction terms. If `aoc_codegraph` reports CodeGraph missing, stale, uninitialized, or unavailable, fall back to focused searches and bounded file reads. Keep excerpts small.


### 3. Compare and classify gaps

Classify findings as:

- **Planned but missing** — stated intent exists but no graph/code evidence.
- **Implemented but unplanned** — code exists with no documented intent/provenance.
- **Spec stale** — implementation has moved beyond the documented intent.
- **Intent drift** — current code conflicts with documented intent.
- **Operational gap** — tests, docs, install/runtime flows, observability, or safety are missing.
- **Conceptual gap** — user/product concept lacks a concrete code path.

### 5. Output operational plan

Use this format:

```markdown
# Gap Audit: <direction|broad>

## Current reality
- Graph: present/missing/stale, nodes/edges/layers summary
- Relevant code areas: ...
- Relevant documented intent: ...

## Aligned
- ...

## Gaps
1. <gap title>
   - Type: planned-missing | implemented-unplanned | spec-stale | intent-drift | operational | conceptual
   - Evidence: code/graph/documentation references
   - Impact: why it matters
   - Close with: concrete code/doc/test action

## Recommended next actions
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
- State evidence quality clearly; separate observed code facts from inferred gaps.
