# AOC Search Contract (Phase 1)

This document captures the concrete phase-1 contract for the opt-in AOC search stack described in task 175.

## Goals
- Keep search separate from `agent-browser`
- Make SearXNG opt-in and AOC-managed
- Give agents one stable `status/start/query` interface
- Support lazy auto-start for research/scout-style workflows

## Config
Primary config file:
- `.aoc/search.toml`

Example:

```toml
version = 1

[search]
enabled = true
provider = "searxng"
managed = true
auto_start = true

[search.runtime]
base_url = "http://127.0.0.1:8888"
healthcheck_url = "http://127.0.0.1:8888/search?q=aoc+health&format=json"
compose_file = ".aoc/services/searxng/docker-compose.yml"
service_name = "searxng"

[search.query_defaults]
format = "json"
language = "en"
categories = "general"
safe_search = 0

[search.agent_policy]
allow_auto_start = true
prompt_when_missing = true
prompt_when_unhealthy = true
```

Semantics:
- missing config file => `unconfigured`
- `enabled = false` => `disabled`
- `managed = true` => lifecycle controlled by AOC
- `auto_start = true` => agents may start it on demand

## Runtime status vocabulary
Canonical states:
- `unconfigured`
- `disabled`
- `stopped`
- `starting`
- `healthy`
- `unhealthy`
- `error`

Example status payload:

```json
{
  "configured": true,
  "enabled": true,
  "managed": true,
  "provider": "searxng",
  "runtimeStatus": "healthy",
  "healthy": true,
  "autoStart": true,
  "baseUrl": "http://127.0.0.1:8888",
  "message": "Managed SearXNG is running and healthy."
}
```

## CLI surface
Phase-1 wrapper:
- `bin/aoc-search`

Commands:

```bash
aoc-search status
aoc-search status --json
aoc-search start
aoc-search start --wait
aoc-search stop
aoc-search health
aoc-search query "react useeffect docs"
aoc-search query --json "rust clap subcommands"
aoc-search query --mode docs --limit 5 "nextjs caching"
```

Behavior:
- `status` resolves config + service state + health
- `start` starts managed compose service
- `query` auto-starts when allowed, verifies health, calls provider JSON, normalizes output

## Normalized query output
Agents should receive normalized AOC results, not raw SearXNG payloads.

```json
{
  "query": "react useeffect docs",
  "provider": "searxng",
  "status": "ok",
  "results": [
    {
      "title": "Using the Effect Hook – React",
      "url": "https://react.dev/...",
      "snippet": "The Effect Hook lets you perform side effects...",
      "source": "google",
      "rank": 1
    }
  ],
  "warnings": []
}
```

Required fields per result:
- `title`
- `url`
- `snippet`
- `source`
- `rank`

## Managed deployment layout
AOC-managed search service files:
- `.aoc/services/searxng/docker-compose.yml`
- `.aoc/services/searxng/settings.yml`
- `.aoc/services/searxng/.env` (optional)

Defaults:
- bind to `127.0.0.1`
- use port `8888`
- minimal SearXNG settings for agentic structured search
- Docker Compose is the lifecycle primitive
- lazydocker is optional visibility/ops UX only

## Agent behavior rules
When research/search is needed:
1. call `aoc-search status`
2. if `healthy`, query
3. if `stopped` and auto-start allowed, start then query
4. if `unconfigured`, prompt user toward `Alt+C`
5. if `unhealthy` or `error`, report degraded search and fall back gracefully

## Alt+C shape
Recommended menu direction:
- `Agent Browser + Search`
  - install/update Agent Browser tool
  - install/update PI browser skill
  - enable managed local search (SearXNG)
  - start/verify local search

Summary states should expose both browser and search readiness, e.g.:
- `browser installed, search unconfigured`
- `browser installed, search configured/stopped`
- `browser installed, search healthy`

## Non-goals for phase 1
- multi-provider support
- advanced ranking policies
- public network exposure
- collapsing all research semantics into `agent-browser`
