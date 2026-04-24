# HyperFrames

HyperFrames is AOC's preferred optional video-authoring workflow for agent-created videos.

Use HyperFrames for rendered video work: HTML compositions, CSS layout, GSAP timelines, captions, narration, transitions, preview, lint, and render flows.

## Setup

From a repo with AOC initialized:

```bash
aoc-hyperframes init
```

By default this:

- initializes a generated HyperFrames workspace in `hyperframes/` using `npx hyperframes init hyperframes --non-interactive`
- adds `hyperframes/` to `.gitignore`
- seeds HyperFrames PI skills into `.pi/skills/`
- seeds the PI prompt `.pi/prompts/hyperframes.md`
- runs AOC skill validation/sync when available

## Control surface

Use `Alt+C -> Settings -> Tools -> HyperFrames` for setup and maintenance:

- initialize workspace + sync skills
- sync skills only
- run doctor
- start preview pane (`cd hyperframes && npx hyperframes preview`)

## Preview UI

After initialization, use:

```text
Alt+C -> Settings -> Tools -> HyperFrames -> Start preview pane
```

Inside Zellij this opens a pane below and runs:

```bash
cd hyperframes
npx hyperframes preview
```

The preview UI usually serves at `http://localhost:3002`; use the URL printed by the command if different.

## Preset surface

Use `Alt+X -> HyperFrames` to switch the agent into video mode.

Modes:

- `compose` — create/edit HyperFrames videos
- `site` — website-to-video workflow
- `cli` — setup/lint/preview/render/doctor
- `review` — audit/fix an existing HyperFrames project

## Skill separation

HyperFrames video mode uses:

- `hyperframes`
- `hyperframes-cli`
- `website-to-hyperframes`
- `gsap`

Anime.js skills remain for frontend/site UI animation and should not be mixed into HyperFrames video work unless the user explicitly asks for non-video frontend animation.

## Requirements

- Node.js >= 22
- FFmpeg

Run:

```bash
aoc-hyperframes doctor
```

for local checks.
