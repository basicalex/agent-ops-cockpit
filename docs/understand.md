# AOC Understand

`aoc-understand` is AOC's bridge to [Understand-Anything](https://github.com/Lum1104/Understand-Anything): generated repository knowledge graphs, guided tours, explain/chat/onboard flows, domain graphs, diff impact analysis, and an interactive dashboard.

It replaces legacy teach-mode workflows for new repository-understanding work.

## Mental model

| Surface | Role |
|---|---|
| **Understand-Anything** | Deep generated code/domain/knowledge graph in `.understand-anything/` plus dashboard and graph-aware skills |
| **AOC Understand** | Safe AOC wrapper for install/status/doctor, project-root routing, and AOC Map sync |
| **AOC Map** | Curated offline visual microsite under `.aoc/map/` |
| **Open Design (`aoc-od`)** | Optional GUI studio for design/prototype artifacts |

Priority now: integrate `aoc-understand` correctly. Advanced AOC Map convergence with Understand-Anything and Open Design is a later phase.

## Commands

```bash
aoc-understand status
aoc-understand doctor
aoc-understand install
```

Project graph flow:

```bash
aoc-understand analyze --full
# run the printed /understand command in Pi when analysis is needed

aoc-understand dashboard --open
aoc-understand chat "How does task routing work?"
aoc-understand explain crates/aoc-cli/src/map.rs
aoc-understand onboard
aoc-understand domain
aoc-understand diff
```

Curated map bridge:

```bash
aoc-understand map-sync
aoc-map serve --open
```

`map-sync` reads `.understand-anything/knowledge-graph.json` and writes a compact AOC Map overview page. It does not replace the full Understand-Anything dashboard.

## Installation model

`aoc-understand install` explicitly clones/updates Understand-Anything into:

```text
~/.local/share/aoc/tools/understand-anything/source
```

Override paths when needed:

```bash
AOC_UNDERSTAND_HOME=/path/to/tools/understand-anything aoc-understand install
aoc-understand --source /path/to/Understand-Anything install
aoc-understand --ref main install
```

AOC does not run remote curl installers. Status and doctor are read-only.

## Teach deprecation

Legacy teach prompts/skills (`/teach`, `/teach-full`, `/teach-dive`, `/teach-ask`, `teach-workflow`) are deprecated for new work. They wrote Markdown notes under `.aoc/insight/` but did not provide a durable structured graph or dashboard.

Do not delete `.aoc/insight/` automatically; old notes may still be useful history.

## Future AOC Map / Open Design convergence

Future work can make `aoc-map` the curated human/agent entrypoint that links:

- Understand-Anything code/domain graph summaries
- Open Design imported artifact metadata from `.aoc/open-design/artifacts.json`
- AOC task/spec/Mind context

That convergence is intentionally not part of the v1 `aoc-understand` priority.
