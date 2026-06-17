# Repository Guidelines

Scope: `crates/aoc-installer/src`

## Local Contracts
- Treat this as live installer code: routine verification must not run installs, downloaded `install.sh`, `--yes`, post-install doctor, or mutate real `~/.local/bin`/`~/.config`; prefer parser/resolver checks, compile checks, and no-run tests.
- Keep installs explicit, interactive by default, and user-local in messaging. Keep downloads constrained to GitHub archive/API flows and slug-shaped repos; if touching explicit `--repo`, close the current validation seam instead of widening accepted inputs.
- Tests or refactors around command spawning/downloading must isolate temp PATH/filesystem fixtures and target pure helpers before integration paths; never test by executing downloaded code or depending on host `curl`/`wget`/`tar`/`bash`/`git`/`aoc-doctor` behavior.

## Verification
- `cargo check -p aoc-installer`
- `cargo test -p aoc-installer --no-run`
