#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

python3 - "$root/bin/aoc-dispatch" "$tmp_dir" <<'PY'
import fcntl
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

command = Path(sys.argv[1])
temporary = Path(sys.argv[2]).resolve()
fake_bin = temporary / "bin"
project = temporary / "project"
fake_bin.mkdir()
project.mkdir()
subprocess.run(["git", "init", "--quiet", str(project)], check=True)
home = temporary / "home"
(home / ".config/aoc").mkdir(parents=True)
(home / ".config/aoc/communication-contract.md").write_text("Fixture communication contract.")
state_file = temporary / "github.json"
claude_calls = temporary / "claude.jsonl"
state_dir = temporary / "state/aoc/dispatch" / hashlib.sha256(str(project.resolve()).encode()).hexdigest()[:16]
environment = dict(os.environ, PATH=f"{fake_bin}:{os.environ['PATH']}", HOME=str(home),
                   XDG_STATE_HOME=str(temporary / "state"), FAKE_GH_STATE=str(state_file),
                   FAKE_CLAUDE_CALLS=str(claude_calls), FAKE_GH_ACTOR="aoc-bot",
                   AOC_DISPATCH_GH_BIN=str(fake_bin / "gh"), AOC_DISPATCH_CLAUDE_BIN=str(fake_bin / "claude"),
                   AOC_DISPATCH_NOW="2026-10-01T12:00:00Z", FAKE_CLAUDE_MODE="done",
                   FAKE_DISPATCH_STATE=str(state_dir / "state.json"))

(fake_bin / "gh").write_text(r'''#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

path = Path(os.environ["FAKE_GH_STATE"])
state = json.loads(path.read_text())
argv = sys.argv[1:]
actor = os.environ.get("FAKE_GH_ACTOR", "aoc-bot")
created = os.environ["AOC_DISPATCH_NOW"]
entry = {"argv": argv, "actor": actor, "created_at": created}
state["log"].append(entry)
repo = state["repo"]
output = None

def change(issue, action, label):
    labels = issue["labels"]
    if action == "--add-label" and {"name": label} not in labels:
        labels.append({"name": label})
        event = "labeled"
    elif action == "--remove-label" and {"name": label} in labels:
        labels.remove({"name": label})
        event = "unlabeled"
    else:
        return
    state["events"].append({"issue": issue["number"], "event": event, "label": {"name": label},
                            "actor": {"login": actor}, "created_at": created})
    issue["updatedAt"] = created

if argv == ["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"]:
    output = repo
elif argv[:2] == ["issue", "list"]:
    assert len(argv) == 12 and argv[2:7] == ["--repo", repo, "--state", "open", "--label"], argv
    assert argv[8:] == ["--json", "number,title,labels,updatedAt", "--limit", "50"], argv
    output = [{k: issue[k] for k in ("number", "title", "labels", "updatedAt")}
              for issue in reversed(list(state["issues"].values()))
              if issue["state"] == "open" and {"name": argv[7]} in issue["labels"]][:50]
elif argv[:2] == ["issue", "view"]:
    assert argv[3:] == ["--repo", repo, "--json", "number,title,body,author,labels,comments,url"], argv
    issue = state["issues"][argv[2]]
    output = {k: issue[k] for k in ("number", "title", "body", "author", "labels", "comments", "url")}
elif argv[:2] == ["issue", "edit"]:
    assert argv[3:5] == ["--repo", repo] and len(argv) >= 7 and len(argv) % 2 == 1, argv
    issue = state["issues"][argv[2]]
    if argv[5:] == ["--add-label", "agent-running", "--remove-label", "agent-ready"]:
        entry["local_state_before_claim"] = json.loads(Path(os.environ["FAKE_DISPATCH_STATE"]).read_text())
    for index in range(5, len(argv), 2):
        assert argv[index] in ("--add-label", "--remove-label"), argv
        if os.environ.get("FAKE_GH_CLAIM_NOOP") != "1":
            change(issue, argv[index], argv[index + 1])
elif argv[:2] == ["issue", "comment"]:
    assert argv[3:6] == ["--repo", repo, "--body-file"] and len(argv) == 7, argv
    state["issues"][argv[2]]["comments"].append({"body": Path(argv[6]).read_text(),
                                               "author": {"login": actor}, "createdAt": created})
elif argv[:1] == ["api"]:
    assert len(argv) == 3 and argv[2] == "--paginate", argv
    prefix = f"repos/{repo}/issues/"
    assert argv[1].startswith(prefix) and argv[1].endswith("/events"), argv
    number = int(argv[1][len(prefix):-len("/events")])
    output = [event for event in state["events"] if event["issue"] == number]
elif argv[:2] == ["label", "create"]:
    assert len(argv) == 10 and argv[3:6] == ["--repo", repo, "--color"] and argv[7] == "--description" and argv[9] == "--force", argv
    state["labels"][argv[2]] = {"color": argv[6], "description": argv[8]}
else:
    raise AssertionError("unsupported fake gh command: " + repr(argv))
path.write_text(json.dumps(state))
if output is not None:
    print(output if isinstance(output, str) else json.dumps(output))
''')
(fake_bin / "claude").write_text(r'''#!/usr/bin/env python3
import json
import os
import sys
import time
from pathlib import Path

run_dir = Path(os.environ["AOC_DISPATCH_RUN_DIR"])
meta = json.loads((run_dir / "meta.json").read_text())
with Path(os.environ["FAKE_CLAUDE_CALLS"]).open("a") as handle:
    handle.write(json.dumps({"argv": sys.argv[1:], "run_dir": str(run_dir), "cwd": os.getcwd(),
                             "run_id": os.environ["AOC_DISPATCH_RUN_ID"], "repo": os.environ["AOC_DISPATCH_REPO"],
                             "issue": os.environ["AOC_DISPATCH_ISSUE"]}) + "\n")
mode = os.environ["FAKE_CLAUDE_MODE"]
print("fake master output", flush=True)
if mode == "sleep":
    time.sleep(60)
    raise SystemExit(0)
if mode == "noreport":
    raise SystemExit(int(os.environ.get("FAKE_CLAUDE_EXIT", "0")))
status = mode if mode in ("done", "blocked", "failed") else "done"
report = {"schema": "aoc.dispatch.report/v1", "version": 1, "run_id": meta["run_id"],
          "assignmentId": meta["run_id"], "repo": meta["repo"], "issue": meta["issue"], "status": status,
          "summary": "Plan complete" if status == "done" else "Need decision" if status == "blocked" else "Master failed",
          "needsDecision": "Use option A or B?" if status == "blocked" else None,
          "decision_id": meta["run_id"] + "-d1" if status == "blocked" else None,
          "evidence": "Read issue and repository", "timestamp": os.environ["AOC_DISPATCH_NOW"]}
if mode == "invalid":
    report["run_id"] = "wrong-run"
(run_dir / "report.json").write_text(json.dumps(report))
raise SystemExit(int(os.environ.get("FAKE_CLAUDE_EXIT", "0")))
''')
(fake_bin / "aoc-handshake").write_text('#!/usr/bin/env python3\nprint(\'{"fixture": true}\')\n')
for binary in fake_bin.iterdir():
    binary.chmod(0o755)


def github():
    return json.loads(state_file.read_text())


def local():
    return json.loads((state_dir / "state.json").read_text())


def calls():
    return [json.loads(line) for line in claude_calls.read_text().splitlines()] if claude_calls.exists() else []


def reset(issues=None, actor="basicalex"):
    shutil.rmtree(state_dir, ignore_errors=True)
    claude_calls.unlink(missing_ok=True)
    environment.update(AOC_DISPATCH_NOW="2026-10-01T12:00:00Z", FAKE_CLAUDE_MODE="done")
    for name in ("FAKE_GH_CLAIM_NOOP", "FAKE_CLAUDE_EXIT"):
        environment.pop(name, None)
    state = {"repo": "fixture/dispatch", "issues": {}, "labels": {}, "events": [], "log": []}
    for number, labels in (issues or [(2, ["agent-ready"])]):
        state["issues"][str(number)] = {"number": number, "title": f"Issue {number}", "body": "Plan this change",
              "author": {"login": "reporter"}, "labels": [{"name": label} for label in labels],
              "comments": [{"body": "Earlier context", "author": {"login": "reporter"}, "createdAt": "2026-10-01T10:00:00Z"}],
              "url": f"https://example.invalid/issues/{number}", "state": "open", "updatedAt": "2026-10-01T11:00:00Z"}
        for label in labels:
            state["events"].append({"issue": number, "event": "labeled", "label": {"name": label},
                                    "actor": {"login": actor}, "created_at": "2026-10-01T11:00:00Z"})
    state_file.write_text(json.dumps(state))


def dispatch(*args, expected=0, defaults=True):
    argv = [str(command), "--root", str(project)]
    if defaults:
        argv += ["--repo", "fixture/dispatch"]
    proc = subprocess.run(argv + list(args), env=environment, capture_output=True, text=True, timeout=30)
    assert proc.returncode == expected, (argv, args, proc.returncode, proc.stdout, proc.stderr)
    return proc


def human(*flags, actor="basicalex"):
    subprocess.run([str(fake_bin / "gh"), "issue", "edit", "2", "--repo", "fixture/dispatch", *flags],
                   env=dict(environment, FAKE_GH_ACTOR=actor), check=True)


def answer(body, actor="basicalex"):
    path = temporary / "answer.txt"
    path.write_text(body)
    subprocess.run([str(fake_bin / "gh"), "issue", "comment", "2", "--repo", "fixture/dispatch", "--body-file", str(path)],
                   env=dict(environment, FAKE_GH_ACTOR=actor), check=True)


def issue_labels(number=2):
    return {label["name"] for label in github()["issues"][str(number)]["labels"]}


def result(number=2):
    record = local()["issues"][str(number)]
    return json.loads((state_dir / "runs" / record["last_run_id"] / "result.json").read_text())


def comments(event):
    return [comment["body"] for comment in github()["issues"]["2"]["comments"]
            if comment["body"].startswith("<!-- aoc-dispatch ") and f"event={event} -->" in comment["body"]]


def blocked():
    reset()
    environment["FAKE_CLAUDE_MODE"] = "blocked"
    dispatch("tick")
    record = local()["issues"]["2"]
    assert issue_labels() == {"needs-alex"}
    assert result()["outcome"] == "needs_alex"
    assert record["blocked_at"] == "2026-10-01T12:00:00Z"
    assert record["decision_id"] == record["last_run_id"] + "-d1"
    assert f"AOC-DECISION {record['decision_id']}" in comments("blocked")[0]
    assert "then remove `needs-alex` and add `agent-ready`." in comments("blocked")[0]
    return record["last_run_id"], record["decision_id"]


# a: full done path, exact launch permissions, packet, markers and state-before-claim.
reset()
dispatch("tick", defaults=False)
assert issue_labels() == {"agent-review"}
assert local()["seat"] == "IDLE" and local()["active"] is None
assert result()["outcome"] == "review"
run_id = local()["issues"]["2"]["last_run_id"]
assert re.fullmatch(r"2-20261001T120000Z-[0-9a-f]{4}", run_id)
launch = calls()[0]
argv = launch["argv"]
assert argv[:1] == ["-p"] and argv[argv.index("--model") + 1] == "claude-opus-5-5"
assert "--no-session-persistence" in argv and "--dangerously-skip-permissions" not in argv
allowed = argv[argv.index("--allowedTools") + 1:argv.index("--append-system-prompt")]
assert allowed == ["Read", "Glob", "Grep", "Bash(aoc-report:*)", "Bash(git status:*)", "Bash(git log:*)", "Bash(git diff:*)"]
assert argv[-1] == "Fixture communication contract."
assert launch["cwd"] == str(project) and launch["run_id"] == run_id and launch["repo"] == "fixture/dispatch" and launch["issue"] == "2"
assert comments("claim") and comments("result")
assert "Plan complete" in comments("result")[0] and "<details>" in comments("result")[0]
claim = next(entry for entry in github()["log"] if "local_state_before_claim" in entry)
assert claim["local_state_before_claim"]["seat"] == "CLAIMED"
assert claim["local_state_before_claim"]["active"]["run_id"] == run_id
packet = (Path(launch["run_dir"]) / "packet.md").read_text()
assert comments("claim")[0] in packet and "Earlier context" in packet
sections = ["# AOC Dispatch run ", "## Source", "## Mode rules", "## Issue", "## Comments", "## Repo state", "## AOC handshake", "## Reporting"]
assert [packet.index(section) for section in sections] == sorted(packet.index(section) for section in sections)
assert '"fixture": true' in packet and "do not delegate to workers or subagents" in packet
assert "report a plan" in packet and "Call `aoc-report` exactly once, as the last action." in packet
status = json.loads(dispatch("status", "--json").stdout)
assert status["seat"] == "IDLE" and status["issues"]["2"]["state"] == "review"

# b: repeated tick does not relaunch or change labels/comments.
before = github()
dispatch("tick")
assert len(calls()) == 1 and github()["events"] == before["events"]
assert github()["issues"]["2"]["comments"] == before["issues"]["2"]["comments"]

# c: untrusted ready actor, including a newer untrusted event after a trusted one.
reset(actor="stranger")
proc = dispatch("tick")
assert not calls() and not local()["issues"] and issue_labels() == {"agent-ready"}
assert "actor is not trusted" in proc.stderr
assert not any(entry["argv"][:2] == ["issue", "edit"] for entry in github()["log"])
reset()
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:01:00Z"
human("--remove-label", "agent-ready", actor="stranger")
human("--add-label", "agent-ready", actor="stranger")
dispatch("tick")
assert not calls() and issue_labels() == {"agent-ready"}

# d: missing reports and hard timeout (even with a controlled/frozen timestamp).
for mode, reason in (("noreport", "without a report"), ("sleep", "timeout"), ("failed", "Master failed"), ("invalid", "invalid report")):
    reset()
    environment["FAKE_CLAUDE_MODE"] = mode
    dispatch("tick", "--timeout", "0.2" if mode == "sleep" else "5")
    assert issue_labels() == {"agent-failed"}
    assert local()["seat"] == "IDLE" and result()["outcome"] == "failed"
    assert reason in result()["reason"] and reason in comments("failed")[0]
    assert "fake master output" in comments("failed")[0] and "<details>" in comments("failed")[0]

# e/f: blocked state, labels alone insufficient; exact ID, actor and timestamp gates.
original, decision_id = blocked()
environment.update(AOC_DISPATCH_NOW="2026-10-01T12:01:00Z", FAKE_CLAUDE_MODE="done")
human("--remove-label", "needs-alex")
human("--add-label", "agent-ready")
answer("looking at this")
dispatch("tick")
assert len(calls()) == 1 and local()["issues"]["2"]["ready_seen_at"] == "2026-10-01T12:01:00Z"
answer(f"AOC-DECISION {decision_id}\nUse option A", actor="stranger")
dispatch("tick")
assert len(calls()) == 1
answer(f"AOC-DECISION {decision_id}1\nWrong ID")
dispatch("tick")
assert len(calls()) == 1
# Matching ID from before the block must not count.
environment["AOC_DISPATCH_NOW"] = "2026-10-01T11:59:00Z"
answer(f"AOC-DECISION {decision_id}\nOld decision")
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:02:00Z"
answer(f"\nAOC-DECISION {decision_id}\nUse option A")
dispatch("tick")
assert len(calls()) == 2 and issue_labels() == {"agent-review"}
continued = Path(calls()[-1]["run_dir"])
meta = json.loads((continued / "meta.json").read_text())
assert meta["continuation_of"] == original and meta["decision_id"] == decision_id
packet = (continued / "packet.md").read_text()
assert packet.index("## Issue") < packet.index("## Decision") < packet.index("## Comments")
assert f"> AOC-DECISION {decision_id}\n> Use option A" in packet
assert local()["issues"]["2"]["decision_id"] is None

# Current needs-alex, and label changes by another actor, each prevent continuation.
for gate in ("still-blocked", "untrusted-remove"):
    original, decision_id = blocked()
    environment.update(AOC_DISPATCH_NOW="2026-10-01T12:01:00Z", FAKE_CLAUDE_MODE="done")
    if gate == "untrusted-remove":
        human("--remove-label", "needs-alex", actor="stranger")
    human("--add-label", "agent-ready")
    answer(f"AOC-DECISION {decision_id}\nUse option A")
    dispatch("tick")
    assert len(calls()) == 1 and local()["issues"]["2"]["state"] == "needs_alex"

# g: timeout boundary and rollback order must not leave both labels present.
original, decision_id = blocked()
environment.update(AOC_DISPATCH_NOW="2026-10-01T12:01:00Z", FAKE_CLAUDE_MODE="done")
human("--remove-label", "needs-alex")
human("--add-label", "agent-ready")
dispatch("tick", "--decision-timeout", "60")
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:02:00Z"
dispatch("tick", "--decision-timeout", "60")
assert issue_labels() == {"agent-ready"}  # Strictly greater, not >=.
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:02:01Z"
start = len(github()["log"])
dispatch("tick", "--decision-timeout", "60")
edits = [entry["argv"] for entry in github()["log"][start:] if entry["argv"][:2] == ["issue", "edit"]]
assert edits == [["issue", "edit", "2", "--repo", "fixture/dispatch", "--remove-label", "agent-ready"],
                 ["issue", "edit", "2", "--repo", "fixture/dispatch", "--add-label", "needs-alex"]]
assert issue_labels() == {"needs-alex"} and len(calls()) == 1
assert local()["issues"]["2"]["ready_seen_at"] is None
assert f"AOC-DECISION {decision_id}" in comments("resume-timeout")[0]
# Replay actual events and prove the timeout transition never had both labels.
labels = set()
for event in github()["events"]:
    if event["event"] == "labeled":
        labels.add(event["label"]["name"])
    else:
        labels.discard(event["label"]["name"])
    assert not {"agent-ready", "needs-alex"} <= labels

# h: dead PID recovery without a report; alive PID keeps the seat occupied.
reset([(2, ["agent-running"])])
dispatch("status", "--json")
run_id = "2-20261001T115900Z-abcd"
run_dir = state_dir / "runs" / run_id
run_dir.mkdir(parents=True)
(run_dir / "claude.log").write_text("interrupted master\n")
state = local()
state.update(seat="RUNNING", active={"run_id": run_id, "issue": 2, "pid": os.getpid()})
state["issues"]["2"] = {"state": "running", "last_run_id": run_id, "claimed_at": "2026-10-01T11:59:00Z",
                        "decision_id": None, "blocked_at": None, "ready_seen_at": None, "runs": [run_id]}
(state_dir / "state.json").write_text(json.dumps(state))
dispatch("tick")
assert local()["seat"] == "RUNNING" and not calls() and issue_labels() == {"agent-running"}
# Obtain a PID proven dead, without assuming a magic PID is unused.
dead = subprocess.Popen([sys.executable, "-c", "pass"])
dead.wait()
state["active"]["pid"] = dead.pid
(state_dir / "state.json").write_text(json.dumps(state))
dispatch("tick")
assert local()["seat"] == "IDLE" and local()["active"] is None
assert issue_labels() == {"agent-failed"} and result()["outcome"] == "failed"
assert result()["reason"] == "master process ended without a report (dispatcher restart)"
assert "interrupted master" in comments("failed")[0]

# i: tick and watch both reject another poller while holding the same lock.
with (state_dir / "poll.lock").open("a+") as handle:
    fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    for mode in ("tick", "watch"):
        proc = dispatch(mode, expected=3)
        assert f"another aoc-dispatch is polling {project}" in proc.stderr

# j: label creation/update is idempotent; real mode rejected before any GitHub call.
reset()
dispatch("labels")
expected_colors = {"agent-ready": "0E8A16", "agent-running": "1D76DB", "agent-review": "5319E7",
                   "needs-alex": "D93F0B", "risk-review": "FBCA04", "agent-failed": "B60205"}
assert {name: label["color"] for name, label in github()["labels"].items()} == expected_colors
dispatch("labels")
assert len(github()["labels"]) == 6
before = github()
proc = dispatch("tick", "--mode", "real", expected=2)
assert "only dry-run is implemented" in proc.stderr and github() == before

# Sort ascending, skip conflicting candidates, and launch at most one master.
reset([(4, ["agent-ready"]), (2, ["agent-ready", "risk-review"]), (3, ["agent-ready"])])
proc = dispatch("tick")
assert "conflicting dispatch label" in proc.stderr
assert len(calls()) == 1 and calls()[0]["issue"] == "3"
assert issue_labels(4) == {"agent-ready"} and issue_labels(3) == {"agent-review"}

# A claim that cannot be confirmed rolls back locally and never launches.
reset()
environment["FAKE_GH_CLAIM_NOOP"] = "1"
proc = dispatch("tick")
assert "claim was not confirmed" in proc.stderr
assert local()["seat"] == "IDLE" and local()["active"] is None and not local()["issues"]
assert not calls() and issue_labels() == {"agent-ready"}

# A nonzero exit does not discard a valid structured report.
reset()
environment["FAKE_CLAUDE_EXIT"] = "7"
dispatch("tick")
assert result()["outcome"] == "review"

# Restart recovery uses a surviving report for both CLAIMED and RUNNING seats.
for seat in ("CLAIMED", "RUNNING"):
    human("--remove-label", "agent-review", "--add-label", "agent-running")
    dead = subprocess.Popen([sys.executable, "-c", "pass"])
    dead.wait()
    state = local()
    run_id = state["issues"]["2"]["last_run_id"]
    state.update(seat=seat, active={"run_id": run_id, "issue": 2, "pid": dead.pid})
    state["issues"]["2"]["state"] = "running"
    (state_dir / "state.json").write_text(json.dumps(state))
    dispatch("tick")
    assert local()["seat"] == "IDLE" and local()["active"] is None
    assert result()["outcome"] == "review" and issue_labels() == {"agent-review"}
    assert len(calls()) == 1

print("AOC Dispatch smoke passed (a-j)")
PY
