# Project contract

Canonical AOC project surfaces:

```text
.aoc/                        # AOC context, memory CLI state, presets, layouts, managed metadata
.taskmaster/                 # Taskmaster tasks, tags, specs/PRDs
.omp/extensions/             # repo-owned AOC OMP extension sources
.omp/agents/                 # repo-owned AOC OMP agent template sources
.omp/skills/                 # repo-owned AOC OMP skill sources
AGENTS.md                    # agent contract
DESIGN.md                    # root product/design contract
```

`.gitignore` must keep repo-owned AOC/OMP sources trackable:

```text
!/.aoc/
!/.aoc/**
!/.taskmaster/
!/.taskmaster/**
!/.omp/
!/.omp/extensions/
!/.omp/extensions/**
!/.omp/agents/
!/.omp/agents/**
!/.omp/skills/
!/.omp/skills/**
```

High-churn/runtime artifacts remain ignored:

```text
/.aoc/logs/
/.aoc/mind/
/.aoc/tools/
/.taskmaster/logs/
/.taskmaster/**/*.log
/.taskmaster/**/*.lock
**/.aoc-backups/
.codegraph/
```

Global/operator OMP runtime config lives outside the repo at `~/.omp/agent/config.yml` and may be created by the operator or upstream OMP tooling.
