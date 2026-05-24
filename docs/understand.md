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

## Alt+C setup path

In any initialized AOC project:

1. Press `Alt+C`
2. Open **Settings**
3. Open **Tools**
4. Open **AOC Understand**

Available actions mirror the CLI wrapper and run as background jobs with logs in the right-hand detail pane:

- **Status** — `aoc-understand status`
- **Run doctor** — `aoc-understand doctor`
- **Install/update Understand-Anything** — `aoc-understand install`
- **Analyze guidance** — `aoc-understand analyze --full`
- **Open dashboard** — `aoc-understand dashboard --open`
- **Sync graph to AOC Map** — `aoc-understand map-sync`

Status, doctor, and analyze guidance do not install anything implicitly. The install/update action is the explicit network step.

## Commands

```bash
aoc-understand status
aoc-understand doctor
aoc-understand install
```

Project graph flow:

```bash
aoc-understand analyze --full
# run the printed /skill:understand command in Pi chat when analysis is needed

aoc-understand dashboard --open
aoc-understand chat "How does task routing work?"
aoc-understand explain crates/aoc-cli/src/map.rs
aoc-understand onboard
aoc-understand domain
aoc-understand diff
aoc-understand gaps mission-control observability
```

Curated map bridge:

```bash
aoc-understand map-sync
aoc-map serve --open
```

`map-sync` reads `.understand-anything/knowledge-graph.json` and writes a compact AOC Map overview page. It does not replace the full Understand-Anything dashboard.

## Post-commit graph refresh

AOC can install an opt-in, project-local `post-commit` hook that enqueues a background graph refresh after each implementation commit:

```bash
aoc-understand hook install
aoc-understand hook status
aoc-understand hook uninstall
```

The hook writes only to the current project's `.git/hooks/post-commit` and logs to `.aoc/logs/understand-refresh.log`. It sets `AOC_UNDERSTAND_REFRESH=1` to avoid recursion, then runs:

```bash
aoc-understand --root "$PWD" refresh --source-commit <sha> --commit
```

`refresh` is intentionally constrained. It refuses to auto-commit when unrelated dirty files exist, stages only generated graph artifacts, and creates a follow-up commit such as `chore(graph): refresh repository knowledge graph`. By default it syncs an existing `.understand-anything/knowledge-graph.json` into AOC Map. If a stable noninteractive analyzer is available, configure it explicitly with `AOC_UNDERSTAND_REFRESH_CMD`; otherwise use `/skill:understand --full` for graph regeneration and let the hook keep map artifacts aligned.

## Gap audits

Use the AOC gap skill to compare implemented code reality with Taskmaster tasks/specs, AOC memory/STM decisions, git state, and an optional operator direction:

```text
/skill:aoc-gaps
/skill:aoc-gaps mission-control observability
/skill:aoc-gaps voyager onboarding
```

The wrapper can print the same Pi command:

```bash
aoc-understand gaps
aoc-understand gaps mission-control observability
```

Broad audits find repo-level conceptual and operational gaps. Directed audits focus graph/task/memory searches on the provided direction and return a concrete closure plan.

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
