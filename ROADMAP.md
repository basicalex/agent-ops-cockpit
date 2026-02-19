# AOC Roadmap

This document outlines the future direction of Agent Ops Cockpit (AOC), with a focus on cross-platform support and community involvement.

## Vision: Cross-Platform Agent Development

AOC aims to be the universal terminal workspace for AI-assisted development, regardless of operating system or shell preference. We believe developers should be able to use their preferred tools without friction.

### Current Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Linux | Fully Supported | Primary development platform |
| macOS | Fully Supported | All features functional |
| Windows (WSL) | Supported | Full functionality via WSL2 |
| Windows (Native) | Not Yet | Blocked by Zellij availability |

### Current Shell Support

| Shell | Terminal Pane | Internal Scripts |
|-------|--------------|------------------|
| bash | Default | Required |
| zsh | Via `$SHELL` | N/A |
| fish | Via `$SHELL` | N/A |
| PowerShell | Planned | N/A |
| Nushell | Planned | N/A |

## Multiplexer Dependency

AOC is built on [Zellij](https://zellij.dev/), a modern terminal multiplexer written in Rust. This architectural choice provides:

- Native plugin system (WASM-based)
- Rich layout management (KDL configuration)
- Session persistence and management
- Modern UX with floating panes and tabs

### Platform Limitations

**Zellij does not currently support native Windows.** This is the primary blocker for full cross-platform AOC deployment.

The Zellij project tracks Windows support in:
- [zellij-org/zellij#463](https://github.com/zellij-org/zellij/issues/463) - Windows support discussion
- [zellij-org/zellij#1663](https://github.com/zellij-org/zellij/issues/1663) - Cross-platform roadmap

**We encourage interested contributors to participate in the Zellij project directly** to help accelerate Windows support.

## Roadmap Items

### Phase 1: Multi-Shell Terminal Support (In Progress)

Enable users to run their preferred shell in terminal panes while AOC's internal tooling remains bash-based.

**Goal:** Fish, Zsh, PowerShell, and Nushell users can work natively in AOC terminal panes.

**Approach:**
- Use Zellij's native `cwd` directive instead of `bash -lc cd ...`
- Respect `$SHELL` environment variable
- Add `AOC_SHELL` override for explicit selection
- Create shell detection utilities for edge cases

**Tracking:** Task #41 in `.taskmaster/tasks/tasks.json`

### Phase 2: Custom Layout Selection (In Progress)

Empower users to create, share, and use custom Zellij layouts for different "AOC Modes" (e.g., Coding, Writing, Reviewing).

**Goal:** Users can select different layouts via CLI, persist their preference, and have AOC automatically inject context (project root, tab names) into any compatible layout.

**Approach:**
- Create `aoc-layout` tool for selection and persistence
- Update `aoc-new-tab` to support generic layout variable substitution
- Standardize layout placeholders (`__AOC_TAB_NAME__`, etc.)
- Provide Bash shortcut integration (`aoc.<layout>` + `aoc.` completion) for fast tab/session launch

**Next extension:**
- Port `aoc.<layout>` shell integration to Zsh and Fish with parity behavior (project-local refresh + completion)

**Tracking:** Task #42 in `.taskmaster/tasks/tasks.json`

### Phase 3: Script Portability (Planned)

Migrate critical user-facing scripts to POSIX sh or Rust for broader compatibility.

**Candidates for migration:**
- `aoc-init` - Project initialization
- `aoc-mem` - Memory management
- `aoc-doctor` - Dependency validation

**Non-candidates (remain bash):**
- Layout generation scripts (Zellij-specific)
- Agent wrappers (tmux integration)
- Internal utilities

### Phase 4: Alternative Multiplexer Support (Future)

Investigate support for alternative multiplexers to enable native Windows:

| Multiplexer | Platform | Plugin System | Feasibility |
|-------------|----------|---------------|-------------|
| tmux | Unix | Limited | Medium - would need major rework |
| Wezterm | Cross-platform | Lua | High - native Windows, extensible |
| Windows Terminal | Windows | Limited | Low - no plugin model |

**Note:** This is exploratory. Zellij remains the primary target.

### Phase 5: Native Windows Support (Future)

Contingent on either:
1. Zellij adding Windows support, OR
2. AOC supporting an alternative multiplexer (Phase 4)

## Contributing

We welcome community contributions toward cross-platform support!

### How to Help

1. **Multi-Shell Testing**
   - Test AOC with your preferred shell
   - Report issues with shell-specific behavior
   - Contribute shell detection improvements

2. **Zellij Project**
   - The Zellij team welcomes contributors
   - Windows support benefits the entire ecosystem
   - See: https://github.com/zellij-org/zellij/blob/main/CONTRIBUTING.md

3. **Script Portability**
   - Identify bash-specific constructs that could be POSIX
   - Propose Rust rewrites for performance-critical paths
   - Test on different Unix variants (BSD, Alpine, etc.)

4. **Documentation**
   - Improve setup guides for different platforms
   - Document shell-specific configuration
   - Translate documentation

### Contribution Guidelines

- Open an issue before starting major work
- Follow existing code style (run `./scripts/lint.sh`)
- Update documentation with changes
- Add tests where applicable

## Timeline

| Phase | Target | Status |
|-------|--------|--------|
| Multi-Shell Support | Q1 2026 | In Progress |
| Custom Layouts | Q1 2026 | In Progress |
| Script Portability | Q2 2026 | Planned |
| Alternative Multiplexers | TBD | Research |
| Native Windows | TBD | Blocked |

## Feedback

We'd love to hear from the community:

- What shells do you use that aren't well-supported?
- Would you contribute to Zellij Windows support?
- Are there other multiplexers we should consider?

Open a discussion or issue at: https://github.com/basicalex/agent-ops-cockpit/discussions
