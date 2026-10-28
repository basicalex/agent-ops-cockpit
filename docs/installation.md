# Installation

## Recommended install
For a fresh Linux or macOS system, follow the [new-machine runbook](new-machine.md).


```bash
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash
```

This installs the latest tagged release. To install current `main` instead (what these docs describe), add `-s -- --ref main`:

```bash
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --ref main
```

Then:

```bash
aoc-doctor
```

## Local clone install

```bash
git clone https://github.com/basicalex/agent-ops-cockpit.git
cd agent-ops-cockpit
./install.sh
aoc-doctor
```

## Initialize a project

Run this once per repo:

```bash
cd ~/your-project
aoc-init
```

Then launch:

```bash
aoc
```

## What install does

`install.sh` installs AOC binaries, scripts, the Herdr config baseline, OMP defaults, optional skill templates, OMP extension/skill/agent assets declared in `.omp/manifest.toml`, the AOC-aware OMP shim, and global config under user-local paths.

It does **not** assume every repo should become an AOC repo. Use `aoc-init` for each project you want to use with AOC.

## Useful install options

```bash
# Pin release
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --ref v0.2.0

# Non-interactive
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --yes

# Skip doctor after install
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --skip-doctor

# Install from fork
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --repo your-org/agent-ops-cockpit
```

Local install overrides:

```bash
AOC_INSTALL_RUST=0 ./install.sh           # skip Rust bootstrap
AOC_SKIP_SHELL_PROFILE=1 ./install.sh     # leave shell profiles unchanged
```

The profile step writes two marked blocks to zsh, bash, and fish: `~/.local/bin` on PATH, and a `y` function that opens yazi and leaves the shell in the directory yazi was closed in.

## Requirements

Bootstrap prerequisites:

- Git
- Bash
- Python 3.11+ (`python3` with `tomllib`)

Herdr and the OMP coding agent CLI (`omp`) are required for Herdr/OMP orchestration after bootstrap. Install them with their supported installers, then run:

```bash
herdr integration install omp
```

Recommended:

- Rust/Cargo for local builds
- Node.js `>= 22` for OMP extensions
- Bun for `aoc-live` and fframes

## Verify

Run the health check, project status check, and OMP startup checks:

```bash
aoc-doctor
aoc-init --status
aoc-handshake --json
aoc-omp-context
omp --help
```

## Detailed reference

The old exhaustive install contract is preserved at [reference/installation-details.md](reference/installation-details.md).

Project-local seeded paths are summarized in [reference/project-contract.md](reference/project-contract.md).
