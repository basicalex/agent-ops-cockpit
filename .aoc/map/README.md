# AOC Map

AOC Map is the project-local graph microsite for this repo.

## Canonical surface
- command: `aoc-map`
- workspace: `.aoc/map/`
- skill: `.pi/skills/aoc-map/SKILL.md`

Legacy `aoc-see` / `.aoc/see` content is tolerated only for migration and compatibility.

## Layout
- `pages/*.html` — graph-first pages.
- `diagrams/*.mmd` — Mermaid graph sources.
- `assets/mermaid.min.js` — vendored Mermaid runtime.
- `assets/render-mermaid.js` — local Mermaid render helper.
- `manifest.json` — site + page metadata.
- `index.html` — generated homepage.

## Seed / refresh
```bash
aoc-map init
aoc-map build
aoc-map serve --open
```

You can also seed from:
- `aoc-init`
- `Alt+C -> Settings -> Tools -> AOC Map microsite`

## Homepage shape
The generated homepage is intentionally compact:
- `AOC Map` title
- total pages
- total diagrams
- search box
- closed-by-default filters dropdown
- one filtered page list
- compact recent updates sidebar

## Workflow
1. `aoc-map init`
2. `aoc-map new agent-topology --section agents --kind topology --summary "How AOC agents route through this repo"`
3. Edit `diagrams/agent-topology.mmd`.
4. Keep `pages/agent-topology.html` minimal and graph-first.
5. Rebuild with `aoc-map build`.
6. Browse with `aoc-map serve --open`.

## Metadata conventions
Pages can declare metadata directly in HTML:
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

Inline Mermaid blocks still work, but external graph files are preferred.

Keep pages self-contained and avoid external network-loaded assets when possible.
