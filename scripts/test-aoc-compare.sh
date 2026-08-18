#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

fake_bin="$tmp_dir/bin"
state="$tmp_dir/herdr-state.json"
contract="$tmp_dir/contract.md"
mkdir -p "$fake_bin"
printf '{"log":[]}' > "$state"
printf 'CONTRACT_TEXT\n' > "$contract"

python3 - "$fake_bin/herdr" <<'PY'
import sys
from pathlib import Path

Path(sys.argv[1]).write_text(r'''#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

state_path = Path(os.environ["HERDR_FAKE_STATE"])
state = json.loads(state_path.read_text())
argv = sys.argv[1:]
state["log"].append(argv)
state_path.write_text(json.dumps(state))

if argv[:2] == ["workspace", "list"]:
    print(json.dumps({"result": {"workspaces": []}}))
elif argv[:2] == ["workspace", "create"]:
    print(json.dumps({"result": {"workspace": {"workspace_id": "w1"}, "root_pane": {"pane_id": "p1"}}}))
elif argv[:2] == ["pane", "split"]:
    print(json.dumps({"result": {"pane": {"pane_id": "p2"}}}))
elif argv[:2] in (["workspace", "close"], ["pane", "rename"], ["pane", "run"]):
    print(json.dumps({"result": {}}))
else:
    print("unsupported fake herdr command: " + " ".join(argv), file=sys.stderr)
    raise SystemExit(2)
''')
PY
chmod +x "$fake_bin/herdr"

for harness in claude omp-raw prime-agent jcode; do
  cat > "$fake_bin/$harness" <<'EOF'
#!/usr/bin/env bash
if [[ "${1:-}" == "--help" ]]; then
  printf '%s\n' '  --model <id>  Select a model'
fi
EOF
  chmod +x "$fake_bin/$harness"
done

run_with_fake() {
  PATH="$fake_bin:$PATH" HERDR_FAKE_STATE="$state" "$@"
}

assert_log() {
  python3 - "$state" "$@" <<'PY'
import json
import sys
from pathlib import Path

log = json.loads(Path(sys.argv[1]).read_text())["log"]
mode = sys.argv[2]
if mode == "two_panes":
    assert sum(entry[:2] == ["workspace", "create"] for entry in log) == 1, log
    assert sum(entry[:2] == ["pane", "split"] for entry in log) == 1, log
    assert sum(entry[:2] == ["pane", "rename"] for entry in log) == 2, log
    assert sum(entry[:2] == ["pane", "run"] for entry in log) == 2, log
    names = [entry[3] for entry in log if entry[:2] == ["pane", "rename"]]
    assert names == ["claude", "omp-test"], names
elif mode == "claude_contract":
    commands = [entry[3] for entry in log if entry[:2] == ["pane", "run"]]
    assert len(commands) == 1, log
    assert "--append-system-prompt" in commands[0], commands
    assert "CONTRACT_TEXT" in commands[0], commands
elif mode == "omp_contract":
    contract = sys.argv[3]
    commands = [entry[3] for entry in log if entry[:2] == ["pane", "run"]]
    assert len(commands) == 1, log
    assert "--append-system-prompt" in commands[0], commands
    assert contract in commands[0], commands
else:
    raise AssertionError(mode)
PY
}

run_with_fake "$root/bin/aoc-compare" --help >/dev/null
if run_with_fake "$root/bin/aoc-compare" claude >/dev/null 2>&1; then
  echo "ERROR: missing prompt succeeded" >&2
  exit 1
fi

printf '{"log":[]}' > "$state"
run_with_fake "$root/bin/aoc-compare" --prompt 'review this change' --cwd "$tmp_dir" claude omp:test >/dev/null
assert_log two_panes

printf '{"log":[]}' > "$state"
run_with_fake "$root/bin/aoc-compare" --prompt 'review this change' --contract "$contract" --cwd "$tmp_dir" claude+contract >/dev/null
assert_log claude_contract

printf '{"log":[]}' > "$state"
run_with_fake "$root/bin/aoc-compare" --prompt 'review this change' --contract "$contract" --cwd "$tmp_dir" omp+contract >/dev/null
assert_log omp_contract "$contract"

if run_with_fake "$root/bin/aoc-compare" --prompt 'review this change' unknown >/dev/null 2>&1; then
  echo "ERROR: unknown harness succeeded" >&2
  exit 1
fi

printf 'aoc-compare smoke passed\n'
