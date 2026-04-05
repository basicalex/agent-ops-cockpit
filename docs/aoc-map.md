# AOC Map

`aoc-map` is the canonical AOC mapping system.

It turns `.aoc/map/` into a **project-local graph microsite** for a repo: graph pages, Mermaid sources, a generated homepage, and a local server for browsing the whole thing as a small site.

## Canonical naming

AOC Map is the canonical surface now:

- command: `aoc-map`
- subcommand: `aoc map ...`
- workspace: `.aoc/map/`
- skill: `.pi/skills/aoc-map/SKILL.md`

Legacy compatibility still exists during transition:

- `aoc see ...` still resolves to the map command surface
- old `.aoc/see/` workspaces migrate to `.aoc/map/`
- old `.aoc/diagrams/` workspaces also migrate to `.aoc/map/`
- old page metadata such as `aoc-see:*` and `data-aoc-see-*` is normalized to `aoc-map:*` and `data-aoc-map-*` during build/migration flows

## Workspace layout

```text
.aoc/map/
  README.md
  manifest.json
  index.html
  assets/
    mermaid.min.js
    render-mermaid.js
  diagrams/
    agent-topology.mmd
    task-flow.mmd
  pages/
    agent-topology.html
    task-flow.html
```

Purpose of each part:

- `pages/*.html` — graph-first pages
- `diagrams/*.mmd` — canonical Mermaid graph sources
- `assets/*` — vendored local Mermaid runtime/helpers
- `manifest.json` — site/page metadata
- `index.html` — generated homepage
- `README.md` — local usage guidance for the repo

## How it gets seeded

You can seed or refresh AOC Map in three main ways:

### CLI

```bash
aoc-map init
aoc-map build
aoc-map serve --open
```

### Through `aoc-init`

```bash
aoc-init
aoc-init --force
```

`aoc-init` now seeds the AOC Map skill and workspace as part of repo setup.

### Through Alt+C

Path:

- `Alt+C -> Settings -> Tools -> AOC Map microsite`

That action:

- syncs `.pi/skills/aoc-map/SKILL.md`
- runs `aoc-map init`
- seeds or confirms `.aoc/map/`
- migrates older AOC See workspaces when needed

## Core commands

```bash
aoc-map init
aoc-map new agent-topology --section agents --kind topology --summary "How AOC agents route through this repo"
aoc-map list
aoc-map build
aoc-map serve --port 43111 --open
```

## Homepage behavior

The generated homepage is intentionally compact.

It now provides:

- a minimal `AOC Map` title area
- total page count
- total diagram count
- a search box
- a closed-by-default filters dropdown
- one filtered page list
- a compact recent-updates sidebar

### UX rules

The homepage should stay low-clutter:

- page cards should not show visible tags or source lists
- recent updates should stay compact and title-first
- filters should stay collapsed by default
- pages should appear in one main list, not repeated across multiple homepage sections
- the hero should remain minimal and metrics-only

## Typical workflow

1. Run `aoc-map init` once.
2. Create a page with `aoc-map new <slug>`.
3. Edit the generated Mermaid file in `.aoc/map/diagrams/`.
4. Keep the page in `.aoc/map/pages/` minimal and visual-first.
5. Run `aoc-map build` to refresh the homepage and sync assets.
6. Use `aoc-map serve` to browse locally.

Example:

```bash
aoc-map new task-flow \
  --section tasks \
  --kind flow \
  --summary "Task lifecycle and dependency flow" \
  --tags taskmaster,workflow \
  --source .taskmaster/tasks/tasks.json \
  --source docs/plan.md
```

## Page scaffolding

`aoc-map new` creates both:

- a page shell in `pages/`
- a Mermaid source file in `diagrams/`

The default scaffold is visual-first:

- primary graph first
- summary second
- supporting notes minimal
- local Mermaid assets already wired

## Mermaid rendering

Preferred pattern:

```html
<script type="text/plain" data-aoc-map-mermaid-src="../diagrams/agent-topology.mmd"></script>
```

Inline Mermaid blocks still work, but external `.mmd` files are preferred.

AOC Map uses vendored repo-local Mermaid JS assets under `.aoc/map/assets/`, so pages render offline without CDNs.

## Metadata discovery

Pages can self-declare metadata:

```html
<meta name="aoc-map:summary" content="How the session lifecycle works">
<meta name="aoc-map:section" content="ops">
<meta name="aoc-map:kind" content="timeline">
<meta name="aoc-map:status" content="active">
<meta name="aoc-map:diagram" content="diagrams/session-lifecycle.mmd">
<meta name="aoc-map:tags" content="sessions,lifecycle,ops">
```

Supported metadata fields include:

- `summary`
- `section`
- `kind`
- `status`
- `diagram`
- `tags`

The manifest can also store:

- `source_paths`
- `featured`
- `generated`
- `order`
- timestamps

## Authoring guidance

- Prefer Mermaid files under `.aoc/map/diagrams/`.
- Keep pages graph-first.
- Keep prose secondary.
- Avoid external assets when possible.
- Prefer updating an existing page over making overlapping duplicates.
- Treat AOC Map as the repo’s visual explanation layer.

## Migration notes

If a repo still has old AOC See content:

- `.aoc/see/` is migrated to `.aoc/map/`
- `.aoc/diagrams/` is migrated to `.aoc/map/`
- `.pi/skills/aoc-see/` is replaced by `.pi/skills/aoc-map/`
- old page attributes are normalized to the new `aoc-map` names

For a stale older repo, the safest refresh path is usually:

```bash
aoc-init --force
```

or:

```bash
aoc-map init --force
```
