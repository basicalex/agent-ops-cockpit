---
name: aoc-see
description: Build and maintain a project-local graph-first microsite under `.aoc/see/`, then browse it with `aoc-see`.
---

## When to use
- The user wants a visual explainer, architecture page, workflow diagram, timeline, topology map, provenance walkthrough, or status/dashboard page.
- The user wants a repo-local **website layer** for project understanding, not just one-off diagrams.
- You want reviewable HTML artifacts that agents and humans can browse locally.

## Mental model
AOC See is not just a folder of diagrams.
It is a **microsite for the repo**.

Each page under `.aoc/see/pages/` is a first-class page in that site, and `aoc-see build` regenerates the homepage/gallery shell that ties them together.

## Workspace layout
- `.aoc/see/pages/*.html` — minimal graph-first pages.
- `.aoc/see/diagrams/*.mmd` — Mermaid graph sources.
- `.aoc/see/manifest.json` — site + page metadata.
- `.aoc/see/index.html` — generated microsite homepage.
- `.aoc/see/README.md` — local guidance.

## Core commands
- `aoc-see init`
- `aoc-see new <slug> --section <section> --kind <kind> --summary "..."`
- `aoc-see list`
- `aoc-see build`
- `aoc-see serve --port 43111 --open`

## Recommended workflow
1. Run `aoc-see init` once per project.
2. Scaffold a page with `aoc-see new <slug>`.
3. Edit the generated Mermaid file in `.aoc/see/diagrams/` and keep the page in `.aoc/see/pages/` minimal.
4. Prefer a visual-first page: make the graph or dashboard the first-class artifact and keep prose secondary.
5. Prefer Mermaid source files referenced from the page when a graph is the clearest authoring format.
6. Rebuild the homepage and sync Mermaid assets with `aoc-see build`.
7. Preview the full microsite with `aoc-see serve`.

## Preferred collections
Use these sections when the page fits:
- `architecture`
- `agents`
- `tasks`
- `mind`
- `ops`
- `dashboards`
- `explainers`
- `research`
- `other`

## Common page kinds
- `flow`
- `sequence`
- `timeline`
- `topology`
- `dashboard`
- `explain`
- `other`

## Good page examples
- Agent topology / routing maps
- Task dependency and lifecycle pages
- Mind/provenance walkthroughs
- Session lifecycle / overseer runbooks
- Architecture overview pages
- Research comparison pages
- Local dashboards sourced from AOC state

## Authoring guidance
- Prefer plain HTML + CSS + inline SVG for portability.
- Prefer visual-first layouts with a large primary graph or dashboard surface near the top.
- Prefer Mermaid source files under `.aoc/see/diagrams/` and reference them from pages:

```html
<script type="text/plain" data-aoc-see-mermaid-src="../diagrams/agent-topology.mmd"></script>
```

- Inline Mermaid blocks still work when helpful.
- `aoc-see build` syncs local Mermaid JS assets and ensures Mermaid pages reference the vendored renderer.
- Include a clear title, short summary, and visible source references.
- If the page explains repo behavior, cite file paths and commands used.
- Prefer updating an existing page over creating a duplicate.
- If the content is speculative, label it clearly.

## HTML metadata
Pages can self-register metadata using meta tags:

```html
<meta name="aoc-see:summary" content="What this page explains">
<meta name="aoc-see:section" content="agents">
<meta name="aoc-see:kind" content="topology">
<meta name="aoc-see:status" content="active">
<meta name="aoc-see:diagram" content="diagrams/agent-topology.mmd">
<meta name="aoc-see:tags" content="agents,orchestration,routing">
```

Use this when you want a page to remain understandable and discoverable even if the manifest is only partially maintained.

## Guardrails
- Do not write outside `.aoc/see/` unless the user asks.
- Avoid external analytics or network-loaded assets. Use the repo-local vendored Mermaid assets seeded by AOC See.
- Keep pages reviewable offline when possible.
- Treat the homepage as the repo’s visual entrypoint, not a dumping ground.
