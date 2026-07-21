# New machine setup

This runbook covers Linux and macOS.

## Install

Run the bootstrap installer:

```bash
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash
```

The installer selects a Linux or macOS release. If no release binary is available, it downloads the source archive and runs `install.sh`.

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

## Restart the shell

The installer adds a marked `~/.local/bin` PATH block for fish, zsh, and bash. Restart the shell or open a new terminal before running installed commands.

To skip profile edits:

```bash
AOC_SKIP_SHELL_PROFILE=1 ./install.sh
```

Then add `~/.local/bin` to PATH yourself.

## Manual steps

1. Copy required `.env` files and secrets from an existing machine. The repository and installer do not carry secrets.
2. If the doctor reports `omp` or `herdr` missing, install each CLI with its supported installer. Then connect Herdr to OMP:

   ```bash
   herdr integration install omp
   ```

3. Complete the logins used by your work, such as GitHub, Claude, Codex, model providers, and deployment services. For GitHub CLI:

   ```bash
   gh auth login
   ```

4. Initialize each project separately:

   ```bash
   cd ~/your-project
   aoc-init
   ```

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

The macOS path uses Homebrew for Yazi and Micro. Install Homebrew first if the doctor or installer reports that it is missing.
