# Repository Guidelines

## Project Overview

AOC (Agent Ops Cockpit) is now a Herdr/OMP-first tooling layer for AI workspaces: launch convenience, metadata-only startup context, Taskmaster workflows, CodeGraph, and optional project tools such as HyperFrames/OpenDesign/Understand/web research/RTK. Legacy Zellij cockpit pieces are compatibility-only.

## Architecture & Data Flow

- `bin/aoc` is the primary entrypoint. It forwards Rust CLI subcommands (`task`, `mem`, `rlm`, `insight`, `overseer`, `map`, `see`) to `aoc-cli`; `aoc omp` launches OMP with an AOC context capsule; default launch is `aoc-herdr-launch`; legacy Zellij is explicit compatibility via `AOC_LEGACY_ZELLIJ=1 aoc`.
- Project initialization flows through `bin/aoc-init`, which creates/updates `.aoc/`, `.taskmaster/`, `.pi/`, context, presets, prompts/skills, Taskmaster, RTK, OMP extensions, and kept tooling. It must not seed Zellij layouts, topbar, Mission Control, Control pane, subagent manager UI, AOC status/health panes, or tab metadata by default.
- Rust crates under `crates/` provide durable tooling:
  - `aoc-core`: shared domain types, status/priority parsing, Mind/session/provenance/pulse/Zellij modules.
  - `aoc-storage`: SQLite-backed Mind store and migrations.
  - `aoc-mind`: optional ingestion, retrieval, context packs, distillation, provenance, session export/finalization.
  - `aoc-cli`: `aoc` subcommands for task/memory/insight/overseer/map flows.
  - `aoc-agent-wrap-rs`: keep only where it remains useful for OMP-native lifecycle/context/provenance without Zellij coupling.
  - `aoc-taskmaster`: retained Taskmaster TUI/helper.
  - `aoc-hub-rs`, `aoc-mission-control`, and `aoc-control`: legacy compatibility surfaces, not default Herdr/OMP data flow.
- TypeScript extensions under `.pi/extensions/` integrate Pi tools, presets, Mind, compaction, models, and CodeGraph. Do not seed AOC subagent manager or AOC agent-presence/status extensions by default. `.omp/extensions/aoc-codegraph.ts` exposes read-only CodeGraph actions for OMP; `.omp/extensions/aoc-commit.ts` registers `/commit` for the safe atomic commit workflow.
- Data flow is generally: launcher/init scripts prepare repo-local state → Herdr owns workspace/pane/status UI → OMP/Pi agents receive metadata-only AOC context when requested → Taskmaster and optional Mind/RTK/tool CLIs remain source-of-truth interfaces.

## Key Directories

- `bin/`: Bash/Python command surface and launchers (`aoc`, `aoc-init`, `aoc-herdr-*`, `aoc-omp`, `aoc-task`, `tm`, `aoc-mem`, `aoc-stm`, `aoc-rtk`).
- `crates/`: Rust workspace; run Cargo commands with `--manifest-path crates/Cargo.toml` or from `crates/`.
- `.pi/`: Pi runtime assets, extensions, prompts, skills, and package shims. Keep canonical `.pi/**` assets tracked except high-churn ignored paths.
- `.omp/extensions/`: OMP-native extensions, currently including AOC CodeGraph.
- `.aoc/`: project-local AOC config/context/presets. Layouts are legacy-only; do not seed or rely on `.aoc/layouts` for default Herdr/OMP work.
- `.taskmaster/`: Taskmaster state/specs. Use `tm`/`aoc-task`; do not edit task JSON directly.
- `scripts/`: CI, smoke, lint, legacy Zellij, Pi, Mind, and web-research validation scripts.
- `docs/`: user/operator/maintainer docs. `docs/herdr-workspace.md` and `docs/aoc-feature-inventory.md` describe the Herdr-first cutover.
- `herdr/`: lean Herdr config baseline installed by `aoc-herdr-install`.
- `config/`, `legacy/`: config templates and retained legacy compatibility assets.

## Development Commands

Use targeted commands for the area you changed.

```bash
# Install / local setup
./install.sh              # Herdr/OMP-first default
./install.sh --mind       # optional Mind/Pi runtime
./install.sh --legacy-zellij
aoc-herdr-install
aoc-doctor
aoc-init

# Launch
aoc                       # Herdr-first launcher
aoc omp                   # OMP with AOC startup capsule
/commit [scope]            # in OMP: inspect, validate, stage explicit paths, commit; never push
AOC_LEGACY_ZELLIJ=1 aoc   # legacy cockpit only

# Root JS/design tooling
pnpm run design:lint

# Rust workspace
cd crates && cargo fmt --check
cargo clippy --workspace --manifest-path crates/Cargo.toml
cargo test --workspace --manifest-path crates/Cargo.toml
cargo build --workspace --manifest-path crates/Cargo.toml

# Shell and smoke checks
bash ./scripts/lint.sh
bash ./scripts/smoke.sh
bash ./scripts/zellij/test-managed-plugin.sh  # legacy Zellij compatibility only
bash ./scripts/test-web-research-stack.sh
```

Examples of focused Rust checks observed in scripts/docs:

```bash
cargo test -p aoc-mind --manifest-path crates/Cargo.toml
cd crates && cargo test -q -p aoc-agent-wrap-rs <test_name> -- --nocapture
# Legacy-only: cargo check --manifest-path crates/Cargo.toml -p aoc-control
```

## Code Conventions & Common Patterns

- Shell scripts use `#!/usr/bin/env bash` with `set -euo pipefail`; prefer small helper functions, explicit env overrides, temp dirs for tests, and fail-fast error messages naming the failed command/action.
- Python helper scripts should be syntax-checked with `python3 -m py_compile` when changed; web/search/render scripts use JSON-ish CLI behavior and explicit status/error output.
- Rust uses workspace crates with `resolver = "2"`; keep shared contracts in `aoc-core`, durable persistence in `aoc-storage`, runtime/query behavior in `aoc-mind`, and UI coordination in Ratatui crates. Prefer typed domain parsing over ad-hoc strings.
- TypeScript extensions use TypeBox schemas, explicit tool parameter types, bounded output, path scoping, and read-only defaults for discovery tools. Do not let agent tools silently initialize/sync/index expensive systems.
- Error handling should be operational: tell the operator what failed, where logs/state live, and what repair command to run.
- State management is repo-local and CLI-mediated. Use `.aoc/context.md` for orientation; use `aoc-handshake --json`/`--prompt` for startup metadata; use `aoc-mem`, `aoc-stm`, `tm`, and `aoc-task` instead of direct reads/edits of protected state.
- Keep product-facing language aligned with `DESIGN.md`: calm, concise, trustworthy, and terminal-native. Read `DESIGN.md` before UI/docs-presentation/theme/HyperFrames work.

## Important Files

- `README.md`: product overview, install flow, requirements, common operator commands.
- `AOC.md`: compressed repo overview and common workflow summary.
- `DESIGN.md`: product/UI/docs tone and design contract.
- `CHANGELOG.md`: current migration/compatibility notes.
- `package.json`, `pnpm-lock.yaml`: minimal JS/design tooling; package manager is pnpm.
- `crates/Cargo.toml`, `crates/Cargo.lock`: Rust workspace and lockfile.
- `.github/workflows/ci.yml`: canonical CI commands.
- `install.sh`, `install/bootstrap.sh`: local and remote install paths. Default install is Herdr/OMP-first; use `--mind` or `--legacy-zellij` for optional compatibility layers.
- `bin/aoc`, `bin/aoc-init`, `bin/aoc-herdr-launch`, `bin/aoc-herdr-install`, `bin/aoc-omp`, `bin/aoc-handshake`: launch/init/context core.
- `.omp/extensions/aoc-codegraph.ts`, `.omp/extensions/aoc-commit.ts`: OMP CodeGraph and `/commit` integration.
- `.pi/settings.json`: Pi extension/model/skill configuration.
- `docs/herdr-workspace.md`, `docs/aoc-feature-inventory.md`: Herdr-first direction and retained/retired feature inventory.

## Runtime/Tooling Preferences

- Required baseline from docs: Linux/macOS/WSL, Git, Herdr, and OMP/Pi coding agent CLI. Zellij is legacy compatibility only.
- Current launcher preference is Herdr + OMP/Pi; use legacy Zellij only when explicitly testing/working on that path.
- JS package manager is `pnpm@10.33.3`; do not switch to npm/yarn for repo scripts.
- Rust stable is used in CI with `clippy`, `rustfmt`, and `wasm32-wasip1` target.
- Optional tools/features: Docker for managed local search, Node.js `>=22` and FFmpeg for HyperFrames, `bun` for some Pi extension smoke scripts, `shellcheck` for shell linting.
- Installer uses user-local paths: `~/.local/bin`, `${XDG_CONFIG_HOME:-~/.config}/aoc`, `${XDG_STATE_HOME:-~/.local/state}/aoc`; it may bootstrap Rust unless `AOC_INSTALL_RUST=0`.
- Generated/high-churn paths are ignored, including `node_modules/`, `target/`, `crates/target/`, `.aoc/logs/`, `.aoc/**/*.lock`, `.aoc/mind/**`, `.taskmaster/logs/`, `.pi/tmp/`, `.codegraph/`, `__pycache__/`, and `**/.aoc-backups/`.

## Testing & QA

- There is no root `test` script in `package.json`; do not invent one. Use Cargo and targeted smoke scripts.
- Rust unit tests live mainly in crate modules such as `crates/aoc-mind/src/tests.rs` and `crates/aoc-mission-control/src/tests.rs` and use built-in `#[test]` with temp dirs/in-memory stores where possible.
- Script QA lives under `scripts/` and `scripts/pi/`; common pattern is isolated temp fixtures plus `set -euo pipefail`.
- For Mind/runtime/export changes, consider targeted scripts such as `scripts/pi/validate-mind-runtime-hardening.sh` and `scripts/verify-mind-runtime-safety.sh`.
- For Zellij/plugin changes, run `bash scripts/zellij/test-managed-plugin.sh`; this is legacy compatibility only.
- For shell changes, run `bash ./scripts/lint.sh`; it requires `shellcheck`.
- For product/design doc changes touching `DESIGN.md`, run `pnpm run design:lint`.
- Verification should match the changed area: prefer focused crate tests or smoke scripts before full workspace CI.
