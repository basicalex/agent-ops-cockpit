# Web research

AOC web research combines search-first discovery, cheap static fetching, optional lightweight rendering, and optional browser automation.

Use it when an agent needs current external sources, documentation, error investigation, website inspection, screenshots, scraping, or web-app testing.

## Mental model

```text
question or investigation
  -> search first through managed local search
  -> fetch/extract candidate pages cheaply with aoc-fetch
  -> render with aoc-render/Obscura when static fetch misses JS content
  -> open browser only when interaction, screenshots, auth, or full Chromium behavior is needed
  -> cite findings in the task/commit/handoff
```

Search first prevents blind browsing and keeps context smaller.

## Components

| Component | Purpose |
|---|---|
| `aoc-search` | Stable AOC CLI for managed local search; agents and OMP tools call this instead of SearXNG/Docker |
| `aoc_web_search` | OMP tool backed by `aoc-search` |
| SearXNG | Local metasearch service, usually bound to `127.0.0.1:8888`; required for general docs/web search unless a paid API is configured later |
| Herdr AOC Services workspace | Visible runtime owner for starting/status-checking managed local search |
| `aoc-fetch` | Lightweight static HTTP fetch + text/markdown/JSON extraction before render/browser escalation |
| `aoc-render` | Optional Obscura-backed one-shot rendered extraction for JS pages without a persistent browser server |
| `web-research` skill | Agent workflow for search, fetch, render, source comparison, citation, and synthesis |
| `agent-browser` skill | Browser automation for navigation, forms, screenshots, scraping, and web-app testing |

## Setup

Start from the Services workspace:

```bash
aoc services
aoc services status
aoc services start search
```

Recommended flow:

1. Open/focus the Herdr AOC Services workspace.
2. Start or verify managed local search.
3. Run package/GitHub direct-mode checks that do not need SearXNG.
4. Run a general docs/web query when SearXNG is healthy.
5. Use `aoc-fetch`, `aoc-render`, or browser automation only after search returns candidate URLs.
CLI checks:

```bash
aoc services status
aoc services start search
aoc-search status
aoc-search health
aoc-search query --mode docs --limit 3 "rust clap subcommands"
aoc-search query --mode github --limit 3 "h4ckf0r0day/obscura"
aoc-search query --mode package --direct --limit 3 "requests"
aoc-fetch https://example.com --format markdown
aoc-render status
# optional managed install if Obscura is missing:
aoc-obscura-install --json
aoc-render https://example.com --format text
bin/aoc-web-smoke
```

## Agent usage

Use `aoc_web_search` / `aoc-search` when the task needs external facts or source comparison. General web/docs search depends on the managed local SearXNG runtime; package direct mode and GitHub mode can work without it.

Use `aoc-fetch <url> --format markdown` after search for cheap static extraction.

Use `aoc-render <url> --format text` when static fetch is insufficient and Obscura is available. `aoc-render` is one-shot by default and does not require a persistent server. If explicitly needed, `--fallback agent-browser` can escalate to the full browser layer.

Use `agent-browser` only when the task requires:

- opening a site
- clicking/filling forms
- login/session flows
- screenshots
- scraping page content
- testing a local or remote web app
- visual/browser behavior verification

## Good research output

A good web research result includes:

- short answer
- cited sources
- source dates/versions when relevant
- conflict notes when sources disagree
- commands/pages inspected
- next action recommendation

Avoid dumping full pages or raw search output unless asked.

## Configuration

Managed search writes project-local config under:

```text
.aoc/search.toml
.aoc/services/searxng/
```

The service should bind locally. Do not expose local search publicly. Current project defaults use fixed `127.0.0.1:8888`, so treat managed SearXNG as one active local runtime unless/until a later project-scoped port/container policy is added.

## Troubleshooting

Service workspace and search status:

```bash
aoc services status
aoc-search status
aoc-search health
```

Restart search centrally:

```bash
aoc services start search
aoc-services start search
```

If search works but browser smoke fails:

```bash
agent-browser --version
bin/aoc-web-smoke
```

Likely causes:

- browser runtime missing
- Playwright/driver install incomplete
- local app not reachable
- network or DNS issue
- SearXNG upstream engine noise despite local health passing

More setup detail: [reference/installation-details.md](reference/installation-details.md).
