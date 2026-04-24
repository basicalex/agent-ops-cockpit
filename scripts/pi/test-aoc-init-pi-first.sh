#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
aoc_init_bin="$repo_root/bin/aoc-init"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_exists() {
  local path="$1"
  [[ -e "$path" ]] || fail "Expected path to exist: $path"
}

assert_not_exists() {
  local path="$1"
  [[ ! -e "$path" ]] || fail "Expected path to be absent: $path"
}

assert_contains() {
  local needle="$1"
  local file="$2"
  grep -Fq "$needle" "$file" || fail "Expected '$needle' in $file"
}

assert_same_file() {
  local left="$1"
  local right="$2"
  cmp -s "$left" "$right" || fail "Expected files to match: $left == $right"
}

run_init() {
  local project_root="$1"
  local log_file="$2"
  AOC_INIT_SKIP_BUILD=1 bash "$aoc_init_bin" "$project_root" >"$log_file" 2>&1
}

run_installed_init() {
  local installed_bin="$1"
  local project_root="$2"
  local log_file="$3"
  AOC_INIT_SKIP_BUILD=1 bash "$installed_bin" "$project_root" >"$log_file" 2>&1
}

seed_cached_pi_runtime() {
  local cache_root="$XDG_CONFIG_HOME/aoc/pi"
  mkdir -p "$cache_root"
  cp -R "$repo_root/.pi/extensions" "$cache_root/extensions"
  cp -R "$repo_root/.pi/prompts" "$cache_root/prompts"
  cp -R "$repo_root/.pi/skills" "$cache_root/skills"
  mkdir -p "$cache_root/packages"
  cp -R "$repo_root/.pi/packages/pi-multi-auth-aoc" "$cache_root/packages/pi-multi-auth-aoc"
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-init-pi-first-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

export HOME="$tmp_root/home"
export XDG_CONFIG_HOME="$tmp_root/config"
mkdir -p "$HOME" "$XDG_CONFIG_HOME"

fake_bin="$tmp_root/bin"
mkdir -p "$fake_bin"
cat > "$fake_bin/pi" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

cmd="${1:-}"
if [[ "$cmd" != "install" ]]; then
  echo "unsupported fake pi command: $*" >&2
  exit 1
fi
shift

if [[ "${1:-}" == "-l" || "${1:-}" == "--local" ]]; then
  echo "unexpected local install in fake pi: $*" >&2
  exit 1
fi

source_spec="${1:-}"
[[ -n "$source_spec" ]] || {
  echo "missing source for fake pi install" >&2
  exit 1
}

log_file="${AOC_PI_TEST_INSTALL_LOG:-$HOME/pi-install.log}"
printf '%s\n' "$source_spec" >> "$log_file"

python3 - "$HOME/.pi/agent/settings.json" "$source_spec" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
source = sys.argv[2]
path.parent.mkdir(parents=True, exist_ok=True)

if path.exists():
    data = json.loads(path.read_text(encoding="utf-8"))
else:
    data = {}

packages = data.get("packages")
if not isinstance(packages, list):
    packages = []
if source not in packages:
    packages.append(source)
data["packages"] = packages
path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
PY
EOF
chmod +x "$fake_bin/pi"
export PATH="$fake_bin:$PATH"
export AOC_PI_TEST_INSTALL_LOG="$tmp_root/pi-install.log"

# --- Fresh repo flow ---
project_fresh="$tmp_root/fresh"
mkdir -p "$project_fresh/.git"

fresh_log_1="$tmp_root/fresh-init-1.log"
fresh_log_2="$tmp_root/fresh-init-2.log"
run_init "$project_fresh" "$fresh_log_1"

assert_exists "$project_fresh/.aoc/context.md"
assert_exists "$project_fresh/.aoc/memory.md"
assert_exists "$project_fresh/.aoc/stm/current.md"
assert_exists "$project_fresh/.aoc/init-state.json"
assert_contains '"projectAocVersion": 2' "$project_fresh/.aoc/init-state.json"
assert_exists "$project_fresh/.pi/settings.json"
assert_exists "$project_fresh/.pi/packages/pi-multi-auth-aoc/package.json"
assert_exists "$project_fresh/.pi/packages/pi-multi-auth-aoc/.aoc-managed"
assert_contains '"packages": [' "$project_fresh/.pi/settings.json"
assert_contains '"./packages/pi-multi-auth-aoc"' "$project_fresh/.pi/settings.json"
assert_contains '"defaultProvider": "openai-codex"' "$project_fresh/.pi/settings.json"
assert_contains '"defaultModel": "gpt-5.5"' "$project_fresh/.pi/settings.json"
assert_contains '"defaultThinkingLevel": "low"' "$project_fresh/.pi/settings.json"
assert_contains '"enabledModels": [' "$project_fresh/.pi/settings.json"
assert_contains '"openai-codex/gpt-5.5"' "$project_fresh/.pi/settings.json"
assert_contains '"openai-codex/gpt-5.4"' "$project_fresh/.pi/settings.json"
assert_contains '"opencode/glm-5"' "$project_fresh/.pi/settings.json"
assert_contains '"opencode/gemini-3-flash"' "$project_fresh/.pi/settings.json"
assert_contains '"opencode/gemini-3.1-pro"' "$project_fresh/.pi/settings.json"
assert_contains '"openrouter/anthropic/claude-sonnet-4"' "$project_fresh/.pi/settings.json"
assert_contains '"openrouter/openai/gpt-5.1-codex"' "$project_fresh/.pi/settings.json"
assert_contains '"openrouter/google/gemini-2.5-pro"' "$project_fresh/.pi/settings.json"
assert_contains '"openrouter/google/gemini-2.5-flash"' "$project_fresh/.pi/settings.json"
assert_contains '"openrouter/qwen/qwen3.6-plus"' "$project_fresh/.pi/settings.json"
assert_contains '"kimi-coding/kimi-for-coding"' "$project_fresh/.pi/settings.json"
assert_exists "$project_fresh/.pi/prompts/tm-cc.md"
assert_exists "$project_fresh/.pi/skills/aoc-init-ops/SKILL.md"
assert_exists "$project_fresh/.pi/extensions/minimal.ts"
assert_exists "$project_fresh/.pi/extensions/themeMap.ts"
assert_exists "$project_fresh/.pi/extensions/mind-ingest.ts"
assert_exists "$project_fresh/.pi/extensions/mind-ops.ts"
assert_exists "$project_fresh/.pi/extensions/mind-context.ts"
assert_exists "$project_fresh/.pi/extensions/mind-focus.ts"
assert_exists "$project_fresh/.pi/extensions/aoc-models.ts"
assert_exists "$project_fresh/.pi/extensions/lib/mind.ts"
assert_exists "$project_fresh/.pi/extensions/lib/caveman.ts"
assert_not_exists "$project_fresh/.pi/extensions/alibaba-model-studio.ts"
assert_exists "$HOME/.config/zellij/plugins/zjstatus-aoc.wasm"

assert_not_exists "$project_fresh/.aoc/skills"
assert_not_exists "$project_fresh/.codex/skills"
assert_not_exists "$project_fresh/.claude/skills"
assert_not_exists "$project_fresh/.opencode/skills"
assert_not_exists "$project_fresh/.agents/skills"

printf 'custom teach marker\n' > "$project_fresh/.pi/prompts/teach.md"
rm -f "$HOME/.config/zellij/plugins/zjstatus-aoc.wasm"
run_init "$project_fresh" "$fresh_log_2"
assert_contains "custom teach marker" "$project_fresh/.pi/prompts/teach.md"
assert_exists "$HOME/.config/zellij/plugins/zjstatus-aoc.wasm"

install_count="$(grep -c 'pi-multi-auth' "$AOC_PI_TEST_INSTALL_LOG" 2>/dev/null || true)"
[[ "$install_count" -eq 0 ]] || fail "Expected no global pi-multi-auth install attempts, got $install_count"

# --- Managed extension/preset/skill refresh flow (stale project copies upgraded) ---
project_refresh="$tmp_root/refresh"
mkdir -p "$project_refresh/.git" \
  "$project_refresh/.pi/extensions/aoc-presets" \
  "$project_refresh/.pi/extensions/lib" \
  "$project_refresh/.pi/skills/design-director" \
  "$project_refresh/.aoc/presets/design/components" \
  "$project_refresh/.aoc/layouts"
mkdir -p "$XDG_CONFIG_HOME/aoc/pi/extensions"

cat > "$XDG_CONFIG_HOME/aoc/pi/extensions/minimal.ts" <<'EOF'
// stale minimal template
export default {};
EOF
cat > "$project_refresh/.pi/extensions/minimal.ts" <<'EOF'
// stale minimal template
export default {};
EOF
cat > "$project_refresh/.pi/extensions/aoc-presets/commands.ts" <<'EOF'
// stale preset runtime
export default {};
EOF
cat > "$project_refresh/.pi/extensions/lib/caveman.ts" <<'EOF'
// stale caveman shared runtime
export const stale = true;
EOF
cat > "$project_refresh/.pi/skills/design-director/SKILL.md" <<'EOF'
---
name: design-director
description: stale design skill
---
EOF
cat > "$project_refresh/.aoc/presets/design/preset.toml" <<'EOF'
id = "design"
label = "Stale Design"
EOF
cat > "$project_refresh/.aoc/presets/design/components/mode-critique.md" <<'EOF'
stale preset component
EOF
cat > "$project_refresh/.aoc/layouts/design.kdl" <<'EOF'
layout {
  pane
}
EOF

refresh_log="$tmp_root/refresh-init.log"
run_init "$project_refresh" "$refresh_log"
assert_same_file "$repo_root/.pi/extensions/minimal.ts" "$project_refresh/.pi/extensions/minimal.ts"
assert_same_file "$repo_root/.pi/extensions/aoc-presets/commands.ts" "$project_refresh/.pi/extensions/aoc-presets/commands.ts"
assert_same_file "$repo_root/.pi/extensions/lib/caveman.ts" "$project_refresh/.pi/extensions/lib/caveman.ts"
assert_same_file "$repo_root/.pi/skills/design-director/SKILL.md" "$project_refresh/.pi/skills/design-director/SKILL.md"
assert_same_file "$repo_root/.aoc/presets/design/preset.toml" "$project_refresh/.aoc/presets/design/preset.toml"
assert_same_file "$repo_root/.aoc/presets/design/components/mode-critique.md" "$project_refresh/.aoc/presets/design/components/mode-critique.md"
assert_same_file "$repo_root/.aoc/layouts/design.kdl" "$project_refresh/.aoc/layouts/design.kdl"
assert_contains "Refreshed managed PI extension family: aoc-presets" "$refresh_log"
assert_contains "Refreshed managed AOC preset assets: design" "$refresh_log"
assert_contains "Refreshed managed AOC layout: design" "$refresh_log"

# --- Existing PI settings remain authoritative when already customized ---
project_settings="$tmp_root/settings-preserve"
mkdir -p "$project_settings/.git" "$project_settings/.pi"
cat > "$project_settings/.pi/settings.json" <<'EOF'
{
  "extensions": [],
  "defaultProvider": "openai",
  "defaultModel": "gpt-4o-mini"
}
EOF

settings_log="$tmp_root/settings-preserve-init.log"
run_init "$project_settings" "$settings_log"
assert_contains '"defaultProvider": "openai"' "$project_settings/.pi/settings.json"
assert_contains '"defaultModel": "gpt-4o-mini"' "$project_settings/.pi/settings.json"
assert_contains '"packages": [' "$project_settings/.pi/settings.json"
assert_contains '"./packages/pi-multi-auth-aoc"' "$project_settings/.pi/settings.json"
if grep -Fq '"enabledModels"' "$project_settings/.pi/settings.json"; then
  fail "Did not expect enabledModels to be injected into customized PI settings"
fi

# --- Legacy seeded PI defaults migrate to current defaults ---
project_legacy_enabled="$tmp_root/legacy-enabled"
mkdir -p "$project_legacy_enabled/.git" "$project_legacy_enabled/.pi"
cat > "$project_legacy_enabled/.pi/settings.json" <<'EOF'
{
  "extensions": [],
  "defaultProvider": "opencode",
  "defaultModel": "gpt-5.3-codex",
  "defaultThinkingLevel": "medium",
  "enabledModels": [
    "opencode/gpt-5.3-codex:medium",
    "opencode/claude-sonnet-4-6:medium",
    "opencode/gemini-3.1-pro:medium",
    "opencode/kimi-k2.5:medium"
  ]
}
EOF
legacy_enabled_log="$tmp_root/legacy-enabled-init.log"
run_init "$project_legacy_enabled" "$legacy_enabled_log"
assert_contains '"defaultProvider": "openai-codex"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"defaultModel": "gpt-5.5"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"defaultThinkingLevel": "low"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"openai-codex/gpt-5.5"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"openai-codex/gpt-5.4"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"opencode/glm-5"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"opencode/gemini-3-flash"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"opencode/gemini-3.1-pro"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"openrouter/anthropic/claude-sonnet-4"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"openrouter/openai/gpt-5.1-codex"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"openrouter/google/gemini-2.5-pro"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"openrouter/google/gemini-2.5-flash"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"openrouter/qwen/qwen3.6-plus"' "$project_legacy_enabled/.pi/settings.json"
assert_contains '"kimi-coding/kimi-for-coding"' "$project_legacy_enabled/.pi/settings.json"

# --- Deprecated Alibaba provider extension is removed/archived on repair ---
project_deprecated_alibaba="$tmp_root/deprecated-alibaba"
mkdir -p "$project_deprecated_alibaba/.git" "$project_deprecated_alibaba/.pi/extensions"
cat > "$project_deprecated_alibaba/.pi/extensions/alibaba-model-studio.ts" <<'EOF'
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
const DEFAULT_ALIBABA_MODEL_STUDIO_BASE_URL = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1";
export default function (pi: ExtensionAPI) {
  pi.registerProvider("alibaba", {
    baseUrl: DEFAULT_ALIBABA_MODEL_STUDIO_BASE_URL,
    apiKey: "DASHSCOPE_API_KEY",
    authHeader: true,
    api: "openai-completions",
    models: [
      {
        id: "qwen3.6-plus",
        name: "Alibaba Qwen 3.6 Plus",
        reasoning: true,
        input: ["text", "image"],
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
        contextWindow: 1000000,
        maxTokens: 65536
      }
    ]
  });
}
EOF

deprecated_alibaba_log="$tmp_root/deprecated-alibaba-init.log"
run_init "$project_deprecated_alibaba" "$deprecated_alibaba_log"
assert_not_exists "$project_deprecated_alibaba/.pi/extensions/alibaba-model-studio.ts"
assert_exists "$project_deprecated_alibaba/.pi/extensions/aoc-models.ts"
assert_contains "Removed deprecated PI extension: alibaba-model-studio.ts" "$deprecated_alibaba_log"

# --- Existing repo migration flow ---
project_migration="$tmp_root/migration"
mkdir -p "$project_migration/.git"
mkdir -p "$project_migration/.aoc/prompts/pi" "$project_migration/.aoc/skills/custom" "$project_migration/.pi/prompts"

printf 'legacy tmcc prompt\n' > "$project_migration/.aoc/prompts/pi/tmcc.md"
cat > "$project_migration/.aoc/skills/custom/SKILL.md" <<'EOF'
---
name: custom
description: custom migration skill
---
EOF

# Duplicate alias case: both files exist with identical content -> alias should be removed.
printf 'canonical tm-cc\n' > "$project_migration/.pi/prompts/tm-cc.md"
printf 'canonical tm-cc\n' > "$project_migration/.pi/prompts/tmcc.md"

migration_log="$tmp_root/migration-init.log"
run_init "$project_migration" "$migration_log"

assert_exists "$project_migration/.pi/prompts/tm-cc.md"
assert_not_exists "$project_migration/.pi/prompts/tmcc.md"
assert_exists "$project_migration/.pi/skills/custom/SKILL.md"

# Non-destructive migration keeps legacy source content in place.
assert_exists "$project_migration/.aoc/prompts/pi/tmcc.md"
assert_exists "$project_migration/.aoc/skills/custom/SKILL.md"

assert_contains "Removed legacy PI prompt alias duplicate: .pi/prompts/tmcc.md" "$migration_log"
assert_contains "Migrated legacy PI skill: .aoc/skills/custom -> .pi/skills/custom" "$migration_log"

# --- Installed-copy flow uses cached package seed + versioned migration repair ---
seed_cached_pi_runtime
installed_bin_dir="$tmp_root/installed/bin"
mkdir -p "$installed_bin_dir"
installed_aoc_init="$installed_bin_dir/aoc-init"
cp "$aoc_init_bin" "$installed_aoc_init"
chmod +x "$installed_aoc_init"

project_versioned="$tmp_root/versioned-migration"
mkdir -p "$project_versioned/.git" "$project_versioned/.aoc" "$project_versioned/.pi"
cat > "$project_versioned/.aoc/init-state.json" <<'EOF'
{
  "schemaVersion": 1,
  "projectAocVersion": 0
}
EOF
cat > "$project_versioned/.pi/settings.json" <<'EOF'
{
  "extensions": [],
  "packages": [
    "./packages/pi-multi-auth-aoc"
  ]
}
EOF

versioned_log_1="$tmp_root/versioned-init-1.log"
run_installed_init "$installed_aoc_init" "$project_versioned" "$versioned_log_1"
assert_exists "$project_versioned/.pi/packages/pi-multi-auth-aoc/package.json"
assert_contains '"projectAocVersion": 2' "$project_versioned/.aoc/init-state.json"
assert_contains '"available": true' "$project_versioned/.aoc/init-state.json"
assert_contains "Applying AOC project migration v1: initialize versioned state and repair PI runtime package wiring." "$versioned_log_1"

versioned_log_2="$tmp_root/versioned-init-2.log"
run_installed_init "$installed_aoc_init" "$project_versioned" "$versioned_log_2"
if grep -Fq "Applying AOC project migration v1" "$versioned_log_2"; then
  fail "Did not expect versioned migration to rerun once projectAocVersion is current"
fi

status_log="$tmp_root/status.log"
bash "$aoc_init_bin" --status "$project_versioned" >"$status_log" 2>&1
assert_contains 'AOC Init Status' "$status_log"
assert_contains 'project_aoc_version: 2' "$status_log"
assert_contains 'pi_runtime_status: ok' "$status_log"
assert_contains 'pi_multi_auth_package: present' "$status_log"

install_count="$(grep -c 'pi-multi-auth' "$AOC_PI_TEST_INSTALL_LOG" 2>/dev/null || true)"
[[ "$install_count" -eq 0 ]] || fail "Expected no global pi-multi-auth install attempts across runs, got $install_count"

echo "aoc-init PI-first fresh + migration smoke tests passed."
