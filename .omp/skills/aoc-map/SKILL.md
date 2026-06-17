---
name: aoc-map
description: Build and maintain a project-local graph-first microsite under `.aoc/map/`, then browse it with `aoc-map`.
---

## When to use
- The user wants a visual explainer, architecture page, workflow diagram, timeline, topology map, provenance walkthrough, or status/dashboard page.
- The user wants a repo-local graph microsite, not just a one-off diagram.
- You want reviewable HTML artifacts that agents and humans can browse locally.

## Mental model
AOC Map is not just a folder of diagrams.
It is the repo's graph-first microsite.

Each page under `.aoc/map/pages/` is a first-class page in that site, and `aoc-map build` regenerates the homepage shell that ties them together.

## Workspace layout
- `.aoc/map/pages/*.html` — minimal graph-first pages.
- `.aoc/map/diagrams/*.mmd` — Mermaid graph sources.
- `.aoc/map/manifest.json` — site + page metadata.
- `.aoc/map/index.html` — generated homepage.
- `.aoc/map/README.md` — local guidance.

## Core commands
- `aoc-map init`
- `aoc-map new <slug> --section <section> --kind <kind> --summary "..."`
- `aoc-map list`
- `aoc-map build`
- `aoc-map serve --port 43111 --open`

Legacy note:
- `aoc see ...` may still work as a compatibility alias, but new usage should be `aoc-map`.

## Recommended workflow
1. Run `aoc-map init` once per project.
2. Scaffold a page with `aoc-map new <slug>`.
3. Edit the generated Mermaid file in `.aoc/map/diagrams/`.
4. Keep the page in `.aoc/map/pages/` minimal and graph-first.
5. Rebuild the homepage and sync Mermaid assets with `aoc-map build`.
6. Preview the microsite with `aoc-map serve`.

## Homepage expectations
The generated homepage should stay compact and low-clutter.

Prefer this shape:
- title: `AOC Map`
- metrics: total pages + total diagrams
- one search box
- one filters dropdown, closed by default
- one main filtered page list
- one compact recent-updates sidebar

Avoid this on the homepage unless the user explicitly wants it:
- repeating the same pages in multiple sections
- large visible tag clouds inside cards
- source-path dumps on cards
- oversized hero copy
- always-open filter walls

## Preferred sections
Use these when the page fits:
- `architecture`
- `agents`
- `tasks`
- `mind`
- `ops`
- `dashboards`
- `explainers`
- `research`
- `other`

## Common kinds
- `flow`
- `sequence`
- `timeline`
- `topology`
- `dashboard`
- `explain`
- `other`

## Authoring guidance
- Prefer plain HTML + CSS + inline SVG for portability.
- Prefer visual-first layouts with a large primary graph or dashboard near the top.
- Prefer Mermaid source files under `.aoc/map/diagrams/` and reference them from pages:

```html
<script type="text/plain" data-aoc-map-mermaid-src="../diagrams/agent-topology.mmd"></script>
```

- Inline Mermaid blocks still work when helpful.
- `aoc-map build` syncs local Mermaid JS assets and ensures Mermaid pages reference the vendored renderer.
- Include a clear title and short summary.
- Keep prose secondary to the graph.
- If the page explains repo behavior, cite file paths and commands inside the page when useful.
- Prefer updating an existing page over creating a duplicate.
- If the content is speculative, label it clearly.

## HTML metadata
Pages can self-register metadata using meta tags:

```html
<meta name="aoc-map:summary" content="What this page explains">
<meta name="aoc-map:section" content="agents">
<meta name="aoc-map:kind" content="topology">
<meta name="aoc-map:status" content="active">
<meta name="aoc-map:diagram" content="diagrams/agent-topology.mmd">
<meta name="aoc-map:tags" content="agents,orchestration,routing">
```

## Migration compatibility
AOC Map should tolerate older AOC See repos:
- `.aoc/see/` may need migration to `.aoc/map/`
- `.aoc/diagrams/` may need migration to `.aoc/map/`
- old `aoc-see:*` / `data-aoc-see-*` metadata may still exist and should be normalized when rebuilding

## Guardrails
- Do not write outside `.aoc/map/` unless the user asks.
- Avoid external analytics or network-loaded assets.
- Use the repo-local vendored Mermaid assets seeded by AOC Map.
- Keep pages reviewable offline when possible.
- Treat the homepage as the repo's visual entrypoint, not a dumping ground.
