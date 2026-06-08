# Agents

AOC is Herdr/OMP-first. OMP is the active coding-agent runtime and owns subagent orchestration. AOC provides project context, prompts, extensions, agent templates, and compatibility assets.

## Runtime contract

Project-local compatibility/source files still live under `.pi/` because AOC reuses those skills, prompts, and extension sources as managed project content:

```text
.pi/settings.json
.pi/prompts/
.pi/skills/
.pi/extensions/
.pi/packages/
```

Repo-owned OMP source surfaces live under `.omp/`:

```text
.omp/extensions/
.omp/agents/
```

`aoc-init` and `aoc-herdr-install` sync those sources into the active OMP runtime directory:

```text
${PI_CODING_AGENT_DIR:-~/.omp/agent}/extensions/
${PI_CODING_AGENT_DIR:-~/.omp/agent}/agents/
```

The environment variable name is legacy. The target is the OMP agent runtime directory. Treat `~/.omp/agent` as user/runtime state; do not commit it.

Run:

```bash
aoc-init
aoc-skill validate --root .
```

to seed or repair project-local AOC assets and sync OMP extensions/agent templates.

AOC reapplies an OMP package footer patch during `aoc-init`, `aoc-herdr-install`, and `aoc omp` launch so jj repositories show `Δfiles +added -removed ⇢bookmarks` instead of Git detached branch state.

## Model/auth setup

Use OMP's own model and auth surfaces for provider credentials and model selection. AOC seeds useful defaults and agent manifests, but it must not commit secrets.

Never commit API keys into `.pi/settings.json`, `.omp/**`, OMP runtime config files, `.aoc/**`, docs, prompts, or manifests.

## Skills and prompts

AOC skills remain project source documentation under:

```text
.pi/skills/<name>/SKILL.md
```

Common commands:

```bash
aoc-skill sync --root .
aoc-skill validate --root .
```

Project prompts live under:

```text
.pi/prompts/
```

Examples:

- `tm-cc.md` for cross-project Taskmaster control
- `hyperframes.md` for HyperFrames compatibility/source prompts

See [Skills](skills.md).

## OMP extensions

AOC OMP extensions are repo-tracked under `.omp/extensions/` and synced to the OMP runtime extension directory.

Current OMP surfaces include:

- `aoc-codegraph.ts` — read-only CodeGraph tool for indexed code discovery.
- `aoc-mind.ts` — read-only AOC Mind evidence/provenance tool.
- `aoc-commit.ts` — `/commit` safe atomic commit workflow for Git-only and Jujutsu repositories; it follows detected VCS metadata and never pushes without explicit approval.
- `aoc-state.ts` — `/state-status`, `/state-commit`, and `/state-push` workflows for repo-owned AOC project state; commit and push are separate, explicit steps.
- `aoc-jj-init.ts` — `/jj-init` explicit workflow for initializing colocated Jujutsu over an existing Git repo after dirty-work inspection.
- `aoc-brand-content.ts` — `/brand-content` and `/hyperframes-director` HyperFrames branded-content modes.
- `aoc-web-search.ts` — `aoc_web_search` wrapper around local `aoc-search`/SearXNG plus direct package/GitHub lookup modes for agents when built-in paid web-search providers fail.

Project-local `.pi/extensions/` remains a compatibility/source surface. Do not base new operator workflows on legacy Pi subagent controls when an OMP extension exists.

## OMP subagents

AOC-managed OMP agent templates live in the repo under:

```text
.omp/agents/
```

`aoc-init` and `aoc-herdr-install` copy them into:

```text
${PI_CODING_AGENT_DIR:-~/.omp/agent}/agents/
```

The branded content pipeline provides:

- `brand-strategy` — brand soul, audience, voice, visual world, off-brand boundaries.
- `brand-concept` — campaign directions and GPT Image 2 prompt packs.
- `svg-asset` — clean SVG specs/code from approved image regions.
- `hyperframes-content` — html-video/HyperFrames storyboard, composition, and shotlist specs.

These specialists initially produce exact specs and target paths; the primary OMP agent/operator applies writes after approval.

## CodeGraph agent tool

AOC includes an OMP `aoc_codegraph` tool for read-only symbol search, context building, call graph probes, impact analysis, file listing, and affected-test selection. The tool shells out to an existing local `codegraph` CLI/index. It never installs CodeGraph or initializes/indexes projects.
This is the replacement graph surface for agents; do not use AOC Understand / Understand-Anything for repository graph evidence.

Operators run CodeGraph setup explicitly, for example:

```bash
codegraph sync /path/to/project
```

## HyperFrames and branded content

Run:

```bash
aoc-hyperframes init
# or only the branded pipeline docs/assets
aoc-hyperframes brand init --brand <brand-slug>
```

Then use OMP commands:

```text
/brand-content strategy
/brand-content concepts
/brand-content image
/brand-content review
/brand-content svg
/brand-content campaign
/hyperframes-director campaign
```

See [HyperFrames](hyperframes.md) and [html-video](html-video.md).

## Legacy boundary

Legacy Pi runtime assets remain for compatibility and source reuse. The active AOC operator path is Herdr + OMP, not the legacy Pi subagent manager or Zellij cockpit.
