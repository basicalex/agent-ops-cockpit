#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

fake_bin="$tmp_dir/bin"
project="$tmp_dir/project"
state="$tmp_dir/herdr-state.json"
mkdir -p "$fake_bin" "$project/.aoc"
printf '[search]\nenabled = true\n' > "$project/.aoc/search.toml"
printf '{"workspaces":[],"tabs":[],"panes":[],"log":[]}' > "$state"

python3 - "$fake_bin/herdr" <<'PY'
import sys
from pathlib import Path

Path(sys.argv[1]).write_text(r'''#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

state_path = Path(os.environ["HERDR_FAKE_STATE"])


def load():
    return json.loads(state_path.read_text())


def save(state):
    state_path.write_text(json.dumps(state, indent=2))


def log(state, argv):
    state.setdefault("log", []).append(argv)


def opt(argv, name, default=None):
    if name not in argv:
        return default
    idx = argv.index(name)
    return argv[idx + 1] if idx + 1 < len(argv) else default


def emit(payload):
    print(json.dumps(payload))


argv = sys.argv[1:]
if os.environ.get("HERDR_FAKE_DOWN") == "1" and argv[:2] == ["status", "server"]:
    print("status: stopped", file=sys.stderr)
    raise SystemExit(1)

state = load()
log(state, argv)

if argv[:2] == ["status", "server"]:
    save(state)
    print("status: running")
    raise SystemExit(0)

if argv and argv[0] == "--session":
    save(state)
    print("fake herdr session")
    raise SystemExit(0)

if argv[:2] == ["workspace", "list"]:
    save(state)
    emit({"id": "cli:workspace:list", "result": {"type": "workspace_list", "workspaces": state["workspaces"]}})
    raise SystemExit(0)

if argv[:2] == ["workspace", "create"]:
    workspace_id = f"w{len(state['workspaces']) + 1}"
    label = opt(argv, "--label", "workspace")
    workspace = {"workspace_id": workspace_id, "label": label, "focused": "--focus" in argv, "pane_count": 0, "tab_count": 0}
    state["workspaces"].append(workspace)
    save(state)
    emit({"id": "cli:workspace:create", "result": {"workspace": workspace}})
    raise SystemExit(0)

if argv[:2] == ["workspace", "focus"]:
    target = argv[2]
    for workspace in state["workspaces"]:
        workspace["focused"] = workspace["workspace_id"] == target
    save(state)
    emit({"id": "cli:workspace:focus", "result": {"workspace_id": target}})
    raise SystemExit(0)

if argv[:2] == ["tab", "list"]:
    workspace_id = opt(argv, "--workspace")
    tabs = [tab for tab in state["tabs"] if workspace_id is None or tab["workspace_id"] == workspace_id]
    save(state)
    emit({"id": "cli:tab:list", "result": {"tabs": tabs}})
    raise SystemExit(0)

if argv[:2] == ["tab", "create"]:
    workspace_id = opt(argv, "--workspace")
    label = opt(argv, "--label", "tab")
    tab_id = f"{workspace_id}:{len([t for t in state['tabs'] if t['workspace_id'] == workspace_id]) + 1}"
    pane_id = f"{workspace_id}-p{len(state['panes']) + 1}"
    tab = {"tab_id": tab_id, "workspace_id": workspace_id, "label": label, "focused": "--focus" in argv, "pane_count": 1}
    pane = {"pane_id": pane_id, "workspace_id": workspace_id, "tab_id": tab_id, "cwd": opt(argv, "--cwd", os.getcwd())}
    state["tabs"].append(tab)
    state["panes"].append(pane)
    for workspace in state["workspaces"]:
        if workspace["workspace_id"] == workspace_id:
            workspace["tab_count"] += 1
            workspace["pane_count"] += 1
    save(state)
    emit({"id": "cli:tab:create", "result": {"tab": tab}})
    raise SystemExit(0)

if argv[:2] == ["pane", "list"]:
    workspace_id = opt(argv, "--workspace")
    panes = [pane for pane in state["panes"] if workspace_id is None or pane["workspace_id"] == workspace_id]
    save(state)
    emit({"id": "cli:pane:list", "result": {"panes": panes}})
    raise SystemExit(0)

if argv[:2] == ["pane", "rename"]:
    pane_id = argv[2]
    label = argv[3]
    for pane in state["panes"]:
        if pane["pane_id"] == pane_id:
            pane["label"] = label
    save(state)
    emit({"id": "cli:pane:rename", "result": {"pane_id": pane_id, "label": label}})
    raise SystemExit(0)

if argv[:2] == ["pane", "run"]:
    pane_id = argv[2]
    command = argv[3]
    for pane in state["panes"]:
        if pane["pane_id"] == pane_id:
            pane["command"] = command
    save(state)
    emit({"id": "cli:pane:run", "result": {"pane_id": pane_id}})
    raise SystemExit(0)

save(state)
print("unsupported fake herdr command: " + " ".join(argv), file=sys.stderr)
raise SystemExit(2)
''')
PY
chmod +x "$fake_bin/herdr"

run_with_fake() {
  PATH="$fake_bin:$root/bin:$PATH" HERDR_FAKE_STATE="$state" "$@"
}

assert_state() {
  python3 - "$state" "$@" <<'PY'
import json
import sys
from pathlib import Path

state = json.loads(Path(sys.argv[1]).read_text())
mode = sys.argv[2]
if mode == "created":
    assert len(state["workspaces"]) == 1, state
    assert state["workspaces"][0]["label"].startswith("AOC Services · project · "), state
    assert [tab["label"] for tab in state["tabs"]] == ["Overview", "Search"], state
    assert [pane.get("label") for pane in state["panes"]] == ["AOC Services", "Managed Search"], state
    commands = [pane.get("command", "") for pane in state["panes"]]
    assert any("aoc-services up --watch --interval 30" in command for command in commands), state
    assert any("aoc-search status" in command for command in commands), state
elif mode == "idempotent":
    assert len(state["workspaces"]) == 1, state
    assert len(state["tabs"]) == 2, state
    assert len(state["panes"]) == 2, state
elif mode == "focused":
    assert any(entry[:2] == ["workspace", "focus"] for entry in state["log"]), state
elif mode == "launch_auto":
    labels = [workspace["label"] for workspace in state["workspaces"]]
    assert "project" in labels, state
    assert any(label.startswith("AOC Services · project · ") for label in labels), state
    assert any(entry[:2] == ["--session", "test"] for entry in state["log"]), state
else:
    raise AssertionError(mode)
PY
}

output="$(run_with_fake "$root/bin/aoc-herdr-services" ensure --cwd "$project" --no-focus)"
if [[ "$output" != *"AOC Services workspace created"* || "$output" != *"tabs: Overview, Search"* ]]; then
  echo "ERROR: ensure did not create expected Services workspace" >&2
  printf '%s\n' "$output" >&2
  exit 1
fi
assert_state created

output="$(run_with_fake "$root/bin/aoc-herdr-services" ensure --cwd "$project" --no-focus)"
if [[ "$output" != *"tabs: reused existing Overview/Search"* ]]; then
  echo "ERROR: second ensure did not reuse existing tabs" >&2
  printf '%s\n' "$output" >&2
  exit 1
fi
assert_state idempotent

run_with_fake "$root/bin/aoc-herdr-services" focus --cwd "$project" >/dev/null
assert_state focused

if PATH="$fake_bin:$root/bin:$PATH" HERDR_FAKE_STATE="$state" HERDR_FAKE_DOWN=1 "$root/bin/aoc-herdr-services" ensure --cwd "$project" --no-focus >/dev/null 2>&1; then
  echo "ERROR: ensure succeeded when fake Herdr server was down" >&2
  exit 1
fi

printf '{"workspaces":[],"tabs":[],"panes":[],"log":[]}' > "$state"
PATH="$fake_bin:$root/bin:$PATH" HERDR_FAKE_STATE="$state" AOC_HERDR_SERVICES=auto "$root/bin/aoc-herdr-launch" --cwd "$project" --label project --session test >/dev/null
assert_state launch_auto

printf 'AOC Herdr Services workspace smoke passed\n'
