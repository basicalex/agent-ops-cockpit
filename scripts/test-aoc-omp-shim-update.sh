#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)"

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/aoc-omp-shim-update.XXXXXX")"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

home_dir="$tmp_dir/home"
bin_dir="$tmp_dir/bin"
fake_dir="$tmp_dir/fakes"
project_dir="$tmp_dir/project"
node_modules="$tmp_dir/bun-global/node_modules"
log_dir="$tmp_dir/logs"
mkdir -p "$home_dir" "$bin_dir" "$fake_dir" "$project_dir/.aoc" "$log_dir"
printf 'context\n' > "$project_dir/.aoc/context.md"

raw_source="$tmp_dir/raw-source"
raw_log="$log_dir/raw.log"
aoc_wrapper_log="$log_dir/aoc-wrapper.log"
shim_update_log="$log_dir/shim-update.log"
updater_dispatch_log="$log_dir/updater-dispatch.log"
bun_log="$log_dir/bun.log"
installer_log="$log_dir/installer.log"

cat > "$raw_source" <<'RAW'
#!/usr/bin/env bash
set -euo pipefail
printf 'raw:%s\n' "$*" >> "${RAW_LOG:?}"
RAW
chmod +x "$raw_source"

cat > "$fake_dir/aoc-omp" <<'AOC'
#!/usr/bin/env bash
set -euo pipefail
printf 'aoc:%s\n' "$*" >> "${AOC_WRAPPER_LOG:?}"
AOC
chmod +x "$fake_dir/aoc-omp"

cat > "$fake_dir/aoc-omp-update" <<'UPDATER'
#!/usr/bin/env bash
set -euo pipefail
printf 'updater:%s\n' "$*" >> "${UPDATER_DISPATCH_LOG:?}"
UPDATER
chmod +x "$fake_dir/aoc-omp-update"

env \
  HOME="$home_dir" \
  PATH="$fake_dir:$PATH" \
  AOC_BIN_DIR="$bin_dir" \
  AOC_RAW_OMP_BIN="$raw_source" \
  "$repo_root/bin/aoc-omp-shim-install" > "$shim_update_log"

if [[ ! -x "$bin_dir/omp" || ! -x "$bin_dir/omp-raw" ]]; then
  echo "shim installer did not create executable omp and omp-raw" >&2
  exit 1
fi

# Non-update commands keep the existing shim behavior: outside an AOC project they
# go to omp-raw; inside one they go through aoc-omp.
(
  cd "$tmp_dir"
  env \
    HOME="$home_dir" \
    AOC_PROJECT_ROOT= \
    PATH="$fake_dir:$bin_dir:$PATH" \
    RAW_LOG="$raw_log" \
    AOC_WRAPPER_LOG="$aoc_wrapper_log" \
    "$bin_dir/omp" say hello
)

(
  cd "$project_dir"
  env \
    HOME="$home_dir" \
    PATH="$fake_dir:$bin_dir:$PATH" \
    RAW_LOG="$raw_log" \
    AOC_WRAPPER_LOG="$aoc_wrapper_log" \
    "$bin_dir/omp" prompt text
)

if ! grep -Fxq 'raw:say hello' "$raw_log"; then
  echo "normal command outside AOC project did not route to omp-raw" >&2
  exit 1
fi
if ! grep -Fxq 'aoc:prompt text' "$aoc_wrapper_log"; then
  echo "normal command inside AOC project did not route to aoc-omp" >&2
  exit 1
fi

# update must dispatch to the updater before AOC project routing; the old wrapper
# incorrectly sent this path through aoc-omp/raw routing.
(
  cd "$project_dir"
  env \
    HOME="$home_dir" \
    PATH="$fake_dir:$bin_dir:$PATH" \
    RAW_LOG="$raw_log" \
    AOC_WRAPPER_LOG="$aoc_wrapper_log" \
    UPDATER_DISPATCH_LOG="$updater_dispatch_log" \
    AOC_OMP_UPDATER="$fake_dir/aoc-omp-update" \
    "$bin_dir/omp" update --check
)

if ! grep -Fxq 'updater:update --check' "$updater_dispatch_log"; then
  echo "generated shim did not dispatch update to aoc-omp-update" >&2
  exit 1
fi
if grep -Fxq 'aoc:update --check' "$aoc_wrapper_log" || grep -Fxq 'raw:update --check' "$raw_log"; then
  echo "generated shim routed update through normal AOC/raw path" >&2
  exit 1
fi

# Read-only updater modes forward to raw OMP and must not invoke Bun.
: > "$raw_log"
: > "$bun_log"
env \
  HOME="$home_dir" \
  PATH="$fake_dir:$PATH" \
  RAW_LOG="$raw_log" \
  AOC_RAW_OMP_BIN="$raw_source" \
  AOC_BUN_BIN="$fake_dir/bun" \
  "$repo_root/bin/aoc-omp-update" update --check

if ! grep -Fxq 'raw:update --check' "$raw_log"; then
  echo "update --check was not forwarded to raw OMP" >&2
  exit 1
fi
if [[ -s "$bun_log" ]]; then
  echo "update --check invoked Bun" >&2
  exit 1
fi

mkdir -p \
  "$node_modules/@oh-my-pi/pi-coding-agent/dist" \
  "$node_modules/@oh-my-pi/pi-natives" \
  "$node_modules/@oh-my-pi/pi-natives-linux-x64"
printf 'old cli\n' > "$node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js"

cat > "$fake_dir/bun" <<'BUN'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "${BUN_LOG:?}"
cat > "${AOC_BUN_GLOBAL_NODE_MODULES:?}/@oh-my-pi/pi-coding-agent/dist/cli.js" <<'CLI'
#!/usr/bin/env bash
printf 'updated cli\n'
CLI
chmod +x "${AOC_BUN_GLOBAL_NODE_MODULES:?}/@oh-my-pi/pi-coding-agent/dist/cli.js"
BUN
chmod +x "$fake_dir/bun"

cat > "$fake_dir/shim-installer" <<'INSTALLER'
#!/usr/bin/env bash
set -euo pipefail
printf 'installer raw=%s\n' "${AOC_RAW_OMP_BIN:-}" >> "${INSTALLER_LOG:?}"
INSTALLER
chmod +x "$fake_dir/shim-installer"

: > "$bun_log"
: > "$installer_log"
env \
  HOME="$home_dir" \
  PATH="$fake_dir:$PATH" \
  BUN_LOG="$bun_log" \
  INSTALLER_LOG="$installer_log" \
  AOC_BUN_BIN="$fake_dir/bun" \
  AOC_BUN_GLOBAL_NODE_MODULES="$node_modules" \
  AOC_OMP_SHIM_INSTALLER="$fake_dir/shim-installer" \
  "$repo_root/bin/aoc-omp-update" update

expected_bun='add -g --no-cache --registry https://registry.npmjs.org @oh-my-pi/pi-coding-agent@latest @oh-my-pi/pi-natives@latest @oh-my-pi/pi-natives-linux-x64@latest'
if ! grep -Fxq "$expected_bun" "$bun_log"; then
  echo "Bun update args did not match expected package contract" >&2
  cat "$bun_log" >&2
  exit 1
fi

updated_cli="$node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js"
if ! grep -Fxq "installer raw=$updated_cli" "$installer_log"; then
  echo "shim installer was not called with updated CLI as AOC_RAW_OMP_BIN" >&2
  cat "$installer_log" >&2
  exit 1
fi
if ! grep -Fxq 'printf '\''updated cli\n'\''' "$updated_cli"; then
  echo "fake Bun did not refresh the package CLI fixture" >&2
  exit 1
fi

printf 'test-aoc-omp-shim-update: ok\n'
