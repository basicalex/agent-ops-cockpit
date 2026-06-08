# Open Design studio integration

Open Design (OD) is AOC's optional GUI-first design studio bridge.

AOC's design presets are good at **routing design intent**. OD is better at **seeing, iterating, previewing, and exporting design artifacts**. The intended polished workflow is:

```text
OD creates/iterates visual artifacts → AOC imports them → AOC implements, tracks, proves, and scales them into HyperFrames campaigns or html-video storyboards
```

## Why this matters

AOC's biggest design-quality gap is that terminal-only prompts do not provide a real visual feedback loop. OD provides the missing studio surface:

- design-system picker and `DESIGN.md` workflow
- prototype/deck/template/design-system modes
- sandboxed live preview
- visual artifact iteration
- HTML/PDF/PPTX/MP4-oriented outputs
- agent-driven design generation with project files

AOC then adds what OD does not try to own:

- repo/task/spec/STM/Mind provenance
- project integration and implementation handoff
- durable imported artifact paths
- HyperFrames campaign scale-out
- AOC preset/session context

## Mental model

| Layer | Owns | Does not own |
|---|---|---|
| **Open Design** | GUI design studio, prototypes, decks, templates, design systems, preview/export loop | AOC tasks, STM/Mind, project operating contract |
| **AOC** | Project OS, context, tasks/specs, provenance, install/run/import bridge, implementation/campaign handoff | Rebuilding OD's GUI design studio |
| **HyperFrames** | Durable project-local video/campaign factory, reusable media assets, renders, shotlists | Generic UI design iteration |
| **html-video** | Motion/storyboard meta-layer, ContentGraph, template/studio flow, local MP4 export through HyperFrames | AOC approvals, provenance, or brand strategy source of truth |

Short version:

```text
OD = make it look good
AOC = make it real, tracked, reusable, scalable
HyperFrames = turn polished direction into campaign/media source
html-video = turn campaign beats into short-form storyboard/video flow
```

## Install once

```bash
aoc-od install
```

Default global checkout:

```text
~/.local/share/aoc/tools/open-design/source
```

Override when needed:

```bash
AOC_OD_HOME=/path/to/tools/open-design aoc-od install
AOC_OD_REF=<git-ref> aoc-od install
AOC_OD_REPO_URL=https://github.com/nexu-io/open-design.git aoc-od install
```

`install` is explicit because OD runs a local daemon and can spawn local agent CLIs. AOC does not silently install, start, or configure OD.

## Start OD from a project

```bash
cd <project>
aoc-od start --open
```

This starts OD's managed web runtime with `OD_DATA_DIR=<project>/.od` and writes:

```text
.aoc/open-design/link.json
```

OD still creates its per-chat agent workspaces under:

```text
<project>/.od/projects/<id>/
```

AOC applies compatibility patches during `aoc-od install`/`aoc-od patch` so OD can use the project root as a linked Codex directory, detect the live web port, expose current GPT model IDs, and use Codex OAuth image generation from prototype projects.

By default AOC starts OD with `OD_CODEX_SANDBOX=danger-full-access` because Codex/bwrap sandboxing can fail inside OD on some Linux hosts. This allows OD/Codex to read and write the linked repo. Review `git status` and `git diff` before keeping OD-generated changes. Set `AOC_OD_CODEX_SANDBOX=workspace-write` to retry sandboxed mode.

The link file tells AOC/agents where OD fits in the current project:

- project root
- OD source checkout
- OD URL
- root `DESIGN.md` if present
- OD artifact source path: `.od/artifacts`
- AOC import target: `design-artifacts/od`
- HyperFrames design handoff target: `hyperframes/docs/DESIGN.md`

Foreground run:

```bash
aoc-od run --open
```

Status/health/lifecycle:

```bash
aoc-od status
aoc-od doctor
aoc-od stop
aoc-od open
```

## Provider/auth model

Open Design supports BYOK/OpenAI-compatible provider configuration and local agent adapters. AOC v1 does **not** silently configure OpenAI OAuth, API keys, or provider secrets.

Preferred future path:

```text
AOC auth/proxy → local OpenAI-compatible endpoint → OD provider config
```

Until that bridge is verified, configure OD providers inside OD or run OD through a local agent adapter that already has auth.

Security warning: Do not put API keys, OAuth tokens, cookies, or private credentials in `.aoc/open-design/link.json`, specs, STM, commits, or imported artifact metadata.

## Recommended design journey

```bash
cd <project>
aoc-od start --open
# use OD GUI: choose design system/skill, generate, preview, refine, export/save
aoc-od import latest
# use AOC to implement, task, document, or scale into HyperFrames
```

Use OD for:

- landing page visual direction
- app screen prototypes
- brand/design-system exploration
- pitch decks and sales decks
- templates and document-like artifacts
- high-quality design references before implementation

Use AOC after OD for:

- converting a polished design into Taskmaster implementation work
- promoting reviewed OD `DESIGN.md` into root `DESIGN.md`
- linking artifact paths in specs/tasks/STM/Mind
- turning approved direction into HyperFrames campaign assets
- preserving decisions and proof in commits/release notes

## Import polished artifacts

After iterating in OD, save/export the artifact under project `.od/artifacts/`, then run:

```bash
aoc-od import latest
```

AOC copies the newest artifact to:

```text
design-artifacts/od/<artifact>/
```

AOC also writes:

```text
design-artifacts/od/<artifact>/aoc-open-design-artifact.json
.aoc/open-design/artifacts.json
```

Import a specific artifact:

```bash
aoc-od import .od/artifacts/2026-05-homepage-v1
```

## Artifact contract

Treat these as runtime/generated unless explicitly promoted:

```text
.od/**
```

Treat these as reviewable project evidence when useful:

```text
design-artifacts/od/**
.aoc/open-design/link.json
.aoc/open-design/artifacts.json
```

An imported artifact is safe to reference from:

- Taskmaster tasks/specs
- implementation tickets
- design review notes
- HyperFrames shotlists/briefs
- commit trailers
- STM/Mind provenance entries

## Handoff into implementation

Typical implementation handoff:

1. Import OD artifact.
2. Inspect `design-artifacts/od/<artifact>/index.html` or exported files.
3. Add/align Taskmaster task acceptance criteria with the artifact.
4. Promote any approved design-system rules into root `DESIGN.md`.
5. Implement in app code.
6. Verify against the OD artifact and root `DESIGN.md`.

Recommended task note:

```text
Design reference: design-artifacts/od/<artifact>/index.html
Design contract: DESIGN.md
```

## Handoff into HyperFrames

OD can establish the visual direction. HyperFrames turns it into a reusable campaign/media system.

Workflow:

```bash
aoc-od import latest
# review imported artifact and DESIGN.md
aoc-hyperframes bootstrap-asset-system
aoc-hyperframes campaign create <slug> --audience <audience> --channels meta,reel --durations 15s,6s --concept <concept>
```

Then adapt/import the OD direction into:

```text
hyperframes/docs/DESIGN.md
hyperframes/docs/brand-motion-brief.md
hyperframes/docs/campaign-message-matrix.md
hyperframes/assets/brand/**
hyperframes/compositions/campaigns/**
```

When the approved direction should become a multi-frame short-form video, use html-video as the motion/storyboard counterpart:

```bash
aoc-html-video project create --from hyperframes/docs/content-campaign-plan.md
aoc-html-video project preview <project-id>
```

Open Design remains the visual exploration surface; html-video handles storyboard/template/studio/render orchestration.

Use AOC HyperFrames checks before final render:

```bash
aoc-hyperframes check
```

## Presets relationship

`Alt+X -> Design` remains useful for critique/spec/handoff during implementation.

`aoc-od` is not a preset. It is the external GUI studio bridge. Use it before or alongside design presets:

```text
OD GUI exploration → import artifact → Alt+X Design critique/spec/handoff → implement
```

`Alt+X -> HyperFrames` remains the project media/campaign operating mode after OD direction is approved.

## Commands

```bash
aoc-od install        # explicit global OD clone/install/build
aoc-od status         # read-only status
aoc-od doctor         # read-only prerequisites/install health
aoc-od start --open   # background OD web runtime + project link
aoc-od run --open     # foreground OD web runtime + project link
aoc-od stop           # stop OD runtime
aoc-od open           # open OD URL
aoc-od link           # write .aoc/open-design/link.json
aoc-od import latest  # import newest .od/artifacts/*
aoc-od import PATH    # import specific artifact dir
```

## Safety rules

- Do not commit `.od/` runtime data by default unless a project explicitly wants it.
- Commit imported `design-artifacts/od/**` only when the artifact is intended as project evidence.
- Do not store API keys or OAuth tokens in `.aoc/open-design/link.json`.
- Do not overwrite root `DESIGN.md` from OD without review.
- Do not treat OD output as final implementation without accessibility/responsive QA.
- Keep OD install/start explicit because it runs a daemon and may spawn agent CLIs.

## Current limitations

- OpenAI OAuth reuse is not wired automatically yet.
- `aoc-od install` tracks `main` by default unless `AOC_OD_REF` is set.
- Deep bidirectional sync is not implemented; v1 is install/run/link/import.
- AOC does not yet auto-create HyperFrames campaigns from OD artifacts; operator/agent performs the handoff through `aoc-hyperframes` and, for storyboard videos, `aoc-html-video`.

## Future polish

Likely next upgrades:

- pin a tested OD commit/version by default
- AOC-managed OpenAI-compatible auth proxy for OD
- `aoc-od promote-design` to review/promote OD `DESIGN.md`
- `aoc-od handoff hyperframes` to seed HyperFrames docs from imported artifacts
- preset UI affordance pointing Design users to OD for visual iteration
