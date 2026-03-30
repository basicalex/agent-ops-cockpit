#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
keep_tmp="${AOC_KEEP_MIND_RUNTIME_LIVE_TMP:-0}"
use_cargo="${AOC_VALIDATE_MIND_RUNTIME_USE_CARGO:-0}"
request_timeout_sec="${AOC_VALIDATE_MIND_RUNTIME_TIMEOUT_SEC:-}"
if [[ -z "$request_timeout_sec" ]]; then
  if [[ "$use_cargo" == "1" ]]; then
    request_timeout_sec="45"
  else
    request_timeout_sec="12"
  fi
fi
tmp="$(mktemp -d "${TMPDIR:-/tmp}/aoc-mind-live-XXXXXX")"
project_root="$tmp/project"
mkdir -p "$project_root"
session_id="mind-live-$(date +%s)-$$"
pane_id="41"
sock="$tmp/pulse.sock"
conversation_id="conv-live-runtime"
mind_store_path="$project_root/.aoc/mind/project.sqlite"
result_json="$tmp/result.json"
export_dir_json="$tmp/export-dir.txt"

free_port() {
  python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
}

hub_addr="127.0.0.1:$(free_port)"
hub_stdout="$tmp/hub.stdout"
hub_stderr="$tmp/hub.stderr"
agent_stdout="$tmp/agent.stdout"
agent_stderr="$tmp/agent.stderr"

resolve_wrap_rs_bin() {
  if [[ "${AOC_VALIDATE_MIND_RUNTIME_USE_CARGO:-0}" == "1" ]]; then
    printf '%s' "cargo run --manifest-path $repo_root/crates/Cargo.toml -p aoc-agent-wrap-rs --"
    return
  fi
  if [[ -x "$repo_root/crates/target/debug/aoc-agent-wrap-rs" ]]; then
    printf '%s' "$repo_root/crates/target/debug/aoc-agent-wrap-rs"
    return
  fi
  if [[ -x "$repo_root/crates/target/release/aoc-agent-wrap-rs" ]]; then
    printf '%s' "$repo_root/crates/target/release/aoc-agent-wrap-rs"
    return
  fi
  printf '%s' "cargo run --manifest-path $repo_root/crates/Cargo.toml -p aoc-agent-wrap-rs --"
}

resolve_hub_bin() {
  if [[ "${AOC_VALIDATE_MIND_RUNTIME_USE_CARGO:-0}" == "1" ]]; then
    printf '%s' "cargo run --manifest-path $repo_root/crates/Cargo.toml -p aoc-hub-rs --"
    return
  fi
  if [[ -x "$repo_root/crates/target/debug/aoc-hub-rs" ]]; then
    printf '%s' "$repo_root/crates/target/debug/aoc-hub-rs"
    return
  fi
  if [[ -x "$repo_root/crates/target/release/aoc-hub-rs" ]]; then
    printf '%s' "$repo_root/crates/target/release/aoc-hub-rs"
    return
  fi
  printf '%s' "cargo run --manifest-path $repo_root/crates/Cargo.toml -p aoc-hub-rs --"
}

wrap_rs_bin="$(resolve_wrap_rs_bin)"
hub_bin="$(resolve_hub_bin)"

hub_pid=""
wrap_pid=""
failed=0

cleanup() {
  local status=$?
  if [[ -n "$wrap_pid" ]] && kill -0 "$wrap_pid" >/dev/null 2>&1; then
    kill "$wrap_pid" >/dev/null 2>&1 || true
    wait "$wrap_pid" >/dev/null 2>&1 || true
  fi
  if [[ -n "$hub_pid" ]] && kill -0 "$hub_pid" >/dev/null 2>&1; then
    kill "$hub_pid" >/dev/null 2>&1 || true
    wait "$hub_pid" >/dev/null 2>&1 || true
  fi
  if [[ "$status" -ne 0 || "$failed" -ne 0 || "$keep_tmp" == "1" ]]; then
    echo "Preserved validation workspace: $tmp"
    return
  fi
  rm -rf "$tmp"
}
trap cleanup EXIT

wait_for_hub() {
  local tries=0
  while (( tries < 100 )); do
    if [[ -S "$sock" ]] && curl -fsS --max-time 1 "http://$hub_addr/health" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.2
    tries=$((tries + 1))
  done
  return 1
}

wait_for_file() {
  local path="$1"
  local tries=0
  while (( tries < 100 )); do
    [[ -e "$path" ]] && return 0
    sleep 0.2
    tries=$((tries + 1))
  done
  return 1
}

if [[ "$hub_bin" == *" "* ]]; then
  ZELLIJ_SESSION_NAME="" \
  AOC_LOG_DIR="$project_root/.aoc/logs" \
  AOC_PROJECT_ROOT="$project_root" \
  AOC_SESSION_ID="$session_id" \
  AOC_HUB_ADDR="$hub_addr" \
  AOC_PULSE_SOCK="$sock" \
  AOC_MIND_STORE_PATH="$mind_store_path" \
  sh -lc 'eval "$1 --session \"$2\" --addr \"$3\""' _ \
    "$hub_bin" "$session_id" "$hub_addr" \
    >"$hub_stdout" 2>"$hub_stderr" &
else
  ZELLIJ_SESSION_NAME="" \
  AOC_HUB_BIN="$hub_bin" \
  AOC_LOG_DIR="$project_root/.aoc/logs" \
  AOC_PROJECT_ROOT="$project_root" \
  AOC_SESSION_ID="$session_id" \
  AOC_HUB_ADDR="$hub_addr" \
  AOC_PULSE_SOCK="$sock" \
  AOC_MIND_STORE_PATH="$mind_store_path" \
  "$repo_root/bin/aoc-hub" >"$hub_stdout" 2>"$hub_stderr" &
fi
hub_pid=$!

wait_for_hub || {
  failed=1
  echo "Hub failed to start at $hub_addr" >&2
  exit 1
}

if [[ "$wrap_rs_bin" == *" "* ]]; then
  ZELLIJ_SESSION_NAME="" \
  AOC_LOG_DIR="$project_root/.aoc/logs" \
  AOC_PROJECT_ROOT="$project_root" \
  AOC_SESSION_ID="$session_id" \
  AOC_PANE_ID="$pane_id" \
  AOC_HUB_ADDR="$hub_addr" \
  AOC_PULSE_SOCK="$sock" \
  AOC_MIND_STORE_PATH="$mind_store_path" \
  AOC_AGENT_PTY=0 \
  AOC_HANDSHAKE_MODE=off \
  AOC_RTK_BYPASS=1 \
  AOC_LOG_STDOUT=1 \
  sh -lc 'eval "$1 --session \"$2\" --pane-id \"$3\" --agent-id pi --project-root \"$4\" --hub-addr \"$5\" -- bash -lc '\''printf \"pi live validation agent ready\\n\"; sleep 180'\''"' _ \
    "$wrap_rs_bin" "$session_id" "$pane_id" "$project_root" "$hub_addr" \
    >"$agent_stdout" 2>"$agent_stderr" &
else
  ZELLIJ_SESSION_NAME="" \
  AOC_LOG_DIR="$project_root/.aoc/logs" \
  AOC_PROJECT_ROOT="$project_root" \
  AOC_SESSION_ID="$session_id" \
  AOC_PANE_ID="$pane_id" \
  AOC_HUB_ADDR="$hub_addr" \
  AOC_PULSE_SOCK="$sock" \
  AOC_MIND_STORE_PATH="$mind_store_path" \
  AOC_AGENT_PTY=0 \
  AOC_HANDSHAKE_MODE=off \
  AOC_RTK_BYPASS=1 \
  AOC_LOG_STDOUT=1 \
  sh -lc 'exec "$0" --session "$1" --pane-id "$2" --agent-id pi --project-root "$3" --hub-addr "$4" -- bash -lc '\''printf "pi live validation agent ready\\n"; sleep 180'\''' \
    "$wrap_rs_bin" "$session_id" "$pane_id" "$project_root" "$hub_addr" \
    >"$agent_stdout" 2>"$agent_stderr" &
fi
wrap_pid=$!

SOCK="$sock" SESSION_ID="$session_id" CONVERSATION_ID="$conversation_id" PROJECT_ROOT="$project_root" REQUEST_TIMEOUT_SEC="$request_timeout_sec" AOC_MIND_STORE_PATH="$mind_store_path" \
python3 - <<'PY' >"$result_json"
import datetime as dt
import glob
import json
import os
import socket
import time
from pathlib import Path

sock_path = os.environ["SOCK"]
session_id = os.environ["SESSION_ID"]
conversation_id = os.environ["CONVERSATION_ID"]
project_root = Path(os.environ["PROJECT_ROOT"])
request_timeout = float(os.environ.get("REQUEST_TIMEOUT_SEC", "12"))


def now_iso() -> str:
    return dt.datetime.now(dt.UTC).isoformat()


def connect(role: str, subscribe: bool = False):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(request_timeout)
    s.connect(sock_path)
    reader = s.makefile("r", encoding="utf-8", newline="\n")
    hello = {
        "version": "1",
        "type": "hello",
        "session_id": session_id,
        "sender_id": f"mind-live-validator-{role}",
        "timestamp": now_iso(),
        "payload": {
            "client_id": f"mind-live-validator-{role}",
            "role": role,
            "capabilities": ["subscribe", "command"],
        },
    }
    s.sendall((json.dumps(hello) + "\n").encode())
    if subscribe:
        msg = {
            "version": "1",
            "type": "subscribe",
            "session_id": session_id,
            "sender_id": f"mind-live-validator-{role}",
            "timestamp": now_iso(),
            "payload": {"topics": ["agent_state", "health"]},
        }
        s.sendall((json.dumps(msg) + "\n").encode())
    return s, reader


def read_frame(reader):
    deadline = time.time() + request_timeout
    while time.time() < deadline:
        line = reader.readline()
        if not line:
            raise RuntimeError("pulse socket closed before response")
        if not line.strip():
            continue
        return json.loads(line)
    raise TimeoutError("timed out waiting for frame")


def wait_for_snapshot_state():
    s, reader = connect("subscriber", subscribe=True)
    deadline = time.time() + request_timeout
    last_states = []
    while time.time() < deadline:
        env = read_frame(reader)
        msg_type = env.get("type")
        if msg_type == "snapshot":
            states = env.get("payload", {}).get("states", [])
            last_states = states
            if states:
                reader.close()
                s.close()
                return states
            continue
        if msg_type == "delta":
            changes = env.get("payload", {}).get("changes", [])
            states = [change.get("state") for change in changes if isinstance(change, dict) and change.get("state")]
            if states:
                reader.close()
                s.close()
                return states
    reader.close()
    s.close()
    raise RuntimeError(f"no wrapper state snapshot received: {last_states!r}")


def request_command(target_agent_id: str, command: str, args: dict):
    sender = f"mind-live-validator-command-{command}"
    request_id = f"{command}-{int(time.time() * 1000)}"
    s, reader = connect("subscriber", subscribe=False)
    msg = {
        "version": "1",
        "type": "command",
        "session_id": session_id,
        "sender_id": sender,
        "request_id": request_id,
        "timestamp": now_iso(),
        "payload": {
            "command": command,
            "target_agent_id": target_agent_id,
            "args": args,
        },
    }
    s.sendall((json.dumps(msg) + "\n").encode())
    deadline = time.time() + request_timeout
    accepted = None
    while time.time() < deadline:
        env = read_frame(reader)
        if env.get("type") != "command_result":
            continue
        if env.get("request_id") != request_id:
            continue
        payload = env.get("payload", {})
        if payload.get("status") == "accepted":
            accepted = env
            continue
        reader.close()
        s.close()
        return env
    reader.close()
    s.close()
    if accepted is not None:
        return accepted
    raise TimeoutError(f"timed out waiting for command_result for {command}")


def snapshot_has_mind_fields():
    states = wait_for_snapshot_state()
    for state in states:
        source = state.get("source") or {}
        if "mind_observer" in source or "insight_runtime" in source:
            return True, states
    return False, states


def wait_for_export_dir():
    insight_root = project_root / ".aoc" / "mind" / "insight"
    deadline = time.time() + request_timeout
    while time.time() < deadline:
        dirs = sorted([Path(p) for p in glob.glob(str(insight_root / "*")) if Path(p).is_dir()])
        if dirs:
            return dirs[-1]
        time.sleep(0.2)
    raise RuntimeError("mind finalize did not create an insight export directory")


states = wait_for_snapshot_state()
state = states[0]
target_agent_id = state["agent_id"]

results = {}
results["initial_snapshot_states"] = len(states)
results["target_agent_id"] = target_agent_id
results["hub_snapshot_has_state"] = True

for command, args in [
    (
        "mind_ingest_event",
        {
            "conversation_id": conversation_id,
            "event_id": "evt-live-1",
            "timestamp_ms": 1700000005000,
            "body": {"kind": "message", "role": "user", "text": "live validation observer seed"},
        },
    ),
    (
        "run_observer",
        {"conversation_id": conversation_id, "reason": "live validator manual observer"},
    ),
    (
        "mind_handoff",
        {"conversation_id": conversation_id, "reason": "live validator handoff"},
    ),
    (
        "mind_finalize_session",
        {"conversation_id": conversation_id, "reason": "live validator finalize"},
    ),
]:
    env = request_command(target_agent_id, command, args)
    payload = env.get("payload", {})
    results[command] = {
        "status": payload.get("status"),
        "message": payload.get("message"),
    }
    if payload.get("status") != "ok":
        raise RuntimeError(f"{command} failed: {payload}")

prov = request_command(
    target_agent_id,
    "mind_provenance_query",
    {
        "project_root": str(project_root),
        "conversation_id": conversation_id,
        "max_nodes": 16,
        "max_edges": 16,
    },
)
payload = prov.get("payload", {})
results["mind_provenance_query"] = {
    "status": payload.get("status"),
}
if payload.get("status") != "ok":
    raise RuntimeError(f"mind_provenance_query failed: {payload}")
prov_json = json.loads(payload.get("message") or "{}")
results["provenance_status"] = prov_json.get("graph", {}).get("status")
if results["provenance_status"] != "ok":
    raise RuntimeError(f"unexpected provenance status: {results['provenance_status']!r}")

detached = request_command(
    target_agent_id,
    "insight_detached_status",
    {
        "owner_plane": "mind",
        "limit": 8,
    },
)
payload = detached.get("payload", {})
results["insight_detached_status"] = {
    "status": payload.get("status"),
}
if payload.get("status") != "ok":
    raise RuntimeError(f"insight_detached_status failed: {payload}")
detached_json = json.loads(payload.get("message") or "{}")
results["mind_detached_status"] = detached_json.get("status")
results["mind_detached_jobs"] = len(detached_json.get("jobs") or [])
results["mind_detached_worker_kinds"] = sorted(
    {
        str(job.get("worker_kind"))
        for job in (detached_json.get("jobs") or [])
        if job.get("worker_kind") is not None
    }
)
if detached_json.get("status") not in {"ok", "idle"}:
    raise RuntimeError(f"unexpected detached status result: {detached_json!r}")
for job in detached_json.get("jobs") or []:
    if job.get("owner_plane") != "mind":
        raise RuntimeError(f"non-mind detached job returned for mind status query: {job!r}")

has_mind_fields, latest_states = snapshot_has_mind_fields()
results["pulse_snapshot_has_mind_fields"] = has_mind_fields
results["snapshot_state_count_after_commands"] = len(latest_states)

export_dir = wait_for_export_dir()
results["export_dir"] = str(export_dir)
results["export_files"] = sorted(p.name for p in export_dir.iterdir())
results["store_exists"] = Path(os.environ.get("AOC_MIND_STORE_PATH") or str(project_root / ".aoc" / "mind" / "project.sqlite")).exists()
results["t1_exists"] = (export_dir / "t1.md").exists()
results["t2_exists"] = (export_dir / "t2.md").exists()
results["manifest_exists"] = (export_dir / "manifest.json").exists()

if not results["store_exists"]:
    raise RuntimeError("mind store file missing")
if not results["t1_exists"] or not results["t2_exists"] or not results["manifest_exists"]:
    raise RuntimeError("mind export bundle missing expected files")

print(json.dumps(results, indent=2))
PY

wait_for_file "$result_json" || {
  failed=1
  echo "Validation script did not produce results" >&2
  exit 1
}

python3 - <<'PY' "$result_json"
import json, sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    data = json.load(fh)
print("Mind live validation summary:")
print(f"- target agent: {data['target_agent_id']}")
print(f"- initial snapshot states: {data['initial_snapshot_states']}")
print(f"- pulse mind fields visible: {data['pulse_snapshot_has_mind_fields']}")
print(f"- provenance status: {data['provenance_status']}")
print(f"- detached status: {data['mind_detached_status']} jobs={data['mind_detached_jobs']} worker_kinds={','.join(data['mind_detached_worker_kinds']) or 'none'}")
print(f"- export dir: {data['export_dir']}")
print(f"- commands: ingest={data['mind_ingest_event']['status']} observer={data['run_observer']['status']} handoff={data['mind_handoff']['status']} finalize={data['mind_finalize_session']['status']} provenance={data['mind_provenance_query']['status']} detached={data['insight_detached_status']['status']}")
PY

log_dir="$project_root/.aoc/logs"
if [[ ! -d "$log_dir" ]]; then
  failed=1
  echo "Expected log dir missing: $log_dir" >&2
  exit 1
fi

hub_logs=("$log_dir"/aoc-hub-*.log)
wrap_logs=("$log_dir"/aoc-agent-wrap-*.log)
[[ -e "${hub_logs[0]:-}" ]] || { failed=1; echo "Expected hub log missing in $log_dir" >&2; exit 1; }
[[ -e "${wrap_logs[0]:-}" ]] || { failed=1; echo "Expected wrapper log missing in $log_dir" >&2; exit 1; }

echo "- hub log: ${hub_logs[0]}"
echo "- wrap log: ${wrap_logs[0]}"
echo "Mind runtime live validation passed."
