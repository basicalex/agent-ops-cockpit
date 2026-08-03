# New machine setup

This runbook covers Linux and macOS.

## Install

On macOS, install Homebrew first if it is not already installed. The bootstrap uses Homebrew for Yazi, Micro, and Python:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
if [ -x /opt/homebrew/bin/brew ]; then
  eval "$(/opt/homebrew/bin/brew shellenv)"
elif [ -x /usr/local/bin/brew ]; then
  eval "$(/usr/local/bin/brew shellenv)"
fi
brew install python
```

`brew install python` provides `python3` with `tomllib` before AOC bootstrap.

Then run the AOC bootstrap installer:

```bash
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash
```

The installer selects a Linux or macOS release. If no release binary is available, it downloads the source archive and runs `install.sh`. AOC bootstrap may run before Herdr and OMP are installed; missing `herdr` or `omp` should leave integration work for the manual steps below, not break the bootstrap.

Use `--yes` for a non-interactive install:

```bash
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --yes
```

## Installed paths

The install places:

- commands in `~/.local/bin`
- AOC config in `~/.config/aoc`
- Claude policy and skills in `~/.claude`
- global agent rules in `~/AGENTS.md`
- a default Codex config in `~/.codex/config.toml` when none exists
- Yazi config in `~/.config/yazi`
- Micro config in `~/.config/micro`

Existing Claude and global agent policy files receive timestamped `.bak` copies before replacement. An existing Codex config is never replaced.

### claude-codex (Claude Code on Codex OAuth)

`claude-codex` runs Claude Code through the local CLIProxyAPI bridge with Codex OAuth. `install.sh` installs the wrapper, proxy config, and Linux user services. On each machine, run the login once:

```bash
claude-codex-login
```

## Restart the shell

The installer adds a marked `~/.local/bin` PATH block for fish, zsh, and bash. Restart the shell or open a new terminal before running installed commands.

To skip profile edits:

```bash
AOC_SKIP_SHELL_PROFILE=1 ./install.sh
```

Then add `~/.local/bin` to PATH yourself.

## Manual steps

1. Copy required `.env` files and secrets from an existing machine. The repository and installer do not carry secrets.
2. Install the Herdr CLI with its supported installer.
3. Install the OMP coding agent CLI (`omp`) with its supported installer.
4. Connect Herdr to OMP:

   ```bash
   herdr integration install omp
   ```

5. Complete the logins used by your work, such as GitHub, Claude, Codex, model providers, and deployment services. For GitHub CLI:

   ```bash
   gh auth login
   ```

6. Initialize each project separately:

   ```bash
   cd ~/your-project
   aoc-init
   ```

## macOS keyboard note

The repo Herdr config uses `alt+...` bindings. On macOS, configure your terminal to send Option as Alt/Meta so `Option+H/J/K/L` keeps pane focus shortcuts and `Option+Z` keeps zoom. Common terminal settings call this "Use Option as Meta key", "Left Option key acts as Esc+", or "Option sends Meta".

## Verify

Run:

```bash
aoc-doctor
```

Warnings identify missing optional tools, an old global policy seed, or a shell that has not loaded the new PATH. Re-run `./install.sh` from the repository to refresh stale global policy files.

For a project, also run:

```bash
aoc-init --status
aoc-handshake --json
```

On macOS, if the doctor or installer still reports Homebrew missing, install Homebrew and re-run the failed step.
