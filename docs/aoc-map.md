# AOC Map

`aoc-map` turns `.aoc/map/` into a **project-local graph and visualization microsite** for any repo using AOC.

Instead of treating diagrams as isolated files, AOC Map treats them as pages in a small browsable site for the codebase:

- architecture explainers
- agent and subagent maps
- task and workflow views
- Mind/provenance walkthroughs
- ops/runbook dashboards
- research or visual notes

## Core idea

Each repo gets a local website layer:

```text
.aoc/map/
  README.md
  manifest.json
  index.html          # generated homepage / gallery / site shell
  assets/
  diagrams/
    agent-topology.mmd
    task-flow.mmd
  pages/
    agent-topology.html
    task-flow.html
    provenance-map.html
    session-lifecycle.html
```

`aoc-map serve` then serves that folder as a real microsite, not just a raw directory listing.

## Commands

```bash
aoc-map init
aoc-map new agent-topology --section agents --kind topology --summary "How AOC agents route through this repo"
aoc-map list
aoc-map build
aoc-map serve --port 43111 --open
```

You can also seed/confirm the microsite from the floating control pane:

- `Alt+C -> Settings -> Tools -> AOC Map microsite`

## What the homepage provides

The generated `index.html` is a site shell with:

- a minimal AOC Map header
- counts for total pages and total diagrams
- a search box
- collapsible filters for section, kind, and tag
- one filtered page list
- a compact recent updates sidebar

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

`aoc-map new` creates both:
- a minimal graph-first HTML page in `pages/`
- a Mermaid source file in `diagrams/`

The default scaffold is now **visual-first**:
- a large primary visualization stage at the top
- a page that points at a Mermaid file under `diagrams/`
- supporting notes and source references kept intentionally minimal

`aoc-map init` is safe to re-run. It is the canonical seed/confirm action used both from the CLI and from the Alt+C control pane.

Useful flags:

```bash
aoc-map new task-flow \
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

AOC Map supports both inline Mermaid blocks and graph files referenced from pages.

Preferred pattern:

```html
<script type="text/plain" data-aoc-map-mermaid-src="../diagrams/agent-topology.mmd"></script>
```

AOC Map uses **vendored repo-local Mermaid JS assets** to render those graphs in the browser. `aoc-map init` / `aoc-map build` sync the local Mermaid assets under `.aoc/map/assets/`, and pages render offline without a CDN.

This keeps pages:
- repo-local
- self-contained
- reviewable in git
- offline-capable
- free of external/CDN Mermaid dependencies

## HTML metadata discovery

Pages can also self-declare metadata with meta tags, which AOC Map can discover while building the homepage:

```html
<meta name="aoc-map:summary" content="How the session lifecycle works">
<meta name="aoc-map:section" content="ops">
<meta name="aoc-map:kind" content="timeline">
<meta name="aoc-map:status" content="active">
<meta name="aoc-map:diagram" content="diagrams/session-lifecycle.mmd">
<meta name="aoc-map:tags" content="sessions,lifecycle,ops">
```

This lets agents author standalone HTML pages that still show up correctly on the site homepage.

## Authoring guidance

- Prefer self-contained HTML/CSS/JS/SVG.
- Prefer Mermaid files under `.aoc/map/diagrams/` so graph sources stay project-local and reusable.
- Make the visualization the main artifact; keep prose secondary and minimal.
- Avoid external CDNs when possible.
- Cite source files, commands, task IDs, or runtime surfaces.
- Prefer updating an existing page over creating overlapping duplicates.
- Treat AOC Map as the repo’s **visual explanation layer**.

## Suggested future generators

AOC Map is ready for generated pages too, for example:

- `aoc-map agents`
- `aoc-map tasks`
- `aoc-map provenance`
- `aoc-map session-map`

Those can emit HTML pages into `.aoc/map/pages/` and Mermaid files into `.aoc/map/diagrams/`, then let the homepage surface them automatically.
