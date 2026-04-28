# RTK routing

RTK routing is AOC's command-routing and output-condensing layer. It exists to keep agent context healthy when common commands produce large, repetitive, or low-signal output.

RTK is not a security sandbox. It is a context-health optimization with conservative routing policy and native fallback.

## Mental model

```text
agent/tool command
  -> AOC RTK policy check
  -> allowlisted noisy command? route through RTK condenser
  -> risky command? deny or require native/manual path
  -> unsupported/error? fail open to native command when safe
```

## Why it exists

Agents frequently run commands such as build checks, tests, search, status, and diagnostics. Raw output can overwhelm context and hide the useful lines.

RTK helps by:

- condensing noisy but routine command output
- preserving key errors/warnings/actionable lines
- avoiding repeated huge logs in the conversation
- reducing token waste during iterative debugging
- keeping fallback behavior predictable

## Project policy

Project-local policy lives at:

```text
.aoc/rtk.toml
```

New AOC projects default to RTK `mode = "on"`. Existing projects that explicitly set `mode = "off"` keep that choice.

Common commands:

```bash
aoc-rtk status
aoc-rtk doctor
aoc-rtk enable
aoc-rtk disable
aoc-rtk install --auto
```

## Routing modes

| Mode | Meaning |
|---|---|
| `on` | Route allowlisted commands through RTK where available |
| `off` | Use native shell commands directly |
| missing/stale RTK binary | Fail open to native behavior for safe commands |

## Policy shape

A project policy normally defines:

- mode
- pinned RTK binary/install contract
- allowlisted command patterns
- denylisted risky/destructive patterns
- fallback behavior
- output condensation rules

Exact schema can evolve; use `aoc-rtk doctor` to validate a project.

## Allowlisted commands

Allowlisted commands are commands where condensed output is helpful and semantic risk is low. Examples usually include:

- `cargo check`, `cargo test`, selected build commands
- `npm test`, `pnpm test`, lint/typecheck commands
- `rg`, `find`, status/diagnostic commands
- AOC status/doctor/check commands that can be noisy

RTK should preserve:

- command exit code
- primary errors
- failing test names
- useful warnings
- file paths and line numbers
- final summary/counts

## Denylisted commands

Risky commands should not be silently routed or rewritten. Examples:

- publish/deploy/release commands
- destructive package/cache operations
- restore/reset/clean/prune commands
- credential or auth mutation commands
- unknown shell fragments with unclear side effects

When in doubt, use native command execution and explicit operator confirmation.

## Fail-open behavior

RTK is designed to improve output quality, not block work. If RTK is missing, stale, or cannot parse a safe allowlisted command, AOC should fall back to native execution instead of failing the task.

Fail-open does not apply to known risky commands. Risky/destructive operations still require normal explicit confirmation and should not be hidden behind routing.

## Agent behavior

Agents should:

1. Prefer narrow targeted commands first.
2. Let AOC route allowlisted noisy checks through RTK when enabled.
3. Summarize only actionable output.
4. Escalate to native/full logs only when needed.
5. Use `aoc-rtk status` or `aoc-rtk doctor` when routing looks suspicious.

Agents should not treat RTK-condensed output as a replacement for full logs when a failure requires detailed diagnosis.

## Troubleshooting

Check status:

```bash
aoc-rtk status
```

Run diagnostics:

```bash
aoc-rtk doctor
```

Disable temporarily:

```bash
aoc-rtk disable
```

Re-enable:

```bash
aoc-rtk enable
```

Install/repair pinned binary:

```bash
aoc-rtk install --auto
```

## Boundaries

RTK is separate from:

- AOC Mind: Mind stores/retrieves project knowledge; RTK condenses command output.
- Taskmaster: tasks track work; RTK may condense task command output.
- Mission Control: Mission Control observes session/runtime state; RTK operates at command execution/output level.
- Security controls: RTK is not a secret scanner or sandbox.

## Related docs

- [Project contract](project-contract.md)
- [Configuration details](configuration-details.md)
- [Troubleshooting](../troubleshooting.md)
