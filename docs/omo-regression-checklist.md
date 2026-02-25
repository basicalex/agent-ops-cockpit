# OmO Regression Checklist

Use this checklist before cutting or deploying an OmO+AOC integration update.

## 1) Fast local regression

Run from repo root:

```bash
bash scripts/opencode/verify-omo.sh regression \
  --policy config/opencode/oh-my-opencode.policy.jsonc \
  --project-root "$PWD" \
  --profile sandbox \
  --max-chars 4096
```

Expected:
- Taskmaster-only task authority check passes.
- Control-first policy defaults pass.
- Profile isolation check passes.
- Context-pack order/bounds check passes.
- Shell syntax checks pass.

Optional stricter run:

```bash
bash scripts/opencode/verify-omo.sh regression --run-lint --rust-check
```

## 2) Full smoke

```bash
AOC_SMOKE_TEST=1 bash scripts/smoke.sh
```

Expected:
- Core shell smoke checks pass.
- OmO regression checks pass via `verify-omo.sh regression`.

## 3) Clean profile install/init rehearsal

Use isolated HOME/XDG roots:

```bash
tmp_root="$(mktemp -d)"
HOME="$tmp_root/home" \
XDG_CONFIG_HOME="$tmp_root/config" \
XDG_STATE_HOME="$tmp_root/state" \
AOC_INSTALL_OMO=1 \
AOC_OMO_PROFILE=sandbox \
bash ./install.sh

HOME="$tmp_root/home" \
XDG_CONFIG_HOME="$tmp_root/config" \
XDG_STATE_HOME="$tmp_root/state" \
bash ./bin/aoc-init "$PWD"
```

Expected:
- Sandbox profile exists and contains OmO plugin registration.
- Main profile is unchanged unless explicit promotion is performed.
- Project policy file `.opencode/oh-my-opencode.jsonc` is not auto-seeded by `aoc-init`.

## 4) Profile switch and rollback rehearsal

```bash
aoc-opencode-profile init sandbox
aoc-opencode-profile promote sandbox main --yes
aoc-opencode-profile list-backups main
aoc-opencode-profile rollback main --yes
```

Expected:
- Promotion creates a snapshot for `main`.
- Rollback restores prior `main` state from snapshot.
- No unmanaged path writes occur.

## 5) Governance conflict guard

Ensure no parallel task artifacts exist:

```bash
test ! -d .sisyphus/tasks
```

Expected:
- `.sisyphus/tasks` is absent.
- Task lifecycle remains on `tm` / `aoc-task` only.
