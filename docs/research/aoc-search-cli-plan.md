# AOC Search CLI Plan (Task 175 / Subtask 4)

This document defines the phase-1 CLI and wrapper interface for the managed AOC search stack.

## Goals
- Give agents and operators one stable AOC-owned search command
- Hide provider and Docker details behind a simple interface
- Return deterministic normalized query results suitable for prompts and tooling
- Share the same behavior path as future Alt+C and subagent integrations

## Canonical wrapper
Phase-1 wrapper:
- `bin/aoc-search`

This command should become the primary interface used by:
- Pi agents
- research/scout-style skills
- Alt+C verify/start actions
- developers debugging local search state

## Command surface

### Status
```bash
aoc-search status
aoc-search status --json
```

Purpose:
- inspect configured provider
- report canonical runtime state
- expose enough metadata for agents without leaking implementation noise

### Start
```bash
aoc-search start
aoc-search start --wait
aoc-search start --json
```

Purpose:
- start managed SearXNG when configured
- optionally wait for health
- provide idempotent startup behavior

### Stop
```bash
aoc-search stop
aoc-search stop --json
```

Purpose:
- stop managed SearXNG cleanly when desired
- primarily for developers/operators

### Health
```bash
aoc-search health
aoc-search health --json
```

Purpose:
- verify queryability without changing runtime state
- useful for TUI verify actions and troubleshooting

### Query
```bash
aoc-search query "react useeffect docs"
aoc-search query --json "rust clap subcommands"
aoc-search query --mode docs --limit 5 "nextjs caching"
aoc-search query --mode error --limit 8 "TypeError Cannot read properties of undefined"
```

Purpose:
- provide normalized structured search results
- support a few opinionated query modes without exposing raw provider parameters by default

## Proposed phase-1 flags

### Common flags
- `--json` → machine-readable structured output
- `--verbose` → optional debugging detail

### Query flags
- `--mode <general|docs|error|package>`
- `--limit <n>`
- `--engine <name>` (optional, likely defer unless needed)
- `--no-auto-start`

Phase 1 should keep the surface narrow. `--mode` and `--limit` are enough to start.

## Query mode semantics
These modes should shape provider parameters and/or light query preprocessing without becoming an over-engineered ranking system.

### `general`
- default behavior
- broad search intended for normal discovery

### `docs`
- biases toward documentation-style queries
- may add conservative query hints later, but should remain transparent

### `error`
- intended for stack traces and exception strings
- should preserve exact error text well

### `package`
- intended for package/library-specific lookup
- useful when the query is centered on a named library or framework

Phase-1 requirement:
- modes must not break deterministic output
- if mode-specific behavior is minimal initially, that is acceptable as long as the interface is stable

## Text output contract
Default non-JSON output should stay compact and readable.

### `status` example
```text
managed searxng: healthy (http://127.0.0.1:8888)
```

### `health` example
```text
healthy: query endpoint reachable at http://127.0.0.1:8888
```

### `query` example
```text
1. Using the Effect Hook – React
   https://react.dev/...
   The Effect Hook lets you perform side effects...

2. Synchronizing with Effects – React
   https://react.dev/...
   Effects let you synchronize a component with an external system...
```

## JSON output contract

### `status --json`
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

### `query --json`
```json
{
  "query": "react useeffect docs",
  "provider": "searxng",
  "status": "ok",
  "mode": "docs",
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

## Query lifecycle behavior
`aoc-search query` should:
1. load and validate config
2. resolve runtime status
3. if unconfigured, fail with clear guidance
4. if disabled, fail with clear guidance
5. if stopped and auto-start is allowed, start managed search
6. verify health
7. call SearXNG JSON endpoint
8. normalize provider results into AOC schema
9. emit text or JSON

## Query normalization requirements
Phase-1 normalization must produce:
- `title`
- `url`
- `snippet`
- `source`
- `rank`

### Normalization rules
- rank is output order starting at 1
- if snippet/content is missing, emit empty string rather than malformed data
- if source/engine is missing, emit a safe fallback like `unknown`
- malformed result rows should be skipped only if essential fields like `url` are absent

## Provider request defaults
The wrapper should derive defaults from `.aoc/search.toml`.

Phase-1 defaults:
- use JSON format
- use configured language/category/safe-search defaults
- respect `--limit`
- keep provider-specific parameters hidden from normal callers

## Failure behavior

### `status`
- should never crash on missing config
- should return a clear status envelope or message

### `query`
Should fail clearly for:
- unconfigured search
- disabled search
- compose not available
- managed search could not start
- health check failure
- invalid provider response

### Example failure messages
- `search is not configured for this repo; enable it via Alt+C`
- `search is disabled in .aoc/search.toml`
- `managed search is stopped and auto-start is disabled`
- `managed search could not be started`
- `search backend returned invalid JSON`

## Exit code expectations
Recommended phase-1 behavior:
- `0` on success
- non-zero on any failed status/start/health/query operation

Suggested distinctions if implemented:
- `1` generic failure
- `2` unconfigured/disabled usage error
- `3` startup/runtime error
- `4` provider/response error

Phase 1 can use generic non-zero if needed, as long as behavior is consistent.

## Implementation guidance
Phase-1 recommendation:
- build `bin/aoc-search` first as a shell wrapper
- delegate parsing/health/start/query helpers to reusable scripts/functions
- migrate into Rust later only if complexity grows

This keeps development fast while preserving a stable UX contract.

## Relationship to future skills and subagents
Skills and future scout-style agents should use:
- `aoc-search status`
- `aoc-search query`

They should not:
- call raw SearXNG URLs directly
- invoke Docker Compose directly in prompts
- rely on provider-specific response fields

## Acceptance criteria
Subtask 4 is complete when:
- `aoc-search` exists as a stable phase-1 wrapper
- status/start/stop/health/query commands work against the managed SearXNG contract
- query output is normalized and deterministic in text and JSON forms
- failures produce clear actionable messages
- the interface is documented well enough for skills, Alt+C, and future subagents to depend on it
