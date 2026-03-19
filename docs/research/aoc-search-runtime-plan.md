# AOC Search Runtime Plan (Task 175 / Subtask 3)

This document defines the runtime and lifecycle behavior for managed SearXNG after the Alt+C setup flow has written the search config and service files.

## Goals
- Make managed search inspectable, startable, and health-checkable by AOC
- Reuse one canonical runtime model for TUI actions, CLI wrappers, and future agents/subagents
- Keep Docker/Compose details out of normal agent prompts
- Distinguish clearly between unconfigured, stopped, healthy, and unhealthy states

## Inputs
Runtime helpers should rely on the phase-1 contract in `docs/research/aoc-search-contract.md`:
- `.aoc/search.toml`
- `.aoc/services/searxng/docker-compose.yml`
- optional `.aoc/services/searxng/settings.yml`
- optional `.aoc/services/searxng/.env`

## Canonical runtime responsibilities
The runtime layer must provide:
- config loading
- config validation
- compose availability checks
- service status detection
- lazy start behavior
- health checks
- compact operator/agent status messages

## Recommended runtime helpers
These helpers may initially live in shell and later migrate into Rust/shared code.

### Config helpers
- `search_config_exists()`
- `load_search_config()`
- `validate_search_config()`

Responsibilities:
- detect whether `.aoc/search.toml` exists
- parse required values
- report malformed config as `error`, not `unconfigured`

## Compose/runtime helpers
- `docker_compose_bin()`
- `search_compose_up()`
- `search_compose_down()`
- `search_compose_ps()`
- `search_service_running()`

Responsibilities:
- support `docker compose` first
- optionally fall back to `docker-compose` if desired
- use the compose file path from config
- avoid hardcoding paths outside the config contract

## Health helpers
- `probe_search_health()`
- `wait_for_search_health()`

Responsibilities:
- hit `healthcheck_url` from config
- treat successful JSON response as healthy
- retry for a bounded window on startup
- return actionable unhealthy/error messages

## Derived status model
The runtime layer should derive one of these states:
- `unconfigured`
- `disabled`
- `stopped`
- `starting`
- `healthy`
- `unhealthy`
- `error`

## Derivation rules

### `unconfigured`
Return when:
- `.aoc/search.toml` is missing

### `disabled`
Return when:
- config exists
- `search.enabled = false`

### `stopped`
Return when:
- config is valid and enabled
- managed service is not running

### `starting`
Transient state during:
- compose up completed or is in progress
- health probe is still waiting for success

### `healthy`
Return when:
- config is valid and enabled
- service is running
- health endpoint returns a successful query response

### `unhealthy`
Return when:
- service appears to be running
- but health probe fails or times out

### `error`
Return when:
- config is malformed
- docker/compose tooling is unavailable
- compose command fails unexpectedly
- health probe returns invalid/unparseable provider output in a way that indicates runtime failure

## Proposed `aoc-search status` behavior
`aoc-search status` should:
1. check config existence
2. load and validate config
3. check Docker/Compose availability if managed
4. inspect runtime state
5. probe health when applicable
6. emit compact text or structured JSON

### Text example
```text
managed searxng: healthy (http://127.0.0.1:8888)
```

### JSON example
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

## Proposed `aoc-search start` behavior
`aoc-search start` should:
1. require configured + enabled managed search
2. fail clearly if Docker/Compose is unavailable
3. run compose up in detached mode
4. optionally wait for health
5. return final status

### Idempotency requirements
- if already healthy, return success without restarting
- if already running but unhealthy, retry health and report degraded state
- if stopped, start once and wait

## Proposed `aoc-search health` behavior
`aoc-search health` should:
- perform health-only verification without changing service state
- be safe for TUI verify actions
- report whether the current runtime is queryable

This is useful for:
- Alt+C verify row
- future scout/subagent preflight
- debugging after startup

## Timeout and retry defaults
Recommended phase-1 defaults:
- startup wait timeout: 30 seconds
- health retry interval: 1 second
- query request timeout: 10 seconds
- compose command timeout: 60 seconds for initial startup

These can become configurable later if needed.

## Docker/Compose detection strategy
Recommended order:
1. `docker compose`
2. `docker-compose`

If neither exists:
- return `error`
- message: `Docker Compose not available`

If Docker exists but daemon is unavailable:
- return `error`
- message: `Docker is installed but daemon is not reachable`

## Logging expectations
Runtime helpers should emit compact user-facing output but preserve enough detail for logs.

Recommended logging behavior:
- concise stdout for status/start/health
- stderr for failures
- Alt+C may capture full command output in a temp log file similar to Agent Browser jobs

## Failure message examples
- `search is not configured for this repo`
- `search is disabled in .aoc/search.toml`
- `Docker Compose not available`
- `managed search is stopped`
- `managed search started but health check timed out`
- `managed search is running but unhealthy`

## Interface contract for later subtask 4
Subtask 3 should make subtask 4 trivial by exposing stable runtime helpers that `aoc-search` can call.

Phase-1 recommendation:
- implement lifecycle logic once
- let both Alt+C and `aoc-search` call the same helpers or wrapper path

## Acceptance criteria
Subtask 3 is complete when:
- AOC can load and validate search config reliably
- AOC can distinguish unconfigured/disabled/stopped/healthy/unhealthy/error states
- AOC can start managed SearXNG through Compose
- AOC can verify health through the configured endpoint
- lifecycle behavior is documented clearly enough for the TUI, CLI wrapper, and future subagents to share it
