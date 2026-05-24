---
name: aoc-understand
description: Use AOC's Understand-Anything bridge for repository knowledge graphs, onboarding, architecture questions, explain/chat flows, and dashboard exploration. This replaces legacy teach workflows.
allowed-tools: Bash(aoc-understand:*), Bash(test:*), Bash(python3:*), Bash(rg:*), Bash(grep:*)
---

# AOC Understand

Use `aoc-understand` when the user wants to understand a repo, onboard, ask architecture questions, inspect graph relationships, explain a file/function, analyze code impact, or launch an interactive knowledge graph dashboard.

## Canonical flow

1. Check status:
   ```bash
   aoc-understand status
   ```
2. If Understand-Anything is not installed and the user explicitly wants setup:
   ```bash
   aoc-understand install
   ```
3. Build/update the graph through the UA agent skill flow:
   ```bash
   aoc-understand analyze --full
   # then run the printed /understand command in Pi if needed
   ```
4. Explore:
   ```bash
   aoc-understand dashboard --open
   aoc-understand chat "How does auth work?"
   aoc-understand explain src/file.ts
   aoc-understand onboard
   aoc-understand domain
   aoc-understand diff
   ```
5. Optional curated AOC Map bridge:
   ```bash
   aoc-understand map-sync
   aoc-map serve --open
   ```

## Mental model

- Understand-Anything owns the deep generated knowledge graph under `.understand-anything/`.
- AOC owns safe install/status/doctor/project-root routing through `aoc-understand`.
- AOC Map remains the curated offline visual microsite under `.aoc/map/`.
- Open Design remains the GUI design studio bridge through `aoc-od`.

## Teach deprecation

Do not start new work with `/teach`, `/teach-full`, `/teach-dive`, or `teach-workflow` unless the operator explicitly asks to inspect legacy artifacts. Use `aoc-understand` instead.

## Guardrails

- Do not run remote curl installers. Use `aoc-understand install` for explicit clone/update.
- Do not load the full `.understand-anything/knowledge-graph.json` into context. Search/slice it first.
- Do not delete `.aoc/insight/`; legacy teach notes may still be useful history.
- Treat `aoc-understand map-sync` as a curated bridge, not a replacement for the full dashboard.
