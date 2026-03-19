# AOC Search Alt+C Integration Plan (Task 175 / Subtask 2)

This document turns the phase-1 search contract into a concrete `Alt+C` implementation plan for `aoc-control`.

## Goals
- Keep the setup flow as simple as existing AOC tool integrations
- Let developers opt into managed SearXNG while installing/maintaining Agent Browser
- Keep browser and search visible together without collapsing them into one primitive
- Make future agent/subagent auto-start behavior possible via persisted AOC metadata

## Current relevant surface
Today `Alt+C -> Settings -> Tools` exposes:
- PI agent installer
- PI compaction
- Agent Browser tool/skill
- Vercel CLI + PI skill
- MoreMotion + /momo

Agent Browser currently has two actions:
- install/update tool
- install/update PI skill

## Proposed menu changes

### Tools summary row
Replace:
- `Agent Browser tool/skill · ...`

With:
- `Agent Browser + Search · ...`

## Proposed summary states
The summary should combine browser and search readiness in compact operator language.

Examples:
- `browser missing, search unconfigured, skill missing`
- `browser installed, search unconfigured, skill present`
- `browser installed, search configured/stopped, skill present`
- `browser installed, search healthy, skill present`
- `browser installed, search unhealthy, skill present`

Recommended shape:
- browser: `missing | installed`
- search: `unconfigured | disabled | stopped | healthy | unhealthy`
- skill: `missing | present`

## Proposed nested section
Rename section:
- `Settings · Tools · Agent Browser`

To:
- `Settings · Tools · Agent Browser + Search`

### Proposed items
1. `Install/update Agent Browser tool · <browser runtime summary>`
2. `Install/update PI browser skill`
3. `Enable managed local search (SearXNG) · <search summary>`
4. `Start/verify local search · <search summary>`
5. `Back`

Optional later item:
- `Disable managed local search`

Phase 1 can omit disable if we prefer config edits to be one-way initially, but enabling plus verify/start should exist.

## Behavior by row

### Row 1: Install/update Agent Browser tool
Matches the current behavior.

Behavior:
- if missing -> install
- if installed -> update
- reuse current background job/log model already used for Agent Browser

No functional change required except updated section naming and summary wiring.

### Row 2: Install/update PI browser skill
Matches current browser skill sync behavior.

Behavior:
- ensure `.pi/skills/agent-browser/SKILL.md`
- potentially later append reference to search-first research workflow docs

### Row 3: Enable managed local search (SearXNG)
This is the new provisioning action.

Behavior:
1. Check Docker / Compose availability
2. Generate `.aoc/search.toml`
3. Generate `.aoc/services/searxng/docker-compose.yml`
4. Generate `.aoc/services/searxng/settings.yml`
5. Optionally write `.aoc/services/searxng/.env`
6. Persist default managed search metadata
7. Refresh summary/status in the UI

Default config written should correspond to the contract in `docs/research/aoc-search-contract.md`.

If Docker or Compose is unavailable:
- do not write partial broken state
- show a clear action message in the status line
- tell the user what dependency is missing

### Row 4: Start/verify local search
This is the operational action after search is configured.

Behavior:
- if unconfigured: show a message to enable managed local search first
- if configured and stopped: start via compose, wait for health, refresh status
- if configured and healthy: run a health check / verify query and report success
- if configured and unhealthy: retry verification and surface actionable failure details

This row should become the manual operator equivalent of what `aoc-search start` / future auto-start logic will do.

## Proposed internal state additions in `aoc-control`
To support this cleanly, `App` should track explicit search state just like it already tracks Agent Browser state.

Suggested additions:
- `search_status_checked: bool`
- `search_status: SearchStatusSummary`
- `search_job: Option<SearchJob>` if provisioning/start runs in background
- `search_log_tail: Vec<String>` if we want parity with browser job logs
- `search_log_scroll: usize`

Suggested types:

```rust
enum SearchRuntimeStatus {
    Unconfigured,
    Disabled,
    Stopped,
    Starting,
    Healthy,
    Unhealthy,
    Error,
}

struct SearchStatusSummary {
    configured: bool,
    enabled: bool,
    managed: bool,
    runtime_status: SearchRuntimeStatus,
    healthy: bool,
    message: String,
}
```

## Proposed helper functions
New helper functions analogous to existing Agent Browser helpers:
- `search_config_exists()`
- `load_search_config()`
- `managed_search_installed()`
- `probe_search_health()`
- `search_summary()`
- `generate_managed_search_files()`
- `run_search_enable_action()`
- `run_search_start_or_verify_action()`

If we keep phase 1 simple, these can call a shared shell wrapper later reused by `bin/aoc-search`.

## File generation contract
Phase-1 generated files:
- `.aoc/search.toml`
- `.aoc/services/searxng/docker-compose.yml`
- `.aoc/services/searxng/settings.yml`
- `.aoc/services/searxng/.env` (optional)

### Generation rules
- create parent directories automatically
- write deterministic file content
- avoid overwriting unrelated user files outside AOC-managed paths
- treat `.aoc/services/searxng/` as AOC-owned

## Status refresh rules
The TUI should refresh search state:
- when entering `Settings · Tools`
- when entering `Settings · Tools · Agent Browser + Search`
- after enable/start/verify actions
- after returning from any completed background job

## Error messaging expectations
Keep messages short and actionable.

Examples:
- `Managed search enabled (.aoc/search.toml written)`
- `Managed search files generated; start/verify to launch SearXNG`
- `Docker Compose not found; install Docker/Compose before enabling search`
- `Managed search started and healthy at http://127.0.0.1:8888`
- `Managed search health check failed; inspect .aoc/services/searxng and Docker logs`

## Interaction with future `aoc-search`
The TUI should not become the only control plane.

Phase-1 recommendation:
- Alt+C provisioning may directly generate files and run compose
- runtime logic should be kept close to a reusable wrapper shape
- later, `Alt+C` actions can call `aoc-search status/start/health`

That keeps future subagents and the TUI on the same behavior path.

## Minimal implementation sequence
1. Rename browser section and summary to include search
2. Add search summary derivation helpers
3. Add row: `Enable managed local search (SearXNG)`
4. Add row: `Start/verify local search`
5. Implement deterministic file generation
6. Implement start/verify action
7. Refresh docs and help/detail panel text

## Acceptance criteria
Subtask 2 is complete when:
- `Alt+C` exposes Agent Browser + Search as a combined tool area
- developers can enable managed search without manual file authoring
- developers can manually start/verify local search from the same section
- status summaries clearly distinguish unconfigured/stopped/healthy/unhealthy search
- browser-only usage still works unchanged when search is not enabled
