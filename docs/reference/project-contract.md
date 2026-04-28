# AOC project contract

This page summarizes the project-local files AOC expects after `aoc-init`.

## Core paths

```text
.aoc/context.md              # generated project snapshot
.aoc/memory.md               # durable project memory, managed by aoc-mem
.aoc/stm/current.md          # short-term handoff draft, managed by aoc-stm
.aoc/rtk.toml                # RTK routing policy
.aoc/mind-service.json       # project Mind launcher metadata
.taskmaster/                 # Taskmaster tasks, tags, PRDs
.pi/settings.json            # project Pi settings
.pi/prompts/                 # project Pi prompts
.pi/skills/                  # project Pi skills
.pi/extensions/              # project Pi extensions
DESIGN.md                    # root product/design contract
AGENTS.md                    # agent rules for the repo
```

## Git policy

AOC project state should be tracked:

```gitignore
!/.aoc/
!/.aoc/**
!/.taskmaster/
!/.taskmaster/**
!/.pi/
!/.pi/**
```

Do not commit secrets.

## Commands

```bash
aoc-init                  # initialize/repair project contract
aoc-init --status         # summarize project readiness
aoc-handshake --json      # agent startup metadata
aoc-skill validate --root .
aoc-doctor
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
