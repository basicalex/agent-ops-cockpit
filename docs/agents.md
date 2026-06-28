# Agents

AOC is Herdr/OMP-first. OMP is the active coding-agent runtime and owns subagent orchestration. AOC provides project context, prompts, extensions, agent templates, and compatibility assets.

## Runtime contract

AOC is OMP-only for active coding-agent runtime/config. Repo-owned OMP source surfaces live under:

```text
.omp/extensions/
.omp/agents/
.omp/skills/
.aoc/
```

`aoc-init` and `aoc-herdr-install` sync the active `.omp/manifest.toml` profile surface into the active OMP runtime directory:

```text
${AOC_OMP_AGENT_DIR:-~/.omp/agent}/extensions/
${AOC_OMP_AGENT_DIR:-~/.omp/agent}/agents/
${AOC_OMP_AGENT_DIR:-~/.omp/agent}/skills/
```

Treat `~/.omp/agent` and `~/.omp/agent/config.yml` as user/operator runtime state; do not commit it. Legacy Pi runtime assets have been purged; do not use Pi paths as active runtime evidence.

Run:

```bash
aoc-init
aoc-skill validate --root .
```

to seed or repair project-local AOC assets and sync the extensions, agent templates, and skills selected by active OMP capability profiles.


AOC uses VoxType, not an OMP speech-to-text extension, for operator dictation. `aoc-init` and `aoc-herdr-install` install `voxtype-aoc-lexicon-filter`, seed `~/.config/aoc/voxtype-lexicon.md`, and wire VoxType post-processing so system and active-project `.aoc/lexicon.md` terms normalize after transcription.

## Model/auth setup

Use OMP's own model and auth surfaces for provider credentials and model selection. AOC seeds useful defaults and agent manifests, but it must not commit secrets.

Never commit API keys into `.omp/**`, OMP runtime config files, `.aoc/**`, docs, prompts, or manifests.

## Skills

AOC skills are project source documentation under:

```text
.omp/skills/<name>/SKILL.md
```

Common commands:

```bash
aoc-skill sync --root .
aoc-skill validate --root .
```

Useful former prompt workflows are now OMP skills (`aoc-update`, `aoc-lexicon`, `aoc-stm`). No project prompt registry is active.

See [Skills](skills.md).

## OMP extensions

AOC OMP extensions are repo-tracked under `.omp/extensions/`. `.omp/manifest.toml` keeps the full extension inventory plus profile tables; active profiles decide which extensions are synced to the OMP runtime extension directory.

Current OMP surfaces include:

- `aoc-codegraph.ts` — read-only CodeGraph tool for indexed code discovery.
- `aoc-mind.ts` — read-only AOC Mind evidence/provenance tool.
- `aoc-commit.ts` — `/commit` safe atomic Git commit workflow; stages only explicit paths and never pushes without explicit approval.
- `aoc-state.ts` — `/state-status`, `/state-commit`, and `/state-push` Git workflows for repo-owned AOC project state; commit and push are separate, explicit steps.
- `aoc-brand-content.ts` — `/brand-content` and `/hyperframes-director` HyperFrames branded-content modes.
- `aoc-dox.ts` — `aoc_dox` safe metadata tool for DOX metadata, review, doctor, eval, and apply dry-run.
- `aoc-dox-command.ts` — `/dox` slash command for sparse AGENTS cartography with `dox-*` subagents.
- `aoc-web-search.ts` — `aoc_web_search` wrapper around local `aoc-search`/SearXNG plus direct package/GitHub lookup modes for agents when built-in paid web-search providers fail.
- `aoc-style.ts` — `/ponytail off|lite|full|ultra|status|review|audit|debt|help`, `/caveman off|lite|full|ultra|status`, and persistent AOC host-style hook state.
- `aoc-profile.ts` — `/profile [list|show|enable|disable|set|explain]` capability profile management.

Legacy Pi runtime assets have been purged; do not use Pi paths as active runtime evidence.

## OMP subagents

AOC-managed OMP agent templates live in the repo under:

```text
.omp/agents/
```

`aoc-init` and `aoc-herdr-install` copy them into:

```text
${AOC_OMP_AGENT_DIR:-~/.omp/agent}/agents/
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

Legacy OMP runtime assets were removed from the active AOC operator path. The active path is Herdr + OMP.
