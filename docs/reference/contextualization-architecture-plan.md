# AOC contextualization architecture plan

Status: planning draft  
Scope: make AOC startup/context loading kernel-first, provenance-aware, budgeted, and lazy by default.

## Problem

AOC currently has multiple context producers that can appear equally relevant at startup:

- layered `AGENTS.md` files
- `.aoc/context.md`
- `aoc-handshake --json`
- Taskmaster tasks/specs
- `.aoc/memory.md` through `aoc-mem`
- STM through `aoc-stm`
- Mind context packs/provenance
- `.pi/skills/**/SKILL.md`
- `.pi/prompts/*.md`
- `.pi/extensions/**`
- `.aoc/presets/**`
- `DESIGN.md` and subsystem design docs
- themes/layouts/subagent manifests

These sources need one routing layer. Startup should load the smallest safe kernel and expose everything else as indexed or intent-triggered context.

## Target model

AOC owns a single context router that classifies every context source before agent startup.

### Loading classes

- `always`: compact startup kernel only.
- `index-only`: list/summary visible, body not injected.
- `active-preset`: loaded only while preset/mode is active.
- `intent-triggered`: loaded after user intent matches.
- `manual-only`: loaded only by explicit command/request.
- `never-inject-source`: implementation/source files are discoverable but not prompt context unless editing them.

### Startup kernel

Default startup should include only:

- `.aoc/effective-agent-contract.md` generated from layered `AGENTS.md` files.
- `.aoc/context.md` project snapshot, preferably compacted if it exceeds budget.
- `aoc-handshake --json` metadata.
- active tag/preset/mode metadata.
- tool and context routing policy.
- registry counts/summaries for lazy sources.

### Default non-startup sources

These are not injected by default:

- full `AGENTS.md` chain
- broad Mind memories/context packs
- full Taskmaster task JSON
- full task/spec docs
- skill bodies
- prompt bodies
- extension source
- `DESIGN.md`
- STM archives/current draft
- themes/layouts except metadata

## Context source registry

Represent every source with a small registry record:

```ts
interface ContextSourceRecord {
  id: string;
  kind:
    | "policy"
    | "project_snapshot"
    | "handshake"
    | "task_index"
    | "task_detail"
    | "spec"
    | "memory"
    | "stm"
    | "mind"
    | "skill"
    | "prompt"
    | "extension"
    | "preset"
    | "design_contract"
    | "theme"
    | "subagent";
  source: { path?: string; command?: string };
  loadingClass: "always" | "index-only" | "active-preset" | "intent-triggered" | "manual-only" | "never-inject-source";
  trigger?: string;
  budgetBytes?: number;
  staleCheck?: string;
  provenance: string;
  notes?: string;
}
```

## Initial source matrix

| Source | Class | Default | Trigger |
|---|---:|---|---|
| `.aoc/effective-agent-contract.md` | always | inject | generated from AGENTS chain |
| raw `AGENTS.md` chain | manual-only | do not inject | debugging policy merge |
| `.aoc/context.md` | always | inject compact | startup orientation |
| `aoc-handshake --json` | always | inject metadata | startup |
| `tm tag current` / active task index | always/index-only | compact summary | startup |
| `tm show <id>` | intent-triggered | no | task id / implementation intent |
| `tm tag spec show` / `tm spec show` | intent-triggered | no | spec grounding required |
| `aoc-mem read/search` | intent-triggered/manual-only | no | prior decisions/provenance |
| `aoc-stm resume/read` | intent-triggered/manual-only | no | resume/handoff request |
| Mind context pack | intent-triggered | no | focused reason required |
| `.pi/skills/**/SKILL.md` | index-only | names/descriptions only | skill intent match |
| `.pi/prompts/*.md` | index-only | names only | slash prompt invoked |
| `.pi/extensions/**` | never-inject-source | capability names only | editing/debugging extension |
| `.aoc/presets/*` components | active-preset | no unless active | preset/mode active |
| `DESIGN.md` | intent-triggered | no | UI/product/design/HyperFrames work |
| themes/layouts | index-only | names only | UI/theme/layout work |
| `.pi/agents/**` | index-only | names/capabilities only | explicit subagent dispatch |

## Budget targets

- Total default startup kernel: <= 12 KB.
- Effective AGENTS contract: <= 4 KB.
- Project snapshot: <= 6 KB, with compact fallback.
- Handshake metadata: <= 3 KB.
- Preset prompt injection: <= 4 KB per active preset/mode.
- Mind default: 0 bytes.
- Skill/prompt/extension bodies default: 0 bytes.

## Commands and diagnostics

Extend `aoc-context` as the operator-visible authority:

```bash
aoc-context doctor
aoc-context budget
aoc-context agents
aoc-context agents --write
aoc-context registry
aoc-context registry --json
aoc-context explain-startup
aoc-context stale
aoc-context why <source-id>
```

Diagnostics should report:

- loaded vs indexed vs lazy sources
- source paths/commands
- byte sizes and budget status
- stale/missing generated artifacts
- why each always-loaded source is included
- what would be loaded for active preset/mode

## Implementation phases

### Phase 1: Contract and diagnostics

Implemented initial shell surface:

- `aoc-context agents --write` generates `.aoc/effective-agent-contract.md`.
- `aoc-context doctor` reports raw AGENTS bytes vs generated contract bytes.
- `aoc-context registry` / `aoc-context registry --json` expose modular context source records.
- `aoc-context stale` detects effective AGENTS contract freshness.
- `aoc-context explain-startup` and `aoc-context why <source-id>` explain routing decisions.

Next:

- add budget warnings with stricter pass/fail thresholds.
- integrate generated contract refresh into `aoc-init`/launch.

### Phase 2: Startup integration

Implemented initial managed-Pi path:

- `aoc-agent-wrap` defaults managed Pi launches to `AOC_PI_CONTEXT_KERNEL=on`.
- Startup refreshes `.aoc/effective-agent-contract.md` when stale/missing.
- Pi is launched with `--no-context-files` and a generated `--append-system-prompt` kernel containing the effective contract, compact project snapshot, router explanation, and metadata-only handshake.
- If generation fails, startup fails open to Pi raw context-file discovery with a warning.

Remaining hardening:

- Integrate contract refresh into `aoc-init`.
- Move shell implementation toward a reusable router module if additional runtimes need it.

### Phase 3: Context router enforcement

Implemented initial enforcement in the managed-Pi path:

- Raw AGENTS chain is disabled by `--no-context-files` when the generated kernel is active.
- High-volume sources remain index-only/intent-triggered/manual-only in registry and are not body-injected by the generated kernel.

Remaining hardening:

- Extend the same router path to non-Pi runtimes if needed.
- Add stricter startup budget failures after observing real launch sizes.

### Phase 4: Preset and task/spec routing

- Connect preset state to router records.
- Make active preset components explainable and budgeted.
- Keep task/spec details on-demand with provenance.

### Phase 5: Tests and acceptance

Add tests proving:

- raw AGENTS chain is not injected when effective contract is fresh.
- stale/missing contract is detected.
- project overrides survive contract generation.
- default kernel stays under budget.
- skills/prompts/extensions/Mind are not body-injected by default.
- active preset injects only selected preset components.
- task/spec details load only on intent.

## Acceptance criteria

A clean startup can answer:

> What rules apply, where am I, what capabilities exist, and how do I fetch more safely?

It must not load:

> Every skill, prompt, extension, memory, task, spec, theme, design doc, and raw policy file.
