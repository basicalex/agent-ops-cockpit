#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
cleanup() {
  python3 - "$tmp_dir" <<'PY'
import os
import signal
import sys
from pathlib import Path
path = Path(sys.argv[1]) / "processes"
if path.exists():
    for value in path.read_text().splitlines():
        try:
            os.killpg(int(value), signal.SIGKILL)
        except ProcessLookupError:
            pass
PY
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

python3 - "$root/bin/aoc-dispatch" "$tmp_dir" <<'PY'
import fcntl
import atexit
import hashlib
import json
import os
import re
import runpy
import shutil
import signal
import subprocess
import sys
import time
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
processes = temporary / "processes"
background = []


def stop_background():
    for proc in background:
        if proc.poll() is None:
            proc.kill()
        proc.wait()


atexit.register(stop_background)
claude_calls = temporary / "claude.jsonl"
prism_calls = temporary / "prism.jsonl"
state_dir = temporary / "state/aoc/dispatch" / hashlib.sha256(str(project.resolve()).encode()).hexdigest()[:16]
environment = dict(os.environ, PATH=f"{fake_bin}:{os.environ['PATH']}", HOME=str(home),
                   XDG_STATE_HOME=str(temporary / "state"), FAKE_GH_STATE=str(state_file),
                   FAKE_CLAUDE_CALLS=str(claude_calls), FAKE_GH_ACTOR="aoc-bot",
                   AOC_DISPATCH_GH_BIN=str(fake_bin / "gh"), AOC_DISPATCH_CLAUDE_BIN=str(fake_bin / "claude"),
                   AOC_DISPATCH_PRISM_BIN=str(fake_bin / "aoc-prism"),
                   FAKE_PRISM_CALLS=str(prism_calls), FAKE_PRISM_ROOT=str(project),
                   AOC_DISPATCH_NOW="2026-10-01T12:00:00Z", FAKE_CLAUDE_MODE="done",
                   FAKE_DISPATCH_STATE=str(state_dir / "state.json"), FAKE_PROCESSES=str(processes))

(fake_bin / "gh").write_text(r'''#!/usr/bin/env python3
import datetime as dt
import hashlib
import json
import os
import sys
import time
from pathlib import Path

path = Path(os.environ["FAKE_GH_STATE"])
state = json.loads(path.read_text())
argv = sys.argv[1:]
actor = os.environ.get("FAKE_GH_ACTOR", "aoc-bot")
created = os.environ.get("AOC_DISPATCH_NOW") or dt.datetime.now(dt.timezone.utc).isoformat().replace("+00:00", "Z")
entry = {"argv": argv, "actor": actor, "created_at": created}
state["log"].append(entry)
repo = state["repo"]
output = None
health_control = os.environ.get("FAKE_GH_HEALTH_CONTROL")
if health_control and argv[:2] == ["api", "-i"]:
    control = Path(health_control)
    attempts = control / "attempts"
    attempt = int(attempts.read_text()) + 1 if attempts.exists() else 1
    pending_attempts = control / "attempts.tmp"
    pending_attempts.write_text(str(attempt))
    os.replace(pending_attempts, attempts)
    if attempt in (5, 6):
        while not (control / f"release-{attempt}").exists():
            time.sleep(0.01)
    if attempt <= 6:
        save_pending = path.with_suffix(f".{os.getpid()}.tmp")
        save_pending.write_text(json.dumps(state))
        os.replace(save_pending, path)
        print("fixture outage " + "x" * 350, file=sys.stderr)
        raise SystemExit(1)
    if attempt == 7:
        while not (control / "release").exists():
            time.sleep(0.01)

def save():
    temporary = path.with_suffix(f".{os.getpid()}.tmp")
    temporary.write_text(json.dumps(state))
    os.replace(temporary, path)

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
    output = f"https://github.com/{repo}/issues/{argv[2]}#issuecomment-{len(state['issues'][argv[2]]['comments'])}"
    state["issues"][argv[2]]["comments"][-1]["url"] = output
    if os.environ.get("FAKE_GH_COMMENT_OUTPUT") == "empty":
        output = None
    elif os.environ.get("FAKE_GH_COMMENT_OUTPUT") == "non-url":
        output = "Comment posted\nnot a URL\n\n"
    else:
        output = "Comment posted\n\n" + output + "\n\n"
elif argv[:1] == ["api"]:
    if argv[1:2] == ["-i"]:
        assert len(argv) in (3, 5), argv
        assert argv[-1] == f"repos/{repo}/issues?labels=agent-ready&state=open&per_page=50", argv
        etag = os.environ.get("FAKE_GH_ETAG") or '"' + hashlib.sha256(
            json.dumps([state["issues"], state["events"]], sort_keys=True).encode()).hexdigest() + '"'
        conditional = len(argv) == 5
        if conditional:
            assert argv[2] == "-H" and argv[3].startswith("If-None-Match: "), argv
        unchanged = (conditional and argv[3] == "If-None-Match: " + etag) or os.environ.get("FAKE_GH_FORCE_304") == "1"
        save()
        if unchanged:
            print(f"HTTP/2.0 304 Not Modified\nETag: {etag}\n\n", end="")
            raise SystemExit(int(os.environ.get("FAKE_GH_304_EXIT", "1")))
        output = [issue for issue in state["issues"].values()
                  if issue["state"] == "open" and {"name": "agent-ready"} in issue["labels"]]
        print(f"HTTP/2.0 200 OK\nETag: {etag}\n\n" + json.dumps(output))
        raise SystemExit(0)
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
save()
if output is not None:
    print(output if isinstance(output, str) else json.dumps(output))
''')
(fake_bin / "claude").write_text(r'''#!/usr/bin/env python3
import datetime as dt
import json
import os
import signal
import sys
import time
from pathlib import Path

run_dir = Path(os.environ["AOC_DISPATCH_RUN_DIR"])
meta = json.loads((run_dir / "meta.json").read_text())
with Path(os.environ["FAKE_CLAUDE_CALLS"]).open("a") as handle:
    with Path(os.environ["FAKE_PROCESSES"]).open("a") as registry:
        registry.write(str(os.getpid()) + "\n")
    handle.write(json.dumps({"argv": sys.argv[1:], "run_dir": str(run_dir), "cwd": os.getcwd(),
                             "run_id": os.environ["AOC_DISPATCH_RUN_ID"], "repo": os.environ["AOC_DISPATCH_REPO"],
                             "issue": os.environ["AOC_DISPATCH_ISSUE"]}) + "\n")
mode = os.environ["FAKE_CLAUDE_MODE"]
print("fake master output", flush=True)
print(json.dumps({"type": "assistant", "message": {"content": [
    {"type": "tool_use", "name": "Read", "input": {"file_path": "packet.md"}},
    {"type": "text", "text": "Read issue and plan changes"}]}}), flush=True)
print(json.dumps({"type": "result", "subtype": "success"}), flush=True)
if mode in ("sleep", "stubborn"):
    if mode == "stubborn":
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
    time.sleep(60)
    raise SystemExit(0)
if mode == "delayed":
    while not (run_dir / "release").exists():
        time.sleep(0.03)
if mode == "noreport":
    raise SystemExit(int(os.environ.get("FAKE_CLAUDE_EXIT", "0")))
status = mode if mode in ("done", "blocked", "failed") else "done"
report = {"schema": "aoc.dispatch.report/v1", "version": 1, "run_id": meta["run_id"],
          "assignmentId": meta["run_id"], "repo": meta["repo"], "issue": meta["issue"], "status": status,
          "summary": "Plan complete" if status == "done" else "Need decision" if status == "blocked" else "Master failed",
          "needsDecision": "Use option A or B?" if status == "blocked" else None,
          "decision_id": meta["run_id"] + "-d1" if status == "blocked" else None,
          "evidence": "Read issue and repository", "timestamp": os.environ.get("AOC_DISPATCH_NOW") or dt.datetime.now(dt.timezone.utc).isoformat()}
if mode == "invalid":
    report["run_id"] = "wrong-run"
(run_dir / "report.json").write_text(json.dumps(report))
raise SystemExit(int(os.environ.get("FAKE_CLAUDE_EXIT", "0")))
''')
(fake_bin / "aoc-prism").write_text(r'''#!/usr/bin/env python3
import json
import os
import sys
import time
from pathlib import Path

assert os.getcwd() == os.environ["FAKE_PRISM_ROOT"], os.getcwd()
with Path(os.environ["FAKE_PRISM_CALLS"]).open("a") as handle:
    handle.write(json.dumps(sys.argv[1:]) + "\n")
time.sleep(float(os.environ.get("FAKE_PRISM_SLEEP", "0")))
print("fixture Prism stdout")
print("fixture Prism stderr", file=sys.stderr)
raise SystemExit(int(os.environ.get("FAKE_PRISM_EXIT", "0")))
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



def activities():
    rows = [json.loads(line) for line in prism_calls.read_text().splitlines()] if prism_calls.exists() else []
    result = []
    for argv in rows:
        assert argv[0] == "activity" and len(argv) == 13, argv
        flags = dict(zip(argv[1::2], argv[2::2]))
        assert set(flags) == {"--agent", "--status", "--summary", "--repo", "--issue-url", "--task-ref"}, argv
        assert flags["--repo"] == "fixture/dispatch"
        result.append({key[2:]: value for key, value in flags.items()})
    return result


def comment_url(event):
    return next(comment["url"] for comment in reversed(github()["issues"]["2"]["comments"])
                if f"event={event} -->" in comment["body"])

def reset(issues=None, actor="basicalex"):
    shutil.rmtree(state_dir, ignore_errors=True)
    claude_calls.unlink(missing_ok=True)
    prism_calls.unlink(missing_ok=True)
    environment.update(AOC_DISPATCH_NOW="2026-10-01T12:00:00Z", FAKE_CLAUDE_MODE="done")
    environment["AOC_DISPATCH_PRISM_BIN"] = str(fake_bin / "aoc-prism")
    for name in ("FAKE_GH_CLAIM_NOOP", "FAKE_CLAUDE_EXIT", "FAKE_GH_ETAG",
                 "FAKE_GH_FORCE_304", "FAKE_GH_304_EXIT", "FAKE_GH_COMMENT_OUTPUT",
                 "FAKE_PRISM_EXIT", "FAKE_PRISM_SLEEP", "AOC_DISPATCH_PRISM_TIMEOUT",
                 "FAKE_GH_HEALTH_CONTROL"):
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


def dispatch(*args, expected=0, defaults=True, wait=True):
    argv = [str(command), "--root", str(project)]
    if defaults:
        argv += ["--repo", "fixture/dispatch"]
    if args[:1] == ("tick",) and wait:
        args += ("--wait",)
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
dispatch("tick", wait=False)
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
assert not activities()
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

# k: unchanged inbox skips processing, changed ETag processes, 300s forces shape 2.
reset(actor="stranger")
environment["FAKE_GH_ETAG"] = '"v1"'
dispatch("tick")
assert local()["inbox_etag"] == '"v1"'
start = len(github()["log"])
dispatch("tick")
assert [entry["argv"][:2] for entry in github()["log"][start:]] == [["api", "-i"]]
assert github()["log"][-1]["argv"][2:4] == ["-H", 'If-None-Match: "v1"']
environment["FAKE_GH_304_EXIT"] = "0"
dispatch("tick")
environment["FAKE_GH_ETAG"] = '"v2"'
start = len(github()["log"])
dispatch("tick")
assert local()["inbox_etag"] == '"v2"'
assert any(entry["argv"][:2] == ["issue", "list"] for entry in github()["log"][start:])
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:04:59Z"
start = len(github()["log"])
dispatch("tick")
assert not any(entry["argv"][:2] == ["issue", "list"] for entry in github()["log"][start:])
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:05:00Z"
start = len(github()["log"])
dispatch("tick")
assert any(entry["argv"][:2] == ["issue", "list"] for entry in github()["log"][start:])
assert local()["last_full_check_at"] == "2026-10-01T12:05:00Z"

# l: pending resume is processed even on 304 and still rolls back at timeout.
original, decision_id = blocked()
environment.update(AOC_DISPATCH_NOW="2026-10-01T12:01:00Z", FAKE_CLAUDE_MODE="done")
human("--remove-label", "needs-alex")
human("--add-label", "agent-ready")
dispatch("tick", "--decision-timeout", "1")
assert local()["issues"]["2"]["ready_seen_at"] is not None
environment.update(FAKE_GH_FORCE_304="1", AOC_DISPATCH_NOW="2026-10-01T12:01:02Z")
start = len(github()["log"])
dispatch("tick", "--decision-timeout", "1")
assert github()["log"][start]["argv"][:2] == ["api", "-i"]
assert issue_labels() == {"needs-alex"} and local()["issues"]["2"]["ready_seen_at"] is None
assert comments("resume-timeout") and len(calls()) == 1


def until(check, timeout=5):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = check()
        if value:
            return value
        time.sleep(0.03)
    raise AssertionError("timed out waiting for fixture state")


def pid_running(pid):
    proc = subprocess.run(["ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True)
    return proc.returncode == 0 and not proc.stdout.strip().startswith("Z")


def stop_master(pid):
    try:
        os.killpg(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    until(lambda: not pid_running(pid))


# m: a one-pass tick returns before a held master; a later tick finalizes it.
reset()
environment["FAKE_CLAUDE_MODE"] = "sleep"
started = time.monotonic()
dispatch("tick", wait=False)
assert time.monotonic() - started < 3
active = local()["active"]
assert local()["seat"] == "RUNNING" and pid_running(active["pid"])
until(lambda: len(calls()) == 1)
dispatch("tick", wait=False)
assert local()["active"] == active and issue_labels() == {"agent-running"}
stop_master(active["pid"])
dispatch("tick", wait=False)
assert result()["outcome"] == "failed" and local()["seat"] == "IDLE"

# Timeout comparison survives dispatcher restart; timeout overrides even a report.
reset()
environment["FAKE_CLAUDE_MODE"] = "sleep"
dispatch("tick", "--timeout", "1", wait=False)
active = local()["active"]
until(lambda: len(calls()) == 1)
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:00:02Z"
dispatch("tick", "--timeout", "1", wait=False)
until(lambda: not pid_running(active["pid"]))
dispatch("tick", "--timeout", "1", wait=False)
assert result()["reason"] == "timeout" and issue_labels() == {"agent-failed"}

# A master ignoring SIGTERM is killed after the ten-second grace period.
reset()
environment["FAKE_CLAUDE_MODE"] = "stubborn"
dispatch("tick", "--timeout", "0.2")
pid = int(processes.read_text().splitlines()[-1])
assert not pid_running(pid) and result()["reason"] == "timeout"
assert issue_labels() == {"agent-failed"}

# n: another dispatcher did not spawn the live PID, waits, then uses its report.
reset()
environment["FAKE_CLAUDE_MODE"] = "delayed"
dispatch("tick", wait=False)
active = local()["active"]
dispatch("tick", wait=False)
assert local()["active"] == active and issue_labels() == {"agent-running"}
(state_dir / "runs" / active["run_id"] / "release").touch()
until(lambda: not pid_running(active["pid"]))
dispatch("tick", wait=False)
assert local()["seat"] == "IDLE" and result()["outcome"] == "review"


def start_seat(root, pane):
    output = temporary / (re.sub(r"[^A-Za-z0-9_.-]", "_", pane) + ".log")
    seat_env = dict(environment, HERDR_PANE_ID=pane)
    seat_env.pop("AOC_DISPATCH_NOW", None)
    proc = subprocess.Popen([str(command), "seat", "--root", str(root)], env=seat_env,
                            stdout=output.open("w"), stderr=subprocess.STDOUT, start_new_session=True)
    background.append(proc)
    with processes.open("a") as registry:
        registry.write(str(proc.pid) + "\n")
    return proc, output


def heartbeat(pane):
    path = temporary / "state/aoc/master/seats" / (re.sub(r"[^A-Za-z0-9_.-]", "_", pane) + ".json")
    return json.loads(path.read_text()) if path.exists() else None


def stop_seat(proc, pane):
    proc.send_signal(signal.SIGTERM)
    assert proc.wait(timeout=5) == 0
    assert heartbeat(pane) is None


# o: non-git root needs no gh call; heartbeat and actual view report no inbox.
non_git = temporary / "non-git"
non_git.mkdir()
before = github()
proc, output = start_seat(non_git, "w1D:no/inbox")
beat = until(lambda: heartbeat("w1D:no/inbox"))
until(lambda: "no inbox configured" in output.read_text())
assert beat["role"] == "no-inbox" and beat["inbox"] is None and beat["seat"] is None
assert beat["root"] == str(non_git) and beat["pid"] == proc.pid
assert beat["schema"] == "aoc.master.seat/v1" and beat["pane_id"] == "w1D:no/inbox"
assert github() == before
stop_seat(proc, "w1D:no/inbox")

# p/q: config inbox/interval, exclusive owner, read-only viewer, lock takeover.
reset([(2, ["risk-review"])])
config_dir = project / ".aoc"
config_dir.mkdir()
config_path = config_dir / "dispatch.toml"
config_path.write_text('inbox = "fixture/dispatch"\ninterval = 0.1\n')
first, first_output = start_seat(project, "w1D:first")
until(lambda: (heartbeat("w1D:first") or {}).get("role") == "owner")
second, second_output = start_seat(project, "w1D:second")
until(lambda: (heartbeat("w1D:second") or {}).get("role") == "viewer")
assert heartbeat("w1D:first")["inbox"] == "fixture/dispatch"
assert not any(entry["argv"][:2] == ["repo", "view"] for entry in github()["log"])
until(lambda: sum(entry["argv"][:2] == ["api", "-i"] for entry in github()["log"]) >= 3)
snapshot = (state_dir / "state.json").read_text()
time.sleep(0.25)
assert (state_dir / "state.json").read_text() == snapshot  # 304 owner + viewer do not rewrite state.
stop_seat(first, "w1D:first")
until(lambda: (heartbeat("w1D:second") or {}).get("role") == "owner")
assert second.poll() is None
config_path.write_text("inbox = [invalid TOML\n")
until(lambda: "Error:" in second_output.read_text())
assert second.poll() is None
config_path.write_text('inbox = "fixture/dispatch"\ninterval = 0.1\n')
start = len(github()["log"])
until(lambda: len(github()["log"]) > start)
stop_seat(second, "w1D:second")

# Config values apply to ticks; explicit flags win.
reset()
config_path.write_text('inbox = "fixture/dispatch"\ninterval = 0.1\nmodel = "configured-model"\ntrusted_actor = "stranger"\n')
dispatch("tick", defaults=False)
assert not calls()
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:05:00Z"
dispatch("tick", "--trusted-actor", "basicalex", "--model", "cli-model", defaults=False)
assert calls()[0]["argv"][calls()[0]["argv"].index("--model") + 1] == "cli-model"
reset()
config_path.write_text('inbox = "fixture/dispatch"\nmodel = "configured-model"\n')
dispatch("tick", defaults=False)
assert calls()[0]["argv"][calls()[0]["argv"].index("--model") + 1] == "configured-model"
config_path.write_text('inbox = "fixture/dispatch"\ninterval = 0.1\n')

# r: a seat exit removes its heartbeat without killing an active master.
reset()
environment["FAKE_CLAUDE_MODE"] = "sleep"
proc, output = start_seat(project, "w1D:survivor")
until(lambda: (heartbeat("w1D:survivor") or {}).get("seat") == "RUNNING")
active = local()["active"]
until(lambda: len(calls()) == 1)
assert pid_running(active["pid"])
until(lambda: "tool Read:" in output.read_text() and "result: success" in output.read_text())
stop_seat(proc, "w1D:survivor")
assert pid_running(active["pid"]) and local()["active"] == active
stop_master(active["pid"])
dispatch("tick", wait=False)
assert result()["outcome"] == "failed"
assert "tool Read:" in comments("failed")[0] and "text: Read issue" in comments("failed")[0]
config_path.unlink()

# s: stream rendering truncates text, handles tool/result and retains raw lines.
module = runpy.run_path(str(command), run_name="dispatch_fixture")
render_path = temporary / "render.log"
render_path.write_text("\n".join([
    json.dumps({"type": "assistant", "message": {"content": [
        {"type": "tool_use", "name": "Read", "input": {"file_path": "packet.md"}},
        {"type": "text", "text": "x" * 200}]}}),
    json.dumps({"type": "result", "subtype": "success"}),
    "plain diagnostic",
]) + "\n")
assert module["render_log_lines"](render_path, 20) == [
    'tool Read: {"file_path": "packet.md"}', "text: " + "x" * 160,
    "result: success", "plain diagnostic"]
assert module["render_log_lines"](render_path, 2) == ["result: success", "plain diagnostic"]
assert module["render_log_lines"](temporary / "missing.log", 20) == []

# t: done lifecycle, stable identity, run prefix and the result comment permalink.
reset()
proc = dispatch("tick")
run_id = local()["issues"]["2"]["last_run_id"]
assert activities() == [
    {"agent": "aoc-master@dispatch", "status": "started",
     "summary": f"run {run_id}: started on #2 Issue 2 (dry-run)", "repo": "fixture/dispatch",
     "issue-url": "https://github.com/fixture/dispatch/issues/2", "task-ref": "fixture/dispatch#2"},
    {"agent": "aoc-master@dispatch", "status": "done",
     "summary": f"run {run_id}: Plan complete", "repo": "fixture/dispatch",
     "issue-url": comment_url("result"), "task-ref": "fixture/dispatch#2"},
]
assert not proc.stdout and "fixture Prism" not in proc.stderr
# Empty or non-URL final stdout lines fall back to the issue, not earlier output.
for output_mode in ("empty", "non-url"):
    reset()
    environment["FAKE_GH_COMMENT_OUTPUT"] = output_mode
    dispatch("tick")
    assert activities()[-1]["issue-url"] == "https://github.com/fixture/dispatch/issues/2"

# u: blocked permalink and continuation resolve using the same agent/task-ref.
original, decision_id = blocked()
assert [entry["status"] for entry in activities()] == ["started", "blocked"]
assert activities()[-1]["issue-url"] == comment_url("blocked")
assert activities()[-1]["summary"] == f"run {original}: needs Alex: Use option A or B?"
environment.update(AOC_DISPATCH_NOW="2026-10-01T12:01:00Z", FAKE_CLAUDE_MODE="done")
answer(f"AOC-DECISION {decision_id}\nUse option A")
human("--remove-label", "needs-alex", "--add-label", "agent-ready")
dispatch("tick")
continued = local()["issues"]["2"]["last_run_id"]
assert [entry["status"] for entry in activities()] == ["started", "blocked", "started", "done"]
assert all(entry["task-ref"] == "fixture/dispatch#2" and entry["agent"] == "aoc-master@dispatch"
           for entry in activities())
assert activities()[2]["summary"] == (
    f"run {continued}: started on #2 Issue 2 (dry-run) resumed after AOC-DECISION {decision_id}"
    f" (continuation of {original})")

# v: a missing report sends failed with its diagnostic comment and exact reason.
reset()
environment["FAKE_CLAUDE_MODE"] = "noreport"
dispatch("tick")
run_id = local()["issues"]["2"]["last_run_id"]
assert [entry["status"] for entry in activities()] == ["started", "failed"]
assert activities()[-1]["issue-url"] == comment_url("failed")
assert activities()[-1]["summary"] == f"run {run_id}: master process ended without a report"
assert activities()[-1]["task-ref"] == "fixture/dispatch#2"

# w: resume timeout reopens the same blocked task at the rollback comment.
original, decision_id = blocked()
environment.update(AOC_DISPATCH_NOW="2026-10-01T12:01:00Z", FAKE_CLAUDE_MODE="done")
human("--remove-label", "needs-alex", "--add-label", "agent-ready")
dispatch("tick", "--decision-timeout", "1")
assert [entry["status"] for entry in activities()] == ["started", "blocked"]
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:01:02Z"
dispatch("tick", "--decision-timeout", "1")
assert [entry["status"] for entry in activities()] == ["started", "blocked", "blocked"]
assert activities()[-1]["issue-url"] == comment_url("resume-timeout")
assert activities()[-1]["summary"] == (
    f"run {original}: no AOC-DECISION {decision_id} found; needs-alex restored")
assert activities()[-1]["task-ref"] == "fixture/dispatch#2"
assert issue_labels() == {"needs-alex"} and local()["issues"]["2"]["ready_seen_at"] is None

# x: subprocess failure or timeout cannot stop claim or finalization.
for fault in ("exit", "timeout", "missing", "invalid-timeout"):
    reset()
    if fault == "exit":
        environment["FAKE_PRISM_EXIT"] = "1"
    elif fault == "timeout":
        environment.update(FAKE_PRISM_SLEEP="2", AOC_DISPATCH_PRISM_TIMEOUT="0.1")
    elif fault == "missing":
        environment["AOC_DISPATCH_PRISM_BIN"] = str(fake_bin / "missing-prism")
    else:
        environment["AOC_DISPATCH_PRISM_TIMEOUT"] = "invalid"
    started = time.monotonic()
    proc = dispatch("tick")
    assert time.monotonic() - started < 5
    assert issue_labels() == {"agent-review"} and result()["outcome"] == "review"
    assert local()["seat"] == "IDLE" and local()["active"] is None
    assert len([line for line in proc.stderr.splitlines() if "Prism activity" in line]) == 2, proc.stderr
    assert "fixture Prism" not in proc.stderr and not proc.stdout
    if fault in ("exit", "timeout"):
        assert [entry["status"] for entry in activities()] == ["started", "done"]

# y: six failed owner ticks send once; the next successful tick resolves once.
reset([(2, ["risk-review"])])
control = temporary / "health-control"
control.mkdir()
environment["FAKE_GH_HEALTH_CONTROL"] = str(control)
config_path.write_text('inbox = "fixture/dispatch"\ninterval = 0.05\n')
proc, output = start_seat(project, "w2:health")
until(lambda: (control / "attempts").exists() and (control / "attempts").read_text() == "5")
assert not activities() and output.read_text().count("aoc-dispatch: tick failed:") == 4
(control / "release-5").touch()
until(lambda: (control / "attempts").read_text() == "6")
assert [entry["status"] for entry in activities()] == ["failed"]
assert output.read_text().count("aoc-dispatch: tick failed:") == 5
(control / "release-6").touch()
until(lambda: (control / "attempts").read_text() == "7")
assert [entry["status"] for entry in activities()] == ["failed"], activities()
failure = activities()[0]
assert failure["agent"] == "aoc-dispatch@dispatch" and failure["task-ref"] == "fixture/dispatch:dispatcher"
assert failure["issue-url"] == "https://github.com/fixture/dispatch"
assert failure["summary"] == (
    f"dispatcher for {project} failing: " + ("gh inbox HTTP None: fixture outage " + "x" * 350)[:300])
assert output.read_text().count("aoc-dispatch: tick failed:") == 6
(control / "release").touch()
until(lambda: len(activities()) == 2)
assert activities()[1] == dict(failure, status="done", summary=f"dispatcher for {project} recovered")
until(lambda: int((control / "attempts").read_text()) >= 9)
assert [entry["status"] for entry in activities()] == ["failed", "done"]
stop_seat(proc, "w2:health")
config_path.unlink()

print("AOC Dispatch smoke passed (a-y)")
PY
