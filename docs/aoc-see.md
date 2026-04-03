# AOC See

`aoc-see` turns `.aoc/see/` into a **project-local graph and visualization microsite** for any repo using AOC.

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
.aoc/see/
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

`aoc-see new` creates both:
- a minimal graph-first HTML page in `pages/`
- a Mermaid source file in `diagrams/`

The default scaffold is now **visual-first**:
- a large primary visualization stage at the top
- a page that points at a Mermaid file under `diagrams/`
- supporting notes and source references kept intentionally minimal

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

AOC See supports both inline Mermaid blocks and graph files referenced from pages.

Preferred pattern:

```html
<script type="text/plain" data-aoc-see-mermaid-src="../diagrams/agent-topology.mmd"></script>
```

AOC See uses **vendored repo-local Mermaid JS assets** to render those graphs in the browser. `aoc-see init` / `aoc-see build` sync the local Mermaid assets under `.aoc/see/assets/`, and pages render offline without a CDN.

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
<meta name="aoc-see:diagram" content="diagrams/session-lifecycle.mmd">
<meta name="aoc-see:tags" content="sessions,lifecycle,ops">
```

This lets agents author standalone HTML pages that still show up correctly on the site homepage.

## Authoring guidance

- Prefer self-contained HTML/CSS/JS/SVG.
- Prefer Mermaid files under `.aoc/see/diagrams/` so graph sources stay project-local and reusable.
- Make the visualization the main artifact; keep prose secondary and minimal.
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

Those can emit HTML pages into `.aoc/see/pages/` and Mermaid files into `.aoc/see/diagrams/`, then let the homepage surface them automatically.
