# Managed assets

AOC may refresh repo-owned assets it can prove are AOC-managed. Project-authored work is preserved by default.

Managed by default:

```text
.omp/skills/<aoc skill>/**
.omp/extensions/**
.omp/agents/**
.aoc/presets/**
.aoc/layouts/**
```

Project-authored, preserve by default:

```text
hyperframes/**
docs/** outside generated AOC docs
source code outside managed AOC paths
```

Managed markers use `.aoc-managed` files with asset id, source, checksum, and timestamp. Example:

```text
aoc-managed: true
asset: skill/aoc-hyperframes
asset-version: 3
source: .omp/skills/aoc-hyperframes
sha256: <installed tree/file sha256>
updated-at: <utc>
```

If a managed target changed since the last recorded checksum, AOC backs it up before refresh. HyperFrames uses the same rule: `.omp/skills/aoc-hyperframes/**` and `.aoc/presets/hyperframes/**` are managed; `hyperframes/**` workspace content is project-authored after seed.
