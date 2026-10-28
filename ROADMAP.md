# AOC Roadmap

AOC is a Herdr-first project and tooling layer for AI-assisted development. Herdr owns workspaces, tabs and panes; OMP owns agent orchestration; AOC adds project setup, launch commands and repo-owned agent assets.

## Platform support

| Platform | Status | Notes |
|----------|--------|-------|
| Linux | Supported | Primary development platform |
| macOS | Supported | All features work |
| Windows (WSL) | Supported | Through WSL2 |
| Windows (native) | Not yet | AOC scripts are bash; Herdr and OMP set the limit |

## Shell support

| Shell | Terminal pane | Internal scripts |
|-------|---------------|------------------|
| bash | Default | Required |
| zsh | Via `$SHELL` | N/A |
| fish | Via `$SHELL` | N/A |

`install.sh` writes the PATH and `y` (yazi) blocks for bash, zsh and fish.

## Next

- Keep the default install lean: one installer run, no retired tools, nothing that only one machine needs.
- Port user-facing scripts such as `aoc-init` and `aoc-doctor` to Rust where bash portability hurts.
- Native Windows only if Herdr and OMP support it.

## Contributing

- Open an issue before starting major work.
- Follow the existing code style and run `./scripts/lint.sh`.
- Update docs with each change.
- Add tests where they fit.

Open a discussion or issue at: https://github.com/basicalex/agent-ops-cockpit/discussions
