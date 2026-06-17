# Subagent runtime

AOC no longer ships a project-local retired Pi subagent runtime. Legacy retired Pi subagent manifests, chains, teams, and detached job artifacts were removed from the active operator path.

Active contract:

- OMP/harness built-in subagent dispatch owns orchestration.
- Repo-owned OMP specialist templates live under `.omp/agents/`.
- `aoc-init` and `aoc-herdr-install` install those templates into `${AOC_OMP_AGENT_DIR:-~/.omp/agent}/agents`.
- Mind context remains lazy/focused evidence, not background Pi agent injection.

Do not use removed OMP paths as runtime evidence. Use the OMP agents documented in `docs/agents.md` and the harness `task`/subagent surface.
