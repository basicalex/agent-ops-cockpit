# PI-only Rollout Checklist

Use this checklist when cutting and validating the PI-only surface release.

## 1) Pre-release closeout

- Confirm task scope is complete:
  - `aoc-agent`, `aoc-agent-run`, `aoc-agent-install`, and `aoc-control` accept only `pi`.
  - Legacy non-PI wrappers are removed from `bin/`.
  - `install.sh` prunes retired wrapper artifacts from previous installs.
- Run validation:

```bash
bash scripts/pi/test-aoc-init-pi-first.sh
bash scripts/pi/test-pi-only-agent-surface.sh
aoc-skill validate --root .
cargo check --manifest-path crates/Cargo.toml -p aoc-control
```

## 2) Release notes + user notice timing

- Changelog entry must state:
  - PI-only runtime support is final.
  - Legacy wrapper commands are removed (not just deprecated).
  - Installers and selectors support only `pi`.
- Publish release note with operator action:
  - Run `./install.sh` (or bootstrap installer) once to remove retired wrappers in local bin paths.

## 3) Post-release verification

- Fresh install smoke:
  - bootstrap install
  - `aoc-doctor`
  - `aoc-agent --set` shows only `pi`
- Existing install migration smoke:
  - upgrade via `./install.sh`
  - verify retired wrappers are absent from `~/.local/bin` (and `~/bin` when previously managed by AOC)
  - `aoc-agent-install status pi` returns `installed|missing`

## 4) Rollback guardrail

- If PI-only release must be rolled back, revert to previous tag and rerun installer.
- Do not reintroduce non-PI wrappers on mainline after rollback; fix forward with PI-only behavior.
