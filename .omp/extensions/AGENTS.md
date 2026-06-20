# Repository Guidelines

Scope: `.omp/extensions`

## Local Contracts
- Expose Pi capabilities only through `ExtensionAPI.registerTool`/`registerCommand`; keep tool parameters typed with TypeBox/StringEnum schemas and keep slash-command arguments routed through explicit modes, aliases, and completions.
- For subprocess-backed tools, scope cwd under the project root, bound/truncate output, enforce timeout and AbortSignal cleanup, use `spawn(..., { shell:false })`, and surface nonzero/timeout results as unavailable evidence rather than success.
- Slash-command extensions should hand workflow prompts to the agent with `pi.sendMessage({ customType, display:true, content, details }, { triggerTurn:true })`; include cwd/scope in details and use `ctx.ui.notify` only as fallback.
- Tool descriptions and `promptGuidelines` must encode operational limits and write-safety for each exposed capability; write/apply/install/init/sync-style actions require a safe schema mode and matching prompt guidance before exposure.
- Autonomous worker-to-master prompting belongs in `aoc-master.ts` only, must be gated by typed schemas and master-owned full-retard state, and must preserve bounded output/timeouts plus no arbitrary Herdr host actions.

## Verification
- `bun --check .omp/extensions/<changed-extension>.ts`

## Do Not
- Do not add ad-hoc string-command parsing, untyped parameter bags, or hidden host actions outside the registered Pi extension API.
- Do not expose apply/write/install/init/index/sync actions by name or implication unless the safe mode is encoded in both schema and prompt guidance; do not turn AGENTS output into general documentation.
- Do not introduce `shell:true`, cwd escape paths, unbounded stdout/stderr, synchronous long-running subprocesses on agent-facing paths, or fake-success fallbacks after CLI failure.
- Do not make slash commands mutate the project directly, bypass the agent turn, omit `customType`/`details`, or rely on UI notification when `sendMessage` is available.

## Update When
- Update when adding or changing `.omp/extensions/*.ts` command names, tool schemas, argument parsing, aliases, or completions.
- Update when adding or changing a subprocess-backed tool, cwd parameter, timeout/output limit, or wrapper around AOC/CodeGraph/Mind/search CLIs.
- Update when adding or changing slash commands, workflow prompts, `customType` names, or sendMessage details payloads.
- Update when adding tool actions, changing promptGuidelines, or broadening a wrapper from read-only/dry-run to a mutating capability.
