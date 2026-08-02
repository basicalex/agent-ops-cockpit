# Move AOC to a new machine

`aoc-migrate` creates an encrypted state bundle for credentials, agent state, and user configuration. It prints a separate `rsync` recipe for `~/dev`; project files are not placed in the bundle.

## On the old machine

From the AOC repository, inspect the curated paths:

```bash
bin/aoc-migrate paths
```

Create the bundle:

```bash
bin/aoc-migrate pack
```

The command prefers `age -p` and falls back to OpenSSL. It prompts for a passphrase and deletes the plaintext archive after encryption. Keep the passphrase separate from the bundle.

The pack command also reports dirty Git worktrees and commits that are not on a remote. It does not commit, push, or run the printed `rsync` command. Resolve that report before retiring the old machine.

Copy the encrypted bundle to the new machine through a channel you trust. Keep the old machine until the restore and project checks pass.

## Prepare the new Mac

Install Homebrew if needed, then install Git and age:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
brew install git age
```

Clone AOC:

```bash
git clone https://github.com/basicalex/agent-ops-cockpit.git
cd agent-ops-cockpit
```

## Restore state

Run the migration tool directly from the clone, before installing AOC:

```bash
bin/aoc-migrate restore /path/to/aoc-state-YYYYMMDD.tar.gz.age
```

Enter the bundle passphrase when prompted. The restore reads the old home path from `manifest.txt`, moves existing destination paths to `~/aoc-migrate-backup-<timestamp>/`, restores the captured files, fixes SSH and GnuPG permissions, and renames Claude project-state slugs for the new home path. Review every file listed by the old-home warning scan; the command does not rewrite file contents.

Install AOC:

```bash
./install.sh
```

Open a new shell if the installer changed `PATH`, then check the installation:

```bash
aoc-doctor
```

## Transfer projects

Run the `rsync` recipe printed by `aoc-migrate pack` on the old machine after replacing `<new-machine>` with the new host. It excludes dependency and build output directories. The recipe transfers `~/dev` separately from the encrypted state bundle.

In each transferred project that needs JavaScript or TypeScript dependencies, run:

```bash
bun install
```

Run the project-specific bootstrap or doctor commands required by that repository.

## Re-authenticate device-bound tools

Migrated credentials may remain valid, but device-bound sessions can reject them. Check and re-authenticate:

- Claude Code login
- Codex OAuth, if that installation uses it
- Vercel
- Wrangler and Cloudflare
- any other service that rejects the migrated credential

Do not delete the backup directory or old machine state until these checks pass.

## macOS service gap

The migrated `qa-browser-reaper.service` and `qa-browser-reaper.timer` are systemd user units. macOS does not run them. Port the reaper to a launchd agent before relying on scheduled browser cleanup on the Mac.

On Linux, the restored systemd units still need the normal user-service reload and enablement for that machine.
