# Agent Extensibility (OMP-first)

AOC's active coding-agent runtime is OMP. Extension points are OMP-native and repo-owned under `.omp/**`.

Current contract:

- `aoc-init` seeds project OMP assets under `.omp/extensions`, `.omp/agents`, and `.omp/skills`.
- `aoc-herdr-install` installs those assets into `${AOC_OMP_AGENT_DIR:-~/.omp/agent}`.
- Legacy Pi wrapper/settings/package surfaces are retired, not compatibility inputs.
- Use `aoc omp` or the OMP shim installed by `aoc-omp-shim-install` for runtime launch.
