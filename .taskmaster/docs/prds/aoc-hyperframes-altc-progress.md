# PRD: AOC HyperFrames Alt+C Progress Integration

## Summary

Make Alt+C HyperFrames setup fully operational for Pi users: the existing `Init` action should perform the full production bootstrap, and long-running setup/doctor/sync actions should stream progress into the right-side details pane instead of freezing the TUI.

## Problem

AOC HyperFrames now has a production bootstrap (`bootstrap-asset-system`) that creates canonical folders and docs, including `assets/maps` and `docs/DESIGN.md`. However, the Alt+C HyperFrames action still runs only `aoc-hyperframes init`, which previously did not invoke the production bootstrap, and the TUI runs HyperFrames commands synchronously with no live progress view.

Users need to see install/load progress in the Alt+C details frame while HyperFrames installs, syncs skills, runs doctor checks, or bootstraps docs.

## Goals

- `aoc-hyperframes init` performs full production bootstrap after workspace/skill/prompt setup.
- Alt+C HyperFrames actions run as background jobs where appropriate.
- The right details pane shows current/recent HyperFrames console logs.
- Operators can scroll logs, cancel a running HyperFrames job, and open the full log in a pager.
- Labels/help text reflect production bootstrap, not just workspace init.

## Non-goals

- Do not implement full task/job framework for every Alt+C command.
- Do not change HyperFrames preview pane behavior beyond preserving current preview launch.
- Do not overwrite existing HyperFrames docs/assets.

## UX

Alt+C → Tools → HyperFrames should show:

- `Init workspace + production bootstrap`
- `Sync HyperFrames PI skills only`
- `Run HyperFrames doctor`
- `Start preview pane`

When a HyperFrames job runs, the Details pane should display:

- action name
- log path
- key hints: `PgUp/PgDn scroll · x cancel · Shift+O open full log`
- recent output tail
- latest log path after completion

## Acceptance criteria

1. `aoc-hyperframes init` creates/repairs canonical production folders/docs by invoking `bootstrap_asset_system`.
2. Existing docs, especially `docs/DESIGN.md`, remain preserved on repeated init/bootstrap.
3. Alt+C HyperFrames init/sync/doctor actions spawn background jobs and return control immediately.
4. The Details pane renders live/recent HyperFrames log output.
5. PgUp/PgDn, `x`, and Shift+O work for HyperFrames details logs.
6. `cargo check -p aoc-control` passes.
7. `bash -n bin/aoc-hyperframes` and temp bootstrap preservation checks pass.

## Taskmaster

Primary task: `199` — Fully integrate HyperFrames bootstrap and Alt+C progress logs.
