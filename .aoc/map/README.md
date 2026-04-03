# AOC Map

AOC Map is the project-local graph and visualization microsite for this repo. The main artifact is the graph, not the chrome around it.

## Layout
- `pages/*.html` — minimal graph-first presentation pages.
- `diagrams/*.mmd` — Mermaid source files used as the canonical graph definitions.
- `assets/mermaid.min.js` — vendored Mermaid runtime used locally/offline.
- `assets/render-mermaid.js` — AOC Map helper that renders Mermaid blocks and Mermaid source files in the browser.
- `manifest.json` — site metadata and page metadata used for the homepage shell.
- `index.html` — generated homepage for the microsite.

## Workflow
1. `aoc-map init`
2. `aoc-map new agent-topology --section agents --kind topology --summary "How AOC agents route through this repo"`
3. Edit the generated Mermaid file under `diagrams/agent-topology.mmd`.
4. Keep the page under `pages/agent-topology.html` minimal and graph-first.
5. Rebuild with `aoc-map build`, then browse with `aoc-map serve --open`.

## Metadata conventions
Pages can declare metadata directly in HTML via meta tags such as:
- `<meta name="aoc-map:summary" content="...">`
- `<meta name="aoc-map:section" content="agents">`
- `<meta name="aoc-map:kind" content="topology">`
- `<meta name="aoc-map:status" content="active">`
- `<meta name="aoc-map:diagram" content="diagrams/agent-topology.mmd">`
- `<meta name="aoc-map:tags" content="agents,orchestration">`

## Graph authoring
Prefer Mermaid files in `diagrams/*.mmd` and reference them from pages with:

```html
<script type="text/plain" data-aoc-map-mermaid-src="../diagrams/agent-topology.mmd"></script>
```

Inline Mermaid blocks still work, but external graph files are the preferred project-context-friendly path.

Prefer self-contained HTML/CSS/JS/SVG and avoid external network-loaded assets when possible.
