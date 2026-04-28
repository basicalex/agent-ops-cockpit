# AOC Managed Assets

AOC system assets are authored in this repository and distributed to projects by `aoc-init` and related AOC commands.

## Source-of-truth rule

```text
agent-ops-cockpit repo = edit here
consumer project = receive/update via aoc-init
```

Do not hand-edit managed AOC system internals inside consumer projects unless debugging.

## Managed vs project-authored

Managed by AOC:

```text
.pi/skills/<aoc skill>/**
.pi/extensions/**
.aoc/presets/**
.aoc/layouts/**
.pi/prompts/<aoc prompt>.md
.pi/packages/pi-multi-auth-aoc/**
```

Project-authored, preserve by default:

```text
DESIGN.md
.aoc/memory.md
.aoc/stm/**
.taskmaster/** task state
hyperframes/docs/**
hyperframes/compositions/**
hyperframes/assets/**
hyperframes/renders/**
```

## Marker contract

Managed directories receive:

```text
.aoc-managed
```

Managed files may receive sidecar markers:

```text
<file>.aoc-managed
```

Marker fields:

```yaml
aoc-managed: true
asset: skill/aoc-hyperframes
asset-version: 3
source: .pi/skills/aoc-hyperframes
sha256: <installed tree/file sha256>
updated-at: <utc>
```

## Managed manifest

`aoc-init` writes:

```text
.aoc/managed-assets.json
```

It tracks:

```text
asset id
asset version
source path
installed checksum
expected checksum
status: current | dirty | unknown
marker path
```

## Status check

```bash
aoc-init --check-managed
```

Outputs managed asset counts via existing status surface:

```text
managed_assets_current: N
managed_assets_dirty: N
managed_assets_unknown: N
```

## Update behavior

```text
missing managed asset -> seed latest + marker + manifest
clean managed asset -> refresh latest
current managed asset -> leave/mark current
locally modified managed asset -> backup before refresh
unmarked local managed-path edit -> backup before refresh
project artifact -> preserve
```

HyperFrames uses same rule: `.pi/skills/aoc-hyperframes/**` and `.aoc/presets/hyperframes/**` are managed; `hyperframes/**` workspace content is project-authored after seed.
