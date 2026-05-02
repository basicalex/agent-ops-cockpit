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
| `aoc-search` | AOC CLI for managed local search |
| SearXNG | Local metasearch service, usually bound to `127.0.0.1:8888` |
| `aoc-fetch` | Lightweight static HTTP fetch + text/markdown/JSON extraction before render/browser escalation |
| `aoc-render` | Optional Obscura-backed one-shot rendered extraction for JS pages without a persistent browser server |
| `web-research` skill | Agent workflow for search, fetch, render, source comparison, citation, and synthesis |
| `agent-browser` skill | Browser automation for navigation, forms, screenshots, scraping, and web-app testing |
| Alt+C | Setup, start/verify, skill sync, and web smoke checks |

## Setup

Open:

```text
Alt+C -> Settings -> Tools -> Agent Browser + Search
```

Recommended flow:

1. Install/update browser tooling.
2. Install/update Pi browser skill.
3. Install/update web research skill.
4. Enable managed local search.
5. Start/verify local search.
6. Run web research smoke check.

CLI checks:

```bash
aoc-search status
aoc-search start --wait
aoc-search health
aoc-search query --limit 3 "rust clap subcommands"
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

Use `web-research` when the task needs external facts or source comparison.

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

The service should bind locally. Do not expose local search publicly.

## Troubleshooting

Search status:

```bash
aoc-search status
aoc-search health
```

Restart search:

```bash
aoc-search stop
aoc-search start --wait
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
