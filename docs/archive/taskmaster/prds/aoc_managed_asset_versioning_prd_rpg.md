# AOC Managed Asset Versioning PRD

## Role

AOC is the source-of-truth development pipeline for AOC system assets. Consumer projects run `aoc-init` to install, repair, and update managed AOC assets to the current version.

## Problem

AOC currently refreshes some managed directories by convention, but the managed/unmanaged boundary is implicit. This creates risk:

- AOC-managed skills, presets, extensions, prompts, and layouts may drift stale in consumer projects.
- Local edits inside managed AOC system assets may be overwritten without a clear marker/backup story.
- Agents cannot reliably tell whether a file is project-authored or AOC-owned.
- Optional systems such as HyperFrames need latest factory skill/preset behavior after `aoc-init` without encouraging project-local tinkering.

## Goal

Make AOC managed assets version-aware and safely updateable.

Core contract:

```text
agent-ops-cockpit repo = source of truth
consumer project = receives managed assets via aoc-init/aoc-* sync
AOC-managed asset old/missing = update
project-authored artifact = preserve
locally modified managed asset = backup/warn before replace
```

## Non-goals

- Do not version project artifacts such as `DESIGN.md`, `hyperframes/docs/*`, `hyperframes/compositions/**`, render outputs, or task state as managed AOC assets.
- Do not implement full three-way merge in first pass.
- Do not use Mind as the first-layer update detector.

## Managed asset classes

Managed by AOC:

```text
.pi/skills/<aoc skill>/**
.pi/extensions/**
.aoc/presets/**
.aoc/layouts/**
.pi/prompts/<aoc prompt>.md
.aoc/skills-optional/** templates distributed by this repo
bin/aoc-* installed/synced by AOC packaging
```

Preserved project artifacts:

```text
.aoc/memory.md
.aoc/stm/**
.taskmaster/** task state
DESIGN.md after initial seed
hyperframes/docs/** after workspace seed
hyperframes/compositions/**
hyperframes/assets/**
hyperframes/renders/**
```

## Marker contract

Every managed text file should eventually carry an in-file marker near the top.

Markdown skill example:

```md
---
name: aoc-hyperframes
description: Umbrella production mode for HyperFrames.
aocManaged: true
aocAsset: skill/aoc-hyperframes
aocAssetVersion: 3
---
```

Comment-format example:

```text
# AOC-MANAGED: true
# AOC-ASSET: preset/aoc-hyperframes
# AOC-ASSET-VERSION: 3
# AOC-SOURCE: .aoc/presets/hyperframes/preset.toml
```

Directory sidecar marker for first-pass safety:

```text
.aoc-managed
```

with:

```yaml
aoc-managed: true
asset: preset/aoc-hyperframes
asset-version: 3
source: agent-ops-cockpit
updated-at: <utc>
```

## Update behavior

`aoc-init` / sync tools must follow:

```text
missing managed target -> copy current seed + write marker
managed target current -> leave or refresh if source differs
managed target old -> replace from source + update marker
managed target modified -> backup then replace, or warn and preserve until --force where destructive
unmanaged target -> preserve unless known legacy AOC asset during migration
```

First implementation may use sidecar directory markers plus known legacy allowlists. In-file markers follow for text files where runtime schemas permit it.

## HyperFrames integration

HyperFrames support must follow the same managed boundary:

- `.pi/skills/aoc-hyperframes/**` is AOC-managed and updated by AOC.
- `.aoc/presets/hyperframes/**` is AOC-managed and updated by AOC.
- `hyperframes/**` workspace content is project artifact after seed and must not be overwritten blindly.
- `aoc-hyperframes sync-skills` may replace only managed skill dirs or absent dirs.

## Phase 2: package-manager grade detection

Managed updates should be explainable and checksum-backed.

### In-file markers

Add portable identity markers to managed text files where schemas permit it.

Benefits:

- File identity survives copy/move without sidecar.
- Agents/reviewers see `aocManaged` directly in file headers.
- Single-file assets become self-describing.

### Checksums

Record source and installed checksums for managed files.

Benefits:

- Clean old asset -> safe auto-update.
- Dirty managed asset -> backup/warn before replace.
- Current but locally modified asset -> visible drift warning.

### Managed manifest index

Add project-local index:

```text
.aoc/managed-assets.json
```

Suggested fields:

```json
{
  "schemaVersion": 1,
  "aocAssetVersion": 3,
  "updatedAt": "...",
  "assets": {
    ".pi/skills/aoc-hyperframes/SKILL.md": {
      "asset": "skill/aoc-hyperframes",
      "source": ".pi/skills/aoc-hyperframes/SKILL.md",
      "sha256": "...",
      "status": "current"
    }
  }
}
```

Benefits:

- Fast status checks.
- Clear stale/dirty/current counts.
- `aoc-init --check-managed` UX.
- Future `aoc-init --update-managed` UX.

### Dirty-file comparison

Implement update decision matrix:

```text
installed checksum == previous source checksum -> replace silently
installed checksum != previous source checksum -> backup + replace or warn
source checksum == installed checksum -> current
marker missing -> unmanaged/preserve unless legacy migration path
```

## Acceptance criteria

- Task exists with subtasks for managed marker design, init-state version bump, safe sync wrappers, in-file marker support, HyperFrames sync hardening, docs/tests.
- `aoc-init` records a v3 migration for managed asset versioning.
- Managed directory refreshes write `.aoc-managed` sidecar markers.
- HyperFrames skill sync no longer blindly `rm -rf`s an unmarked local skill dir.
- `aoc-skill validate` accepts AOC marker frontmatter keys.
- `.aoc/managed-assets.json` records managed asset checksum/status metadata.
- `aoc-init --check-managed` reports current/dirty/unknown managed asset counts.
- Locally modified managed directories are backed up before refresh when prior checksum proves drift.
- Docs explain source-of-truth policy.

## Test strategy

- `bash -n bin/aoc-init bin/aoc-hyperframes bin/aoc-skill`
- Temp project: run `aoc-init`; verify managed markers under `.pi/extensions/aoc-presets`, `.aoc/presets/design`, optional HyperFrames preset when present.
- Temp project: simulate unmarked `.pi/skills/aoc-hyperframes`; `aoc-hyperframes sync-skills` preserves or backs it up instead of destructive overwrite.
- Temp project: managed `.pi/skills/aoc-hyperframes/.aoc-managed` refreshes from current source.
