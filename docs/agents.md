# Agents

AOC is Pi-first. The canonical runtime is the Pi coding agent.

## Runtime contract

Project-local Pi files live under `.pi/`:

```text
.pi/settings.json
.pi/prompts/
.pi/skills/
.pi/extensions/
.pi/packages/
```

Run:

```bash
aoc-init
aoc-skill validate --root .
```

to seed or repair project-local Pi assets.

## Model/auth setup

Use Pi's own model and auth surfaces for provider credentials and model selection. AOC seeds useful project defaults, but it does not want secrets committed to the repo.

Never commit API keys into `.pi/settings.json` or any project file.

## Skills

AOC skills are Pi skills under:

```text
.pi/skills/<name>/SKILL.md
```

Common commands:

```bash
aoc-skill sync --root .
aoc-skill validate --root .
```

See [Skills](skills.md).

## Prompts

Project prompts live under:

```text
.pi/prompts/
```

Examples:

- `tm-cc.md` for cross-project Taskmaster control
- `hyperframes.md` for HyperFrames work

## Extensions

Project Pi extensions live under:

```text
.pi/extensions/
```

They provide AOC surfaces such as presets, Mind/context commands, models, subagents, CodeGraph-assisted code discovery, and UI integration.

### CodeGraph agent tool

AOC includes a default project-local `aoc_codegraph` Pi tool (`.pi/extensions/aoc-codegraph.ts`) for the main AOC agent and selected specialists. It is read-only and shells out to an existing local `codegraph` CLI/index for fast symbol search, context building, call graph probes, impact analysis, file listing, and affected-test selection. Install/update the CLI from `Alt+C -> Tools -> CodeGraph agent index`; AOC installs it globally with `pnpm add -g @colbymchenry/codegraph`. The agent tool itself does not install CodeGraph or initialize/index projects; if CodeGraph is missing or uninitialized, agents should fall back to narrow built-in inspection and report the limitation.

## Subagents

Detached specialist agents live under:

```text
.pi/agents/
```

Use them only through explicit Pi/AOC subagent controls. Reference details live in [reference/subagent-runtime.md](reference/subagent-runtime.md).

## HyperFrames

Run:

```bash
aoc-hyperframes init
```

or:

```text
Alt+C -> Settings -> Tools -> HyperFrames -> Init workspace + campaign factory
```

Then use:

```text
Alt+X -> AOC HyperFrames
```

See [HyperFrames](hyperframes.md).

## Legacy boundary

Older non-Pi runtime paths are not the active AOC surface. Compatibility/history notes live in [Deprecations](deprecations.md) and [archive/](archive/README.md).
