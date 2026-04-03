# AOC See

`aoc-see` turns `.aoc/diagrams/` into a **project-local visualization microsite** for any repo using AOC.

Instead of treating diagrams as isolated files, AOC See treats them as pages in a small browsable site for the codebase:

- architecture explainers
- agent and subagent maps
- task and workflow views
- Mind/provenance walkthroughs
- ops/runbook dashboards
- research or visual notes

## Core idea

Each repo gets a local website layer:

```text
.aoc/diagrams/
  README.md
  manifest.json
  index.html          # generated homepage / gallery / site shell
  pages/
    agent-topology.html
    task-flow.html
    provenance-map.html
    session-lifecycle.html
```

`aoc-see serve` then serves that folder as a real microsite, not just a raw directory listing.

## Commands

```bash
aoc-see init
aoc-see new agent-topology --section agents --kind topology --summary "How AOC agents route through this repo"
aoc-see list
aoc-see build
aoc-see serve --port 43111 --open
```

You can also seed/confirm the microsite from the floating control pane:

- `Alt+C -> Settings -> Tools -> AOC See microsite`

## What the homepage provides

The generated `index.html` is a site shell with:

- hero / repo intro
- collection navigation
- counts for pages, collections, kinds, and tags
- search box
- filters for collection, kind, and tag
- featured pages
- recent updates
- grouped collection sections

Default collections:

- Architecture
- Agents
- Tasks
- Mind
- Ops
- Dashboards
- Explainers
- Research
- Other

## Page scaffolding

`aoc-see new` creates a self-contained HTML page and registers it in `manifest.json`.

The default scaffold is now **visual-first**:
- a large primary visualization stage at the top
- a Mermaid source block embedded in the page
- supporting notes and source references below the graph

`aoc-see init` is safe to re-run. It is the canonical seed/confirm action used both from the CLI and from the Alt+C control pane.

Useful flags:

```bash
aoc-see new task-flow \
  --section tasks \
  --kind flow \
  --summary "Task lifecycle and dependency flow" \
  --tags taskmaster,workflow \
  --source .taskmaster/tasks/tasks.json \
  --source docs/plan.md
```

Optional metadata supported by the manifest/page model:

- `section`
- `kind`
- `status`
- `tags`
- `source_paths`
- `featured`
- `generated`
- `order`

## Mermaid rendering

AOC See supports embedded Mermaid source blocks inside page HTML:

```html
<script type="text/plain" data-aoc-see-mermaid>
flowchart LR
  A[Repo] --> B[Model]
  B --> C[View]
</script>
```

AOC See uses **vendored repo-local Mermaid JS assets** to render those blocks in the browser. `aoc-see init` / `aoc-see build` sync the local Mermaid assets under `.aoc/diagrams/assets/`, and pages render offline without a CDN.

This keeps pages:
- repo-local
- self-contained
- reviewable in git
- offline-capable
- free of external/CDN Mermaid dependencies

## HTML metadata discovery

Pages can also self-declare metadata with meta tags, which AOC See can discover while building the homepage:

```html
<meta name="aoc-see:summary" content="How the session lifecycle works">
<meta name="aoc-see:section" content="ops">
<meta name="aoc-see:kind" content="timeline">
<meta name="aoc-see:status" content="active">
<meta name="aoc-see:tags" content="sessions,lifecycle,ops">
```

This lets agents author standalone HTML pages that still show up correctly on the site homepage.

## Authoring guidance

- Prefer self-contained HTML/CSS/JS/SVG.
- Prefer Mermaid for quickly authoring graphs, then let AOC See render it locally from vendored Mermaid JS.
- Make the visualization the main artifact; keep prose secondary.
- Avoid external CDNs when possible.
- Cite source files, commands, task IDs, or runtime surfaces.
- Prefer updating an existing page over creating overlapping duplicates.
- Treat AOC See as the repo’s **visual explanation layer**.

## Suggested future generators

AOC See is ready for generated pages too, for example:

- `aoc-see agents`
- `aoc-see tasks`
- `aoc-see provenance`
- `aoc-see session-map`

Those can emit HTML pages into `.aoc/diagrams/pages/` and let the homepage surface them automatically.
