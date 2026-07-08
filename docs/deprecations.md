# Deprecations and removals

This page tracks intentional simplifications in the OMP-first, Herdr-first AOC surface. Historical removals are listed here so active docs do not keep describing retired cockpit workflows.

## Runtime surface

Removed from the active operator path:

- Legacy Pi settings, prompts, skills, extensions, packages, and agents.
- Multi-runtime wrapper behavior in core agent commands.
- Zellij cockpit-only layout/theme sync surfaces.
- Zellij launcher and tab scripts: `bin/aoc-launch`, `bin/aoc-new-tab`, `bin/aoc-zellij.sh`, `bin/aoc-zellij-resize`, and `bin/aoc-layout`.
- Zellij layout/config templates: `zellij/aoc.config.kdl.template`, `zellij/layouts/aoc.kdl.template`, and seeded `.aoc/layouts/*.kdl` / `~/.config/zellij/layouts/aoc.kdl` assets.
- Mission Control, Control pane, and legacy hub/session UI crates and entrypoints: `crates/aoc-mission-control`, `crates/aoc-control`, `crates/aoc-hub-rs`, `bin/aoc-mission-control*`, `bin/aoc-control*`, `bin/aoc-hub`, `bin/aoc-session-state`, `bin/aoc-pane-evidence`, and `bin/aoc-pulse-pane`.
- AOC subagent manager/control surfaces such as `bin/aoc-subagent-supervision*` and their runtime reference docs.
- Zellij cleanup/inventory helpers: `bin/aoc-cleanup`, `bin/aoc-cleanup-core.py`, and Zellij session inventory helpers.
- Legacy-zellij install/init flags and compatibility paths such as `./install.sh --legacy-zellij` and `AOC_LEGACY_ZELLIJ=1 aoc`; they are no longer part of the supported Herdr-first path.
- Mind subsystem: crates `aoc-mind`, `aoc-pi-adapter`, `aoc-task-attribution`, `aoc-segment-routing`, and `aoc-agent-wrap-rs`; the `aoc-mind-service` binary; and the `.omp/extensions/aoc-mind.ts` extension.
- Pi agent lifecycle scripts: `aoc-pi`, `aoc-agent`, `aoc-agent-run`, `aoc-agent-wrap`, `aoc-agent-install`, and the installed pi shim.
- Lexicon feature: the `aoc-lexicon` skill, `.aoc/lexicon.md`, `aoc-voxtype-setup`, and `voxtype-aoc-lexicon-filter`.

## Canonical supported runtime

- Herdr workspace/runtime surface: `aoc`, `aoc services`, `aoc-herdr-launch`, `aoc-herdr-services`.
- OMP runtime config: `~/.omp/agent/config.yml`.
- Project OMP sources: `.omp/extensions/`, `.omp/agents/`, `.omp/skills/`.
- AOC state/contracts: `.aoc/**`, `.taskmaster/**`, `AGENTS.md`, `DESIGN.md`.

`aoc-init` no longer migrates or recreates legacy Pi paths or retired cockpit assets.

## Skill sync behavior

Kept:

- OMP-only validation/sync via `aoc-skill sync --root .` and `aoc-skill validate --root .`.

Removed:

- Legacy Pi skill sync and prompt alias cleanup.
