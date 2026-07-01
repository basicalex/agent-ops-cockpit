#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

assert_file() {
  local path="$1"
  [[ -f "$path" ]] || { echo "ERROR: missing file: $path" >&2; exit 1; }
}

assert_dir() {
  local path="$1"
  [[ -d "$path" ]] || { echo "ERROR: missing directory: $path" >&2; exit 1; }
}

assert_absent() {
  local path="$1"
  [[ ! -e "$path" ]] || { echo "ERROR: unexpected asset present: $path" >&2; exit 1; }
}

assert_config_extensions_match() {
  local config="$1"
  local extension_dir="$2"
  shift 2
  python3 - "$config" "$extension_dir" "$@" <<'PY'
from pathlib import Path
import sys
config = Path(sys.argv[1])
extension_dir = Path(sys.argv[2])
expected = [str(extension_dir / name) for name in sys.argv[3:]]
lines = config.read_text(encoding='utf-8').splitlines()
try:
    start = next(i for i, line in enumerate(lines) if line.startswith('extensions:'))
except StopIteration:
    raise SystemExit('ERROR: config missing extensions block')
end = start + 1
while end < len(lines) and (lines[end].startswith('  - ') or not lines[end].strip()):
    end += 1
actual = [line[4:] for line in lines[start + 1:end] if line.startswith('  - ')]
if actual != expected:
    raise SystemExit(f'ERROR: config extensions mismatch\nexpected={expected!r}\nactual={actual!r}')
PY
}

run_init_fixture() {
  local name="$1"
  local profiles="${2:-}"
  local project="$tmp/$name-project"
  local runtime="$tmp/$name-omp-agent"
  local config="$runtime/config.yml"
  mkdir -p "$project" "$runtime"
  printf 'disabledExtensions:\n  - extension-module:third-party\nextensions:\n  - stale-extension.ts\n' >"$config"
  mkdir -p "$runtime/extensions" "$runtime/skills/aoc-hyperframes" "$runtime/agents"
  printf 'stale extension\n' >"$runtime/extensions/aoc-brand-content.ts"
  printf 'stale removed AOC extension\n' >"$runtime/extensions/aoc-jj-init.ts"
  printf 'stale skill\n' >"$runtime/skills/aoc-hyperframes/SKILL.md"
  printf 'stale AOC agent\n' >"$runtime/agents/brand-strategy.md"
  printf 'user agent\n' >"$runtime/agents/user-local.md"
  if [[ -n "$profiles" ]]; then
    AOC_INIT_SKIP_BUILD=1 AOC_OMP_PROFILES="$profiles" AOC_OMP_AGENT_DIR="$runtime" AOC_OMP_AGENT_CONFIG="$config" bash "$root/bin/aoc-init" "$project" >&2
  else
    AOC_INIT_SKIP_BUILD=1 AOC_OMP_AGENT_DIR="$runtime" AOC_OMP_AGENT_CONFIG="$config" bash "$root/bin/aoc-init" "$project" >&2
  fi
  printf '%s\n%s\n%s\n' "$project" "$runtime" "$config"
}

mapfile -t default_fixture < <(run_init_fixture default "")
default_project="${default_fixture[0]}"
default_runtime="${default_fixture[1]}"
default_config="${default_fixture[2]}"

for required in \
  .aoc/context.md \
  .omp/manifest.toml \
  .omp/extensions \
  .omp/agents \
  .omp/skills; do
  if [[ ! -e "$default_project/$required" ]]; then
    echo "ERROR: missing initialized project asset: $required" >&2
    exit 1
  fi
done

if [[ -e "$default_project/.pi" ]]; then
  echo "ERROR: aoc-init created legacy .pi directory" >&2
  exit 1
fi

core_extensions=(
  aoc-profile.ts
  aoc-codegraph.ts
  aoc-dox.ts
  aoc-style.ts
)
for required in "${core_extensions[@]}"; do
  assert_file "$default_runtime/extensions/$required"
done

for forbidden in \
  aoc-mind.ts \
  aoc-herdr.ts \
  aoc-master.ts \
  aoc-commit.ts \
  aoc-state.ts \
  aoc-brand-content.ts \
  aoc-web-search.ts \
  aoc-dox-command.ts; do
  assert_absent "$default_runtime/extensions/$forbidden"
done
assert_absent "$default_runtime/extensions/aoc-jj-init.ts"

for forbidden in \
  brand-strategy.md \
  brand-concept.md \
  svg-asset.md \
  hyperframes-content.md \
  dox-scout.md \
  dox-mapper.md \
  dox-critic.md \
  dox-writer.md; do
  assert_absent "$default_runtime/agents/$forbidden"
done
assert_file "$default_runtime/agents/user-local.md"
python3 - "$default_config" <<'PY'
from pathlib import Path
import sys
config = Path(sys.argv[1]).read_text(encoding='utf-8')
for name in (
    'brand-strategy',
    'brand-concept',
    'svg-asset',
    'hyperframes-content',
    'dox-scout',
    'dox-mapper',
    'dox-critic',
    'dox-writer',
):
    needle = f'    - {name}'
    if needle not in config:
        raise SystemExit(f'ERROR: inactive AOC agent not disabled in config: {name}')
if '    - user-local' in config:
    raise SystemExit('ERROR: non-AOC user-local agent was disabled')
PY


for required in \
  aoc-understand \
  aoc-init-ops \
  aoc-update \
  frontend-design \
  motion-director \
  animejs-core-api \
  funnel-design \
  safe-gamification \
  omarchy-theme-ops \
  ponytail-workflows; do
  assert_file "$default_runtime/skills/$required/SKILL.md"
done

for forbidden in \
  aoc-hyperframes \
  hyperframes \
  hyperframes-cli \
  website-to-hyperframes \
  gsap \
  aoc-dox-cartography \
  ponytail-review \
  ponytail-audit \
  ponytail-debt \
  ponytail-help; do
  assert_absent "$default_runtime/skills/$forbidden"
done

assert_config_extensions_match "$default_config" "$default_runtime/extensions" "${core_extensions[@]}"
for disabled_extension in \
  extension-module:aoc-mind \
  extension-module:aoc-herdr \
  extension-module:aoc-master \
  extension-module:aoc-commit \
  extension-module:aoc-state \
  extension-module:aoc-brand-content \
  extension-module:aoc-web-search \
  extension-module:aoc-dox-command; do
  if ! grep -Fq "  - $disabled_extension" "$default_config"; then
    echo "ERROR: inactive AOC extension not disabled in config: $disabled_extension" >&2
    exit 1
  fi
done
if ! grep -Fq '  - extension-module:third-party' "$default_config"; then
  echo "ERROR: user disabled extension was not preserved" >&2
  exit 1
fi
assert_absent "$default_runtime/extensions/ponytail.ts"
assert_absent "$default_runtime/skills/ponytail-review"
assert_absent "$default_runtime/skills/ponytail-audit"
assert_absent "$default_runtime/skills/ponytail-debt"
assert_absent "$default_runtime/skills/ponytail-help"

mapfile -t full_fixture < <(run_init_fixture full full)
full_runtime="${full_fixture[1]}"
full_config="${full_fixture[2]}"
mapfile -t full_extensions < <(AOC_OMP_PROFILES=full bash "$root/bin/aoc-profile" active --kind extensions --root "$root" --manifest "$root/.omp/manifest.toml")
mapfile -t full_skills < <(AOC_OMP_PROFILES=full bash "$root/bin/aoc-profile" active --kind skills --root "$root" --manifest "$root/.omp/manifest.toml")
mapfile -t full_agents < <(AOC_OMP_PROFILES=full bash "$root/bin/aoc-profile" active --kind agents --root "$root" --manifest "$root/.omp/manifest.toml")

for required in "${full_extensions[@]}"; do
  assert_file "$full_runtime/extensions/$required"
done
for required in "${full_skills[@]}"; do
  assert_file "$full_runtime/skills/$required/SKILL.md"
done
for required in "${full_agents[@]}"; do
  assert_file "$full_runtime/agents/$required"
done
assert_config_extensions_match "$full_config" "$full_runtime/extensions" "${full_extensions[@]}"
assert_file "$full_runtime/extensions/aoc-mind.ts"
if grep -Fq 'extension-module:aoc-mind' "$full_config"; then
  echo "ERROR: full profile should not disable aoc-mind extension" >&2
  exit 1
fi
if ! grep -Fq '  disabledAgents: []' "$full_config"; then
  echo "ERROR: full profile should clear AOC disabled-agent list" >&2
  exit 1
fi
if grep -Fq 'extension-module:aoc-master' "$full_config"; then
  echo "ERROR: full profile should not disable aoc-master extension" >&2
  exit 1
fi
if ! grep -Fq '  - extension-module:third-party' "$full_config"; then
  echo "ERROR: full profile did not preserve user disabled extension" >&2
  exit 1
fi
assert_absent "$full_runtime/extensions/ponytail.ts"
assert_absent "$full_runtime/skills/ponytail-review"
assert_absent "$full_runtime/skills/ponytail-audit"
assert_absent "$full_runtime/skills/ponytail-debt"
assert_absent "$full_runtime/skills/ponytail-help"
assert_dir "$full_runtime/skills/ponytail-workflows"

doctor_log="$tmp/doctor.log"
(cd "$default_project" && bash "$root/bin/aoc-doctor") >"$doctor_log"
for forbidden in '.pi/settings.json' 'PI runtime' 'pi-multi-auth'; do
  if grep -Fq "$forbidden" "$doctor_log"; then
    echo "ERROR: aoc-doctor mentioned retired surface: $forbidden" >&2
    exit 1
  fi
done

echo "PASS: AOC OMP canonical init profiles"
