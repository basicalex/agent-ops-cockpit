# AOC project contract

This page summarizes the project-local files AOC expects after `aoc-init`.

## Core paths

```text
.aoc/context.md              # generated project snapshot
.aoc/memory.md               # durable project memory, managed by aoc-mem
.aoc/stm/current.md          # short-term handoff draft, managed by aoc-stm
.aoc/rtk.toml                # RTK routing policy
.aoc/mind-service.json       # project Mind launcher metadata
.taskmaster/                 # Taskmaster tasks, tags, specs/PRDs
.pi/settings.json            # project Pi settings
.pi/prompts/                 # project Pi prompts
.pi/skills/                  # project Pi skills
.pi/extensions/              # project Pi extensions
.omp/extensions/             # repo-owned AOC OMP extension sources
.omp/agents/                 # repo-owned AOC OMP agent template sources
DESIGN.md                    # root product/design contract; Google Labs design.md-compatible YAML tokens + prose
AGENTS.md                    # agent rules for the repo
```

## Git/Jujutsu tracking policy

AOC project source/state should be tracked:

```gitignore
!/.aoc/
!/.aoc/**
!/.taskmaster/
!/.taskmaster/**
!/.pi/
!/.pi/**
!/.omp/
!/.omp/extensions/
!/.omp/extensions/**
!/.omp/agents/
!/.omp/agents/**
```

Keep runtime/churn state ignored:

```gitignore
/.aoc/logs/
/.aoc/**/*.log
/.aoc/**/*.lock
/.aoc/mind/
/.taskmaster/logs/
/.taskmaster/**/*.log
/.taskmaster/**/*.lock
/.pi/tmp/
/.pi/packages/pi-multi-auth-aoc/debug/
**/.aoc-backups/
.codegraph/
```

Do not commit secrets, tokens, private logs, live Mind databases, lock files, caches, or OMP runtime state from `~/.omp/agent`.

## Commands

```bash
aoc-init                  # initialize/repair project contract
aoc-init --status         # summarize project readiness
aoc-handshake --json      # agent startup metadata
aoc state status          # read-only audit of trackable project state
aoc-skill validate --root .
aoc-doctor
pnpm design:lint          # validate root DESIGN.md against Google Labs design.md format
```

## HyperFrames add-on

When HyperFrames is enabled, AOC expects source/docs/assets tracked and generated outputs ignored:

```gitignore
!/hyperframes/
!/hyperframes/**
/hyperframes/renders/**
/hyperframes/.hyperframes/**
/hyperframes/.cache/**
/hyperframes/node_modules/**
```

Check with:

```bash
aoc-hyperframes check --dir hyperframes
```
