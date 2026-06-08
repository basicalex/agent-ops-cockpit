#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

export PATH="$repo_root/bin:$PATH"

bash -n bin/aoc-context
bash -n bin/aoc-agent-wrap
bash -n bin/aoc-pi

registry_json="$(mktemp)"
bin/aoc-context registry --json >"$registry_json"
python3 - "$registry_json" <<'PY'
import json
import sys

data = json.load(open(sys.argv[1]))
ids = {record["id"]: record for record in data["records"]}
required = [
    "policy.effective-agent-contract",
    "policy.raw-agents-chain",
    "project.context-snapshot",
    "startup.handshake-metadata",
    "task.active-tag",
    "task.detail",
    "spec.current",
    "memory.aoc-mem",
    "stm.resume",
    "mind.context-pack",
    "pi.skills",
    "pi.prompts",
    "pi.extensions",
    "aoc.presets",
    "design.root-contract",
    "pi.subagents",
]
missing = [source_id for source_id in required if source_id not in ids]
assert not missing, missing
assert ids["policy.effective-agent-contract"]["loadingClass"] == "always"
assert ids["policy.raw-agents-chain"]["loadingClass"] == "manual-only"
assert ids["pi.skills"]["loadingClass"] == "index-only"
assert ids["pi.prompts"]["loadingClass"] == "index-only"
assert ids["pi.extensions"]["loadingClass"] == "never-inject-source"
assert ids["mind.context-pack"]["loadingClass"] == "intent-triggered"
assert ids["task.detail"]["loadingClass"] == "intent-triggered"
assert data["agents"]["status"] == "fresh"
PY
rm -f "$registry_json"

capabilities_json="$(mktemp)"
bin/aoc-context capabilities --json >"$capabilities_json"
python3 - "$capabilities_json" <<'PY'
import json
import sys

data = json.load(open(sys.argv[1]))
vcs = data["tools"]["vcs"]
assert vcs["kind"] in {"git", "jj", "none", "unknown"}, vcs
assert vcs["source"] == "aoc-handshake --json", vcs
PY
rm -f "$capabilities_json"

bin/aoc-context stale >/dev/null
bin/aoc-context explain-startup | grep -q 'policy.effective-agent-contract'
bin/aoc-context explain-startup | grep -q 'Everything else is index-only'
bin/aoc-context why pi.extensions | grep -q 'never-inject-source'

stub_dir="$(mktemp -d)"
trap 'rm -rf "$stub_dir"' EXIT
cat >"$stub_dir/pi" <<'EOF'
#!/usr/bin/env bash
printf 'AOC_CONTEXT_KERNEL_ACTIVE=%s\n' "${AOC_CONTEXT_KERNEL_ACTIVE:-}"
idx=0
for arg in "$@"; do
  idx=$((idx+1))
  printf 'ARG_%02d=%s\n' "$idx" "$arg"
done
EOF
chmod +x "$stub_dir/pi"

out="$stub_dir/out"
AOC_PROJECT_ROOT="$repo_root" \
AOC_AGENT_RUN=1 \
AOC_AGENT_ID=pi \
AOC_PI_USE_WRAP_RS=off \
AOC_HANDSHAKE_MODE=off \
bin/aoc-agent-wrap pi "$stub_dir/pi" "PI Agent" --version >"$out"

grep -q 'AOC_CONTEXT_KERNEL_ACTIVE=1' "$out"
grep -Eq '^ARG_[0-9][0-9]=--no-context-files$' "$out"
grep -Eq '^ARG_[0-9][0-9]=--append-system-prompt$' "$out"
grep -q '# AOC Context Kernel' "$out"
grep -q '## Effective Agent Contract' "$out"
grep -q '## Project Snapshot' "$out"
grep -q '## Context Router Startup Explanation' "$out"
grep -q '## Handshake Metadata' "$out"

out_off="$stub_dir/out-off"
AOC_PROJECT_ROOT="$repo_root" \
AOC_AGENT_RUN=1 \
AOC_AGENT_ID=pi \
AOC_PI_USE_WRAP_RS=off \
AOC_HANDSHAKE_MODE=off \
AOC_PI_CONTEXT_KERNEL=off \
bin/aoc-agent-wrap pi "$stub_dir/pi" "PI Agent" --version >"$out_off"

grep -q 'AOC_CONTEXT_KERNEL_ACTIVE=0' "$out_off"
if grep -q -- '--no-context-files' "$out_off"; then
  echo "context kernel off should not force --no-context-files" >&2
  exit 1
fi

echo "context router startup tests passed"
