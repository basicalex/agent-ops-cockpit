# Managed assets

AOC may refresh repo-owned assets it can prove are AOC-managed. Project-authored work is preserved by default.

Managed by default:

```text
.omp/skills/<aoc skill>/**
.omp/extensions/**
.omp/agents/**
.aoc/presets/**
```

Project-authored, preserve by default:

```text
docs/** outside generated AOC docs
source code outside managed AOC paths
```

Managed markers use `.aoc-managed` files with asset id, source, checksum, and timestamp. Example:

```text
aoc-managed: true
asset: skill/aoc-init-ops
asset-version: 3
source: .omp/skills/aoc-init-ops
sha256: <installed tree/file sha256>
updated-at: <utc>
```
