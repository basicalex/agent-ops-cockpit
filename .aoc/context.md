# Project Context Snapshot

## Repository
- Name: agent-ops-cockpit
- Root: /home/ceii/dev/agent-ops-cockpit
- Git branch: main

## Key Files
- README.md
- DESIGN.md
- package.json
- pnpm-lock.yaml

## Project Structure (tree -L 2)
```
.
./.agents
./AGENTS.md
./.agents/skills
./AOC.md
./bin
./bin/aoc
./bin/aoc-agent
./bin/aoc-agent-install
./bin/aoc-agent-run
./bin/aoc-agent-wrap
./bin/aoc-align
./bin/aoc-cleanup
./bin/aoc-cleanup-core.py
./bin/aoc-clock
./bin/aoc-clock-set
./bin/aoc-context
./bin/aoc-control
./bin/aoc-control-toggle
./bin/aoc-doctor
./bin/aoc-fetch
./bin/aoc-handshake
./bin/aoc-hf
./bin/aoc-hf-u
./bin/aoc-hub
./bin/aoc-hyperframes
./bin/aoc-init
./bin/aoc-insight
./bin/aoc-launch
./bin/aoc-layout
./bin/aoc-map
./bin/aoc-mem
./bin/aoc-mind-toggle
./bin/aoc-mission-control
./bin/aoc-mission-control-tab
./bin/aoc-mission-control-toggle
./bin/aoc-new-tab
./bin/aoc-obscura-install
./bin/aoc-od
./bin/aoc-open-explorer
... [tree truncated to 40 lines]
```

## README Headings
# AOC — terminal-first AI workspace
## Install
## First run
## Human docs
## Requirements
## Troubleshooting
## License

## Design Contract
- Root DESIGN.md: present
- Use as visual/product design source before product-facing UI, docs-site, marketing, or media changes.

## Current Task Tag
```
env-protec
```

## Active Workstreams (Tags)
```
184 (5)
aoc-presets (1)
aoc/pi_cleanup (9)
deprecation (10)
detached-orchestration (3)
env-protec (51)
master (46)
mermaid (1)
mind (51)
mission-control (17)
omo (10)
pi-compaction-ui (1)
pi-terminal-ops (1)
pulse-hub-spoke (8)
pulse-tab-overview (1)
rtk (5)
safety (9)
session-overseer (0)
sub-agents (6)
subagent-ux (6)
```

## Task spec Location
- Directory: .taskmaster/docs/specs
- Resolve tag spec default with: aoc-task tag spec show --tag <tag>
- Resolve task spec override with: aoc-task spec show <id> --tag <tag>
- Effective precedence: task spec override -> tag spec default
