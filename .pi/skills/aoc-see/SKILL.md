---
name: aoc-see
description: Build and maintain a project-local visualization microsite under `.aoc/diagrams/`, then browse it with `aoc-see`.
---

## When to use
- The user wants a visual explainer, architecture page, workflow diagram, timeline, topology map, provenance walkthrough, or status/dashboard page.
- The user wants a repo-local **website layer** for project understanding, not just one-off diagrams.
- You want reviewable HTML artifacts that agents and humans can browse locally.

## Mental model
AOC See is not just a folder of diagrams.
It is a **microsite for the repo**.

Each page under `.aoc/diagrams/pages/` is a first-class page in that site, and `aoc-see build` regenerates the homepage/gallery shell that ties them together.

## Workspace layout
- `.aoc/diagrams/pages/*.html` — self-contained pages.
- `.aoc/diagrams/manifest.json` — site + page metadata.
- `.aoc/diagrams/index.html` — generated microsite homepage.
- `.aoc/diagrams/README.md` — local guidance.

## Core commands
- `aoc-see init`
- `aoc-see new <slug> --section <section> --kind <kind> --summary "..."`
- `aoc-see list`
- `aoc-see build`
- `aoc-see serve --port 43111 --open`

## Recommended workflow
1. Run `aoc-see init` once per project.
2. Scaffold a page with `aoc-see new <slug>`.
3. Edit the generated HTML page in `.aoc/diagrams/pages/`.
4. Prefer a visual-first page: make the graph or dashboard the first-class artifact and keep prose secondary.
5. Use embedded Mermaid blocks when a graph is the clearest authoring format.
6. Rebuild the homepage with `aoc-see build`.
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
- Use embedded Mermaid blocks when helpful:

```html
<script type="text/plain" data-aoc-see-mermaid>
flowchart LR
  A[Input] --> B[Transform]
  B --> C[View]
</script>
```

- `aoc-see build` renders those Mermaid blocks to inline SVG and keeps the source block in the page.
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
<meta name="aoc-see:tags" content="agents,orchestration,routing">
```

Use this when you want a page to remain understandable and discoverable even if the manifest is only partially maintained.

## Guardrails
- Do not write outside `.aoc/diagrams/` unless the user asks.
- Avoid external analytics or network-loaded assets.
- Keep pages reviewable offline when possible.
- Treat the homepage as the repo’s visual entrypoint, not a dumping ground.
