#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

PYTHONDONTWRITEBYTECODE=1 python3 -B - "$root/bin/aoc-worker-wait" "$tmp_dir" <<'PY'
import json
import os
from pathlib import Path
import subprocess
import sys

cli = Path(sys.argv[1])
tmp = Path(sys.argv[2]).resolve()
fake = tmp / "fake-herdr"
fake.write_text('''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys

path = Path(os.environ["FAKE_HERDR_STATE"])
state = json.loads(path.read_text())
if sys.argv[1:3] == ["pane", "read"] and len(sys.argv) == 4:
    # Screen of the frame most recently returned by pane list.
    frame = state["frames"][min(state["poll"] - 1, len(state["frames"]) - 1)]
    print(frame.get("screens", {}).get(sys.argv[3], ""))
    sys.exit(frame.get("read_code", 0))
assert sys.argv[1:] == ["pane", "list"], sys.argv
index = state["poll"]
frame = state["frames"][min(index, len(state["frames"]) - 1)]
state["poll"] += 1
path.write_text(json.dumps(state))
for filename, content in frame.get("results", {}).items():
    Path(filename).write_text(content)
if "raw" in frame:
    print(frame["raw"])
else:
    print(json.dumps({"result": {"panes": frame.get("panes", [])}}))
sys.exit(frame.get("code", 0))
''')
fake.chmod(0o755)
env = {**os.environ, "AOC_HERDR_BIN": str(fake), "HOME": str(tmp)}
p1, p2 = "w1D:p14", "w2D:p14"


def frame(*statuses, results=None, **extra):
    return {"panes": [{"pane_id": pane, "agent_status": status} for pane, status in statuses],
            "results": {str(path): content for path, content in (results or {}).items()}, **extra}


def invoke(name, frames, workers, code, *, timeout=0.8, stall=0.25, as_json=True,
           overrides=None):
    state_file = tmp / f"{name}.json"
    state_file.write_text(json.dumps({"poll": 0, "frames": frames}))
    args = [str(cli), "--timeout", str(timeout), "--interval", "0.05", "--stall", str(stall)]
    for pane, path in workers:
        args.extend(["--worker", f"{pane}={path}"])
    if as_json:
        args.append("--json")
    result = subprocess.run(args, capture_output=True, text=True, timeout=5,
                            env={**env, "FAKE_HERDR_STATE": str(state_file), **(overrides or {})})
    assert result.returncode == code, (name, result.returncode, result.stdout, result.stderr)
    report = None
    if as_json:
        report = json.loads(result.stdout)
        assert set(report) == {"status", "workers"}, report
        assert report["status"] == {0: "done", 1: "failed", 2: "timeout"}[code], report
        assert [worker["pane"] for worker in report["workers"]] == [pane for pane, _ in workers]
        for worker, (_, path) in zip(report["workers"], workers):
            assert set(worker) == {"pane", "result_file", "state", "idle_seconds"}, worker
            assert worker["result_file"] == str(Path(path).expanduser()), worker
            if worker["state"] in {"idle", "stalled"}:
                assert isinstance(worker["idle_seconds"], (int, float)), worker
                assert worker["idle_seconds"] >= 0, worker
            else:
                assert worker["idle_seconds"] is None, worker
    return result, report, json.loads(state_file.read_text())["poll"]


# Two workers remain working until explicit result files appear after several polls.
r1, r2 = tmp / "first result", tmp / "second=result"
working = frame((p1, "working"), (p2, "working"))
result, report, polls = invoke("both", [working] * 3 + [
    frame((p1, "working"), (p2, "working"), results={r1: "Finished\n", r2: "Finished\n"})
], [(p2, r2), (p1, r1)], 0)
assert [worker["state"] for worker in report["workers"]] == ["done", "done"], report
assert polls == 4 and not result.stderr, (polls, result.stderr)

# Non-working statuses, including 'done', are not completion; working resets the idle timer.
blip_result = tmp / "blips.result"
blips = [frame((p1, status)) for status in ("working", "idle", "working", "done",
                                          "working", "blocked", "working", "unknown")] * 30
result, report, polls = invoke("blips", blips, [(p1, blip_result)], 2, timeout=0.65, stall=0.2)
assert report["workers"][0]["state"] in {"running", "idle"}, report
assert polls >= 5 and not result.stderr, (polls, result.stderr)

# An "unknown" detector status falls back to the pane spinner: busy screens keep the worker
# running past --stall, a failed read changes nothing, and a quiet screen stalls.
spinner = "output\n  \U000f12b7 Working on slice\n" + "\n" * 3 + "status bar"
unknown_result = tmp / "unknown.result"
result, report, polls = invoke("unknown-busy", [frame((p1, "unknown"), screens={p1: spinner})] * 200,
                               [(p1, unknown_result)], 2, timeout=0.5, stall=0.15)
assert report["workers"][0]["state"] == "running" and polls >= 5, (report, polls)
assert not result.stderr, result.stderr
result, report, _ = invoke("unknown-read-fails", [frame((p1, "unknown"), read_code=1)] * 200,
                           [(p1, unknown_result)], 2, timeout=0.4, stall=0.1)
assert report["workers"][0]["state"] == "running" and "pane read" in result.stderr, (report, result.stderr)
result, report, _ = invoke("unknown-quiet", [frame((p1, "unknown"), screens={p1: "prompt\nstatus bar"})],
                           [(p1, unknown_result)], 1, stall=0.15)
assert report["workers"][0]["state"] == "stalled", report

# A disappeared pane fails, but another worker can still finish.
missing_result, survivor = tmp / "missing.result", tmp / "survivor.result"
result, report, polls = invoke("missing", [
    working, frame((p2, "working")),
    frame((p1, "working"), (p2, "working"), results={missing_result: "Too late", survivor: "Done"}),
], [(p1, missing_result), (p2, survivor)], 1)
assert [worker["state"] for worker in report["workers"]] == ["missing", "done"], report
assert polls == 3 and not result.stderr, (polls, result.stderr)

# Continuous idle stalls; whitespace-only results do not complete a worker.
idle_result = tmp / "idle.result"
idle_result.write_text(" \n\t")
result, report, polls = invoke("stalled", [frame((p1, "idle"))], [(p1, idle_result)], 1,
                                stall=0.15)
assert report["workers"][0]["state"] == "stalled", report
assert report["workers"][0]["idle_seconds"] >= 0.15, report
assert not result.stderr, result.stderr

# A preexisting result wins over a missing pane, with expanduser and plain-text output.
(tmp / "ready.result").write_text("Ready\n")
result, _, polls = invoke("ready", [frame()], [(p1, "~/ready.result")], 0, as_json=False)
assert result.stdout == (f"{p1} done {tmp / 'ready.result'}\n"
                         "summary: 1 done, 0 running, 0 idle, 0 missing, 0 stalled\n"), result.stdout
assert polls == 1 and not result.stderr, (polls, result.stderr)

# Nonzero exits, invalid JSON, and malformed shapes never imply a missing pane.
failure_result = tmp / "failures.result"
result, report, polls = invoke("failures", [
    frame((p1, "working")), frame(code=3), frame(raw="not JSON"), frame(raw='{"result": {}}'),
    frame((p1, "working"), results={failure_result: "Recovered"}),
], [(p1, failure_result)], 0)
assert report["workers"][0]["state"] == "done", report
assert polls == 5 and len(result.stderr.splitlines()) == 3, (polls, result.stderr)
assert all("warning:" in line for line in result.stderr.splitlines()), result.stderr

# Result files still complete on a failed poll, and completion is terminal.
terminal_result, pending = tmp / "terminal.result", tmp / "pending.result"
result, report, polls = invoke("terminal", [
    frame(code=4, results={terminal_result: "Done"}),
    frame((p2, "working"), results={terminal_result: " ", pending: "Done"}),
], [(p1, terminal_result), (p2, pending)], 0)
assert [worker["state"] for worker in report["workers"]] == ["done", "done"], report
assert polls == 2 and len(result.stderr.splitlines()) == 1, (polls, result.stderr)

# Repeated failures preserve an idle state, not missing or stalled.
result, report, _ = invoke("failed-idle", [frame((p1, "idle")), frame(code=5)],
                           [(p1, tmp / "failed-idle.result")], 2, timeout=0.3, stall=0.1)
assert report["workers"][0]["state"] == "idle", report
assert report["workers"][0]["idle_seconds"] >= 0.1, report
assert result.stderr, result

# OSError on launching herdr preserves the initial nonterminal state.
result, report, polls = invoke("no-binary", [frame()], [(p1, tmp / "no-binary.result")], 2,
                                timeout=0.15, stall=0.1,
                                overrides={"AOC_HERDR_BIN": str(tmp / "absent-herdr")})
assert report["workers"][0]["state"] == "running" and polls == 0, report
assert all("warning:" in line for line in result.stderr.splitlines()), result.stderr

# Plain-text ordering, idle detail, and all summary counters.
result, _, _ = invoke("plain-failed", [frame((p2, "idle"))],
                       [(p1, tmp / "plain-missing"), (p2, tmp / "plain-stalled")], 1,
                       stall=0.1, as_json=False)
lines = result.stdout.splitlines()
assert lines[0] == f"{p1} missing {tmp / 'plain-missing'}", lines
assert lines[1].startswith(f"{p2} stalled {tmp / 'plain-stalled'} (idle "), lines
assert lines[1].endswith("s)"), lines
assert lines[2] == "summary: 0 done, 0 running, 0 idle, 1 missing, 1 stalled", lines

# All usage failures return 64 before invoking herdr.
valid_worker = ["--worker", f"{p1}={tmp / 'usage.result'}"]
invalid_args = [[], ["--worker", "p14=result"], ["--worker", f"{p1}="],
                ["--worker", p1], valid_worker * 2,
                valid_worker + ["--timeout", "1", "--stall", "1"],
                valid_worker + ["--timeout", "1", "--stall", "2"],
                valid_worker + ["--unknown"]]
for option in ("--timeout", "--interval", "--stall"):
    for value in ("0", "-1", "nan", "inf", "-inf", "no-number"):
        invalid_args.append(valid_worker + [f"{option}={value}"])
for args in invalid_args:
    result = subprocess.run([str(cli), *args], env=env, capture_output=True, text=True, timeout=5)
    assert result.returncode == 64, (args, result.returncode, result.stderr)
    assert result.stderr and not result.stdout, (args, result.stdout, result.stderr)
PY

echo "aoc-worker-wait smoke passed"
