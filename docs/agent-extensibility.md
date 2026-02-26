# Agent Extensibility (PI-first, open by design)

AOC core is intentionally **PI-first** for reliability, licensing clarity, and predictable support.

At the same time, AOC is **not closed** to other agent CLIs. You can plug in your own runtime with low friction via `AOC_AGENT_CMD` and `aoc-agent-wrap`.

## Why PI is the default (and recommended)

PI is the only runtime AOC validates end-to-end in core releases:

- `aoc-agent`, `aoc-agent-run`, and `aoc-agent-install` are PI-only.
- `aoc-init` seeds canonical PI runtime assets under `.pi/**`.
- PI low-token defaults, prompt templates, skill sync, and handshake behavior are maintained as first-class paths.
- PI-only smoke checks are part of release validation.

This keeps the supported path stable and reduces operational drift.

## Bring your own agent CLI in ~2 minutes

Use a tiny wrapper script so launch commands stay simple and shell-quoting safe.

### 1) Create a wrapper

`~/.local/bin/aoc-agent-acme`

```bash
#!/usr/bin/env bash
set -euo pipefail

agent_id="acme"
agent_bin="${ACME_AGENT_BIN:-acme}"
agent_label="AcmeAgent"

exec aoc-agent-wrap "$agent_id" "$agent_bin" "$agent_label" "$@"
```

```bash
chmod +x ~/.local/bin/aoc-agent-acme
```

### 2) Launch AOC with your custom agent

```bash
AOC_AGENT_CMD=aoc-agent-acme aoc
```

This preserves AOC wrapper behavior (session env, hub signaling, handshake path).

### 3) Optional: enable tmux-backed scrollback for custom agent

```bash
AOC_TMUX_AGENT_ALLOWLIST=pi,acme AOC_AGENT_CMD=aoc-agent-acme aoc
```

### 4) Optional: make it your default in shell profile

```bash
export AOC_AGENT_CMD=aoc-agent-acme
```

## Support boundary (important)

- **Core-supported runtime:** `pi`
- **User-managed extensions:** any other CLI via `AOC_AGENT_CMD` + wrapper

So AOC remains Apache-2.0-friendly and extensible, while keeping one hardened default path.
