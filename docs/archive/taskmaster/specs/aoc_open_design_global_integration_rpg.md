# Spec: AOC Open Design Global Integration

## Role
AOC operator who wants a high-quality visual design studio instead of prompt-only design guidance.

## Problem
AOC can route design intent and preserve project intelligence, but it does not provide a GUI-first design iteration loop. Open Design (OD) is a stronger fit for visual prototyping, design-system selection, decks, templates, and design artifact iteration. AOC should not duplicate OD. AOC should install/run/contextualize OD and then import polished OD artifacts for implementation, HyperFrames campaigns, provenance, and task handoff.

## Goal
Add an AOC-managed Open Design bridge that lets an operator run a globally installed OD studio from any project, link it to the current project context, and import OD artifacts into stable project-local AOC paths.

## Non-goals
- Do not vendor Open Design into every project.
- Do not replace AOC presets, Taskmaster, STM, Mind, or HyperFrames factory flows.
- Do not silently configure secrets or OAuth credentials.
- Do not auto-enable OD across projects without explicit operator action.
- Do not implement a full OAuth proxy in v1; discover/configure provider hooks first.

## Product model
- OD = GUI design studio and high-quality artifact iteration engine.
- AOC = project OS, context, provenance, tasking, implementation handoff, campaign scale-out.
- HyperFrames = durable video/campaign factory for project-local reusable media systems.

## User journey
1. From any AOC project, operator runs `aoc-od install` once to clone/build a pinned global OD checkout.
2. Operator runs `aoc-od start` from a project root.
3. AOC writes `.aoc/open-design/link.json` describing the project, design contract, expected OD workspace locations, and import target.
4. OD opens/runs as the GUI studio.
5. Operator iterates designs in OD.
6. Operator runs `aoc-od import latest` to copy the latest OD artifact into `design-artifacts/od/<artifact>/` and update `.aoc/open-design/artifacts.json`.
7. AOC uses imported artifact paths for implementation, Taskmaster specs, Mind/STM provenance, or HyperFrames campaign expansion.

## Acceptance criteria
- `bin/aoc-od` exists and is installed by the normal AOC bin install loop.
- `aoc-od --help` documents install/status/start/run/stop/open/link/import/doctor.
- `aoc-od link` creates `.aoc/open-design/link.json` without overwriting unrelated project files.
- `aoc-od import latest` copies latest `.od/artifacts/*` artifact to `design-artifacts/od/*` and writes `.aoc/open-design/artifacts.json`.
- `aoc-od status` and `aoc-od doctor` are safe read-only diagnostics.
- `aoc-od install` is explicit before network clone/install/build.
- Docs explain OD as optional global design backend for AOC, provider/auth caveats, project artifact flow, and HyperFrames handoff.
- Targeted shell tests pass.

## Test strategy
- `bash -n bin/aoc-od`.
- Run `aoc-od --help`.
- Run `aoc-od doctor` without requiring OD installed.
- In a temp project, create a fake `.od/artifacts/<name>/index.html`, run `aoc-od link`, run `aoc-od import latest`, assert copied artifact and metadata files exist.
- Run existing preset smoke test to ensure preset system remains valid.

## Security / auth notes
Open Design runs a local daemon and can spawn agent CLIs. Installation/start must remain explicit. OAuth/OpenAI account reuse is not assumed in v1. Preferred future path is an AOC-managed local OpenAI-compatible proxy backed by existing AOC/Pi auth, but only after verifying OD provider hooks and secret handling.
