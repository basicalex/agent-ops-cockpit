# Configuration Guide

Advanced configuration options for Agent Ops Cockpit (AOC).

## Most users only need

- `aoc-doctor` to verify install health
- `aoc` to open/focus the Herdr project workspace
- `aoc services` to open/focus the Herdr AOC Services workspace
- `aoc services status` and `aoc-search health` for managed local search/runtime checks
- this document mainly as a reference for paths, env vars, and advanced tuning

## Table of Contents

- [Environment Variables](#environment-variables)
  - [Command Overrides](#command-overrides)
  - [Clock Configuration](#clock-configuration)
  - [Herdr Services](#herdr-services)
  - [RTK Routing](#rtk-routing)
  - [Agent Installers](#agent-installers)
  - [Agent Configuration](#agent-configuration)
- [Theme Management](#theme-management)
- [Per-Project Configuration](#per-project-configuration)

## Environment Variables

### Command Overrides

Override default commands used by AOC helper panes/services:

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_AGENT_CMD` | Command to run in agent pane | Auto-detected |
| `AOC_TASKMASTER_CMD` | Taskmaster TUI command | `aoc-taskmaster` |
| `AOC_TASKMASTER_ROOT` | Override Taskmaster project root for `tm`/`aoc-task`/`aoc-taskmaster` | Current working directory |
| `AOC_FILETREE_CMD` | File manager command | `yazi` |
| `AOC_CLOCK_CMD` | Clock command | Auto-detected |
| `AOC_SYS_CMD` | System stats command | `aoc-sys` |
| `AOC_TERMINAL_CMD` | Terminal shell | `$SHELL` |

For low-pain custom agent integration, point `AOC_AGENT_CMD` at your own launcher script when a project needs a non-default agent command:

```bash
AOC_AGENT_CMD=~/.local/bin/aoc-agent-acme aoc
```

### Clock Configuration

Fine-tune the clock widget appearance:

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_CLOCK_INTERVAL` | Refresh interval in seconds | `1` |
| `AOC_CLOCK_TIME_FORMAT` | Time format (date format string) | `%H:%M` |
| `AOC_CLOCK_DATE_FORMAT` | Date format (date format string) | `%A, %B %d` |
| `AOC_CLOCK_FONT` | Figlet font name | `small` |
| `AOC_CLOCK_BACKEND` | Backend selection | `auto` |
| `AOC_CLOCK_TTY_FLAGS` | Flags for tty-clock | None |

**Backend Priority (when `AOC_CLOCK_BACKEND=auto`):**
1. tty-clock (if installed)
2. Figlet fallback

**Persist Clock Settings:**

```bash
aoc-clock-set
```

### Herdr Services

The default runtime owner is the project-scoped Herdr AOC Services workspace:

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_HERDR_SERVICES` | `auto` ensures the Services workspace when a Herdr server already exists; `off` disables launch-time ensure; `focus` opens Services for explicit ops sessions | `auto` |
| `AOC_SERVICES_ROOT` | Override the service root used by `aoc-services` and the Services workspace panes | Resolved project root |
| `AOC_SERVICES_WATCH_INTERVAL` | Default refresh interval for `aoc-services status --watch` / `up --watch` | `30` |
| `AOC_SERVICES_HEALTH_CACHE_TTL` | Max cached service-health age for watch mode | `20` |

Operator commands:

```bash
aoc services
aoc services status
aoc services start search
```

`aoc services` uses Herdr workspace/tab/pane commands. It does not start Herdr behind the operator's back; start Herdr with `aoc` first if no Herdr server is running.

### RTK Routing

RTK routing is optional, per-project, and fail-open by default. It activates for wrapped agent commands when routing mode is enabled.

Primary benefit: route noisy shell output through RTK so agents keep higher signal density in-context (less output bloat, lower token pressure, faster coding loops).

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_RTK_BYPASS` | Disable RTK routing for the current process/session | `0` |
| `AOC_RTK_MODE` | Force mode override (`off` to bypass) | From `.aoc/rtk.toml` |
| `AOC_RTK_CONFIG` | Override RTK config file location | `<project>/.aoc/rtk.toml` |
| `AOC_RTK_BINARY` | Override RTK binary name/path | `rtk` |
| `AOC_RTK_GAIN_MODE` | RTK invocation mode (`double-dash` or `positional`) | `double-dash` |
| `AOC_RTK_FAIL_OPEN` | Fallback to native command on RTK execution error | `1` |
| `AOC_RTK_ULTRA_COMPACT` | Pass `-u` to RTK commands for tighter output | `0` |
| `AOC_RTK_ROUTE_NON_TTY_STDIN` | Allow RTK routing when stdin is non-tty and not piped/file redirected | `0` |
| `AOC_RTK_INSTALL_URL` | Pinned installer artifact URL for `aoc-rtk install` | None |
| `AOC_RTK_INSTALL_SHA256` | SHA256 for pinned installer artifact | None |
| `AOC_RTK_INSTALL_DIR` | Install target directory for `aoc-rtk install` | `~/.local/bin` |
| `AOC_RTK_RELEASE_REPO` | Upstream repo used by `aoc-rtk install --auto` | `rtk-ai/rtk` |
| `AOC_RTK_RELEASE_TAG` | Override release tag for `aoc-rtk install --auto` | latest |

Runtime/debug variables:

| Variable | Description |
|----------|-------------|
| `AOC_RTK_ACTIVE` | `1` when RTK shims are active in the current agent session |
| `AOC_RTK_SHIM_DIR` | Session-local shim directory prepended to PATH |

Project config file: `.aoc/rtk.toml` (seeded by `aoc-init`).

By default, new `aoc-init` runs seed RTK with `mode = "on"` for context health. Existing projects with `mode = "off"` are preserved as-is.

```toml
mode = "on"
fail_open = true
gain_mode = "double-dash"
binary = "rtk"
allowlist = ["git status", "git diff", "rg", "pytest"]
denylist = ["git push", "git reset --hard", "rm -rf"]
install_url = ""
install_sha256 = ""
```

The seeded allowlist includes read-only Git inspection and common safe local diagnostics. Mutating Git operations are denied in ambient routing so mutations only happen through explicit operator workflows.

Operator commands:

```bash
aoc-rtk status
aoc-rtk enable
aoc-rtk disable
aoc-rtk doctor
aoc-rtk install
aoc-rtk install --auto
# Manual routing test (shorthand)
aoc-rtk git status
# Manual routing test (explicit)
aoc-rtk run rg "TODO"
```

Recommended rollout order:

1. Run `aoc-rtk install --auto`.
2. Optionally review pinned `install_url` + `install_sha256` in `.aoc/rtk.toml`.
3. Validate with `aoc-rtk doctor`.
4. If needed, disable quickly with `aoc-rtk disable`.
5. If needed, bypass immediately with `AOC_RTK_BYPASS=1`.

Safety model:

- Allowlist-first routing through `aoc-rtk-proxy` command shims.
- Session-local PATH wiring (no global command hijack).
- Explicit bypass via `AOC_RTK_BYPASS=1`.
- Fail-open fallback to native execution when RTK is unavailable.

### OMP-first init migration behavior

`aoc-init` is the one-command repair path for OMP-first repos.

What it guarantees:
- Seeds/repairs canonical project OMP sources under `.omp/extensions/`, `.omp/agents/`, and `.omp/skills/`.
- Installs AOC OMP extensions into `${AOC_OMP_AGENT_DIR:-~/.omp/agent}/extensions`, including CodeGraph, commit, state, DOX, brand-content, and web-search surfaces.
- Installs AOC OMP agent templates into `${AOC_OMP_AGENT_DIR:-~/.omp/agent}/agents`.
- Installs AOC OMP skills into `${AOC_OMP_AGENT_DIR:-~/.omp/agent}/skills`.
- Keeps AOC control-plane state under `.aoc/**`.
- Seeds reusable preset assets when missing: `.aoc/presets/design/**`.
- Does not create or repair legacy Pi runtime paths.

Validation commands:

```bash
bash scripts/pi/test-aoc-init-pi-first.sh
bash scripts/pi/test-pi-only-agent-surface.sh
aoc-handshake --json >/tmp/aoc-handshake.json
```

`aoc-handshake` is metadata-only by design: it advertises context health/policy and focused retrieval commands without injecting broad memory into agent startup context.

**Preview Pane Placement:**

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_PREVIEW_WIDTH` | Floating preview width | Percentage |
| `AOC_PREVIEW_HEIGHT` | Floating preview height | Percentage |
| `AOC_PREVIEW_X` | X position | Percentage |
| `AOC_PREVIEW_Y` | Y position | Percentage |
| `AOC_PREVIEW_PINED` | Keep pinned | Boolean |
| `AOC_PREVIEW_PANE_NAME` | Pane name | `Preview` |

### Agent runtime setup

Use OMP-managed agent installation and update flows through `aoc-init`, `aoc-herdr-install`, or direct OMP tooling when needed.

Default command overrides:

| Variable | Description |
|----------|-------------|
| `AOC_PI_INSTALL_CMD` / `AOC_PI_UPDATE_CMD` | PI (npm) install/update command |

PI installer behavior:

- By default, `pi` installs from npm (`pnpm add -g @mariozechner/pi-coding-agent`).
- AOC does not bundle PI artifacts; it only executes installer commands.

### Tools Integrations

Prefer direct Herdr/CLI surfaces for default work:

- Managed search/service runtime: `aoc services`, `aoc-search`
- HyperFrames: `aoc-hyperframes`
- RTK: `aoc-rtk`
- Vercel CLI: `vercel`

### OMP runtime config

Use OMP's own runtime config at `~/.omp/agent/config.yml` for model/auth/status-line settings. AOC project state does not write global OMP secrets or model credentials.

`aoc-doctor` warns if `~/.omp/agent/config.yml` is missing, but that is not a project failure because global/operator config may be created outside the repo.

| Variable | Description |
|----------|-------------|
| `AOC_OMP_AGENT_DIR` | OMP runtime directory used by `aoc-init`/`aoc-herdr-install` for extensions, agents, and skills (default `~/.omp/agent`) |
| `AOC_VERCEL_BIN` | Vercel CLI binary name/path check (default `vercel`) |
| `AOC_VERCEL_INSTALL_CMD` / `AOC_VERCEL_UPDATE_CMD` | Vercel CLI install/update commands |
| `AOC_HYPERFRAMES_DIR` | Workspace directory used by `aoc-hyperframes` (default `hyperframes`) |
| `AOC_HYPERFRAMES_TRACK_WORKSPACE` | Set to `1` to avoid adding the HyperFrames workspace to `.gitignore` |

### Managed Local Search

Managed local search is project-local. The Herdr AOC Services workspace is the visible runtime owner; `aoc-search` is the stable CLI/tool surface.

Services/search setup writes:

- write `.aoc/search.toml`
- write `.aoc/services/searxng/docker-compose.yml`
- write `.aoc/services/searxng/settings.yml`
- start/verify the managed SearXNG container through `aoc services start search` or `aoc-search start --wait`

Managed local search no longer seeds browser/search legacy browser/search skills. Agents use the built-in/web-search tool surface and OMP extension paths.

Canonical phase-1 paths:

- `.aoc/search.toml`
- `.aoc/services/searxng/docker-compose.yml`
- `.aoc/services/searxng/settings.yml`
- `bin/aoc-search`
- `.omp/extensions/aoc-web-search.ts`

Use `aoc services` for operator-visible runtime ownership and `aoc-search` as the stable interface for agents and operators:

```bash
aoc services
aoc services status
aoc services start search
aoc-search status
aoc-search health
aoc-search query --mode docs --limit 5 "rust clap subcommands"
bin/aoc-web-smoke
```

General docs/web search needs managed local SearXNG unless a separate paid search API is configured later. Direct package and GitHub modes can run without SearXNG. Use `agent-browser` after you have candidate URLs or need rendered-page interaction.

### Agent Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_AGENT_ID` | Override default agent for session | From `aoc-agent` |
| `AOC_PI_BIN` | PI Agent (npm) binary path | `pi` |
| `AOC_OMP_LOW_TOKEN_MODE` | Enable default OMP low-token prompt append (`1`/`0`) | `1` |
| `AOC_PI_LOW_TOKEN_PROMPT` | Override OMP low-token prompt file path | `<project>/.aoc/prompts/pi-low-token.md` |
| `AOC_PI_APPEND_SYSTEM_PROMPT` | Extra `--append-system-prompt` text/path passed to PI | None |
| `AOC_OMP_HANDSHAKE_MODE` | PI handshake verbosity (`compact`, `full`, `off`) | `compact` |
| `AOC_HANDSHAKE_MODE` | Global handshake verbosity override (`compact`, `full`, `off`) | Agent default |
| `AOC_OMP_CONTEXT_LEVEL` | OMP startup capsule size (`min`, `compact`, `full`) | `compact` |
| `AOC_PI_USE_WRAP_RS` | PI launch mode (`auto`, `1`, `0`) | `auto` |
| `AOC_PI_USE_PTY` | Preferred PTY mode for PI children | managed pane=`1`, manual=`0` |
| `AOC_AGENT_PTY` | Explicit PTY override for the wrapped child | Auto |
| `AOC_PI_USE_BOOTLOADER` | Pre-PI shell handshake/bootloader (`auto`, `1`, `0`) | managed pane=`0`, manual=`1` |
| `AOC_PI_USE_TMUX` | Nested tmux use for PI (`auto`, `1`, `0`); default AOC PI panes opt into tmux persistence with a stable `AOC_AGENT_TMUX_ID` | managed AOC PI pane=`1`, manual=`allowlist` |
| `AOC_AGENT_PATTERN` | Additional agent names for cleanup | None |
| `AOC_AGENT_TMUX_CONF` | Custom tmux config for tmux-enabled agents | Default |
| `AOC_TMUX_AGENT_ALLOWLIST` | Comma-separated agent IDs that should run inside tmux when tmux is enabled/auto | `pi` |

Valid `AOC_AGENT_ID` value is `pi`.

- `pi` launches the npm PI Agent CLI.
- `pi` auto-appends `.aoc/prompts/omp-low-token.md` unless disabled by `AOC_OMP_LOW_TOKEN_MODE=0` or overridden by explicit PI prompt flags.
- `pi` defaults to compact handshake output; set `AOC_OMP_HANDSHAKE_MODE=full` for the richer focus-first briefing.
- Full handshake mode now favors: focus provenance, high-value open work, workstream health, recent developments, and open fronts before lower-value inventory.
- When canon or task state is missing, the briefing degrades explicitly with fallback status notes instead of silently pretending a stronger focus signal exists.
- `pi` enables RTK ultra-compact output and non-tty routing by default (`AOC_RTK_ULTRA_COMPACT=1`, `AOC_RTK_ROUTE_NON_TTY_STDIN=1`) unless you override them.
- Managed `pi` launches should use the supported Herdr/OMP/AOC CLI path directly.

## Theme Management

Theme management is outside the default Herdr-first AOC cockpit surface. Use direct system/Omarchy theme tooling for desktop themes. AOC still writes tool-specific artifacts where individual retained tools require them, such as generated Yazi theme output.

## Per-Project Configuration

AOC uses a **Distributed Cognitive Architecture** with four layers:

### 1. Project Context (`.aoc/context.md`)

- **Purpose:** Auto-generated project map
- **Content:** Project-specific snapshot (repo facts, VCS mode, Git branch when present, key files, structure tree, README headings, workstream tags, task PRD location)
- **Refresh:** `aoc-init` (manual) or `aoc-watcher` (auto)

### 2. Long-Term Memory (`.aoc/memory.md`)

- **Purpose:** Persistent architectural decisions
- **Access:** `aoc-mem read` (start of task), `aoc-mem add` (decisions)

### 3. Task State (`.taskmaster/tasks/tasks.json`)

- **Purpose:** Active work queue
- **Management:** `aoc-task` commands

### 4. RTK Routing Policy (`.aoc/rtk.toml`)

- **Purpose:** Project-local routing mode, allowlist/denylist, and pinned install contract
- **Management:** `aoc-rtk status|enable|disable|doctor|install --auto`

### 5. Search Configuration (`.aoc/search.toml`)

- **Purpose:** Project-local managed search contract
- **Management:** `aoc services`, `aoc-services`, or `bin/aoc-search`
- **Related paths:** `.aoc/services/searxng/**`, `.omp/extensions/aoc-web-search.ts`

### Global Configuration

User defaults are stored in:

- `~/.config/aoc/config.toml` - AOC settings
- `~/.taskmaster/config.json` - Taskmaster preferences

---

**See Also:**
- [Installation Guide](../installation.md)
- [Main README](../../README.md)
