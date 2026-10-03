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
herdr_state = temporary / "herdr.json"
state_dir = temporary / "state/aoc/dispatch" / hashlib.sha256(str(project.resolve()).encode()).hexdigest()[:16]
environment = dict(os.environ, PATH=f"{fake_bin}:{os.environ['PATH']}", HOME=str(home),
                   XDG_STATE_HOME=str(temporary / "state"), FAKE_GH_STATE=str(state_file),
                   FAKE_CLAUDE_CALLS=str(claude_calls), FAKE_GH_ACTOR="aoc-bot",
                   AOC_DISPATCH_GH_BIN=str(fake_bin / "gh"), AOC_DISPATCH_CLAUDE_BIN=str(fake_bin / "claude"),
                   AOC_DISPATCH_PRISM_BIN=str(fake_bin / "aoc-prism"),
                   FAKE_PRISM_CALLS=str(prism_calls), FAKE_PRISM_ROOT=str(project),
                   AOC_DISPATCH_NOW="2026-10-01T12:00:00Z", FAKE_CLAUDE_MODE="done",
                   FAKE_DISPATCH_STATE=str(state_dir / "state.json"), FAKE_PROCESSES=str(processes),
                   AOC_HERDR_BIN=str(fake_bin / "herdr"), FAKE_HERDR_STATE=str(herdr_state),
                   FAKE_WORKER_WAIT=str(command.parent / "aoc-worker-wait"))
environment = {key: value for key, value in environment.items() if not key.startswith("HERDR_")}
environment["AOC_DISPATCH_HERDR_BIN"] = str(fake_bin / "herdr")
herdr_text = temporary / "herdr-text.txt"
environment["FAKE_HERDR_TEXT"] = str(herdr_text)

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
elif argv == ["repo", "view", repo, "--json", "defaultBranchRef", "-q", ".defaultBranchRef.name"]:
    output = "main"
elif argv[:2] == ["pr", "list"]:
    assert argv[2:5] == ["--repo", repo, "--head"] and argv[6:] == ["--state", "open", "--json", "number,url"], argv
    output = [pr for pr in state.get("prs", []) if pr["head"] == argv[5]]
elif argv[:2] == ["pr", "create"]:
    assert len(argv) == 12 and argv[2:5] == ["--repo", repo, "--head"], argv
    assert argv[6] == "--base" and argv[8] == "--title" and argv[10] == "--body-file", argv
    entry["body"] = Path(argv[11]).read_text()
    if os.environ.get("FAKE_GH_PR_FAIL") == "1":
        save()
        print("fixture PR creation failed", file=sys.stderr)
        raise SystemExit(1)
    url = f"https://github.com/{repo}/pull/{100 + len(state.get('prs', []))}"
    state.setdefault("prs", []).append({"number": 100, "url": url, "head": argv[5]})
    output = "Created pull request\n" + url + "\n\n"
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
    missing = []
    for index in range(5, len(argv), 2):
        assert argv[index] in ("--add-label", "--remove-label"), argv
        if os.environ.get("FAKE_GH_CLAIM_NOOP") != "1":
            action, label = argv[index:index + 2]
            if (action == "--add-label" and os.environ.get("FAKE_GH_ENFORCE_LABELS") == "1"
                    and label not in state["labels"]):
                missing.append(label)
            else:
                change(issue, action, label)
    if missing:
        save()
        print("label not found: " + ", ".join(missing), file=sys.stderr)
        raise SystemExit(1)
elif argv[:2] == ["issue", "comment"]:
    assert argv[3:6] == ["--repo", repo, "--body-file"] and len(argv) == 7, argv
    body = Path(argv[6]).read_text()
    fail_event = os.environ.get("FAKE_GH_FAIL_COMMENT_EVENT")
    if fail_event and f"event={fail_event} -->" in body:
        save()
        print("fixture comment failed", file=sys.stderr)
        raise SystemExit(1)
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
    if os.environ.get("FAKE_GH_FAIL_LABEL") == argv[2]:
        save()
        print("fixture label creation failed", file=sys.stderr)
        raise SystemExit(1)
    state["labels"][argv[2]] = {"color": argv[6], "description": argv[8]}
else:
    raise AssertionError("unsupported fake gh command: " + repr(argv))
save()
if output is not None:
    print(output if isinstance(output, str) else json.dumps(output))
''')
(fake_bin / "herdr").write_text(r'''#!/usr/bin/env python3
import fcntl
import json
import os
import sys
import subprocess
from pathlib import Path

path = Path(os.environ["FAKE_HERDR_STATE"])
args = sys.argv[1:]
with path.with_suffix(".lock").open("w") as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    state = json.loads(path.read_text())
    state.setdefault("calls", []).append(args)
    if os.environ.get("FAKE_HERDR_FAIL") == " ".join(args[:2]):
        pending = path.with_suffix(".tmp")
        pending.write_text(json.dumps(state))
        os.replace(pending, path)
        raise SystemExit("fixture herdr failure")
    if args[:2] == ["tab", "create"]:
        label = args[args.index("--label") + 1]
        number = len(state["tabs"]) + 1
        tab = {"tab_id": f"fixture:t{number}", "label": label,
               "cwd": args[args.index("--cwd") + 1]}
        pane = {"pane_id": f"fixture:p{number}", "tab_id": tab["tab_id"], "agent_status": "working"}
        state["tabs"].append(tab)
        state["panes"].append(pane)
        result = {"tab": tab, "root_pane": pane}
    elif args[:2] == ["pane", "run"]:
        state["runs"].append({"pane_id": args[2], "command": args[3]})
        if args[3].startswith("bash "):
            child = subprocess.Popen(["bash", "-c", args[3]], env=os.environ,
                                     start_new_session=True, stdout=subprocess.DEVNULL,
                                     stderr=subprocess.DEVNULL)
            with Path(os.environ["FAKE_PROCESSES"]).open("a") as registry:
                registry.write(str(child.pid) + "\n")
        result = {}
    elif args == ["pane", "list"]:
        result = {"panes": state["panes"]}
    elif args[:2] == ["tab", "list"]:
        result = {"tabs": state["tabs"]}
    elif args[:2] == ["pane", "read"]:
        result = {}
    elif args[:2] in (["pane", "send-keys"], ["pane", "send-text"]):
        if args[2:] and args[-1] == "Enter":
            Path(os.environ["FAKE_HERDR_TEXT"]).write_text("")
        result = {}
    else:
        raise SystemExit("unexpected fixture herdr command: " + repr(args))
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(state))
    os.replace(temporary, path)
if args[:2] == ["pane", "read"]:
    print(Path(os.environ["FAKE_HERDR_TEXT"]).read_text())
else:
    print(json.dumps({"result": result}))
''')
(fake_bin / "claude").write_text(r'''#!/usr/bin/env python3
import datetime as dt
import json
import os
import signal
import sys
import subprocess
import time
from pathlib import Path

run_dir = Path(os.environ["AOC_DISPATCH_RUN_DIR"])
meta = json.loads((run_dir / "meta.json").read_text())
with Path(os.environ["FAKE_CLAUDE_CALLS"]).open("a") as handle:
    with Path(os.environ["FAKE_PROCESSES"]).open("a") as registry:
        registry.write(str(os.getpid()) + "\n")
    handle.write(json.dumps({"argv": sys.argv[1:], "run_dir": str(run_dir), "cwd": os.getcwd(),
                             "run_id": os.environ["AOC_DISPATCH_RUN_ID"], "repo": os.environ["AOC_DISPATCH_REPO"],
                             "issue": os.environ["AOC_DISPATCH_ISSUE"],
                             "env": {key: value for key, value in os.environ.items()
                                     if key.startswith("GIT_CONFIG_") or key in (
                                         "GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN",
                                         "GITHUB_ENTERPRISE_TOKEN", "GH_CONFIG_DIR",
                                         "GIT_TERMINAL_PROMPT", "AOC_DISPATCH_WORKTREE")}}) + "\n")
if os.environ.get("FAKE_CLAUDE_NO_SESSION") != "1":
    print(json.dumps({"type": "system", "subtype": "init", "session_id": "fixture-session"}), flush=True)
mode = os.environ["FAKE_CLAUDE_MODE"]
print("fake master output: " + ("resumed" if "--resume" in sys.argv else "initial"), flush=True)
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
worker_output = None
if mode in ("delegate", "delegate-lost"):
    worker_files = [run_dir / "workers" / f"issue-2-w{k}.md" for k in (1, 2)]
    panes = ["fixture:p1", "fixture:p2"]
    if "--resume" not in sys.argv:
        for k in (1, 2):
            created = subprocess.run([os.environ["AOC_HERDR_BIN"], "tab", "create",
                                     "--workspace", "fixture-workspace", "--cwd", os.getcwd(),
                                     "--label", f"issue-2-w{k}", "--no-focus"],
                                    check=True, capture_output=True, text=True)
            pane = json.loads(created.stdout)["result"]["root_pane"]["pane_id"]
            assert pane == panes[k - 1]
            subprocess.run([os.environ["AOC_HERDR_BIN"], "pane", "run", pane,
                            f"aoc-omp --prompt-file issue-2-w{k}.txt"], check=True, capture_output=True)
        worker_code = r"""
import fcntl
import json
import os
import sys
import time
from pathlib import Path

path = Path(os.environ["FAKE_HERDR_STATE"])
pane, result_file, lost = sys.argv[1:]
def update(status):
    with path.with_suffix(".lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        state = json.loads(path.read_text())
        if status is None:
            state["panes"] = [item for item in state["panes"] if item["pane_id"] != pane]
        else:
            next(item for item in state["panes"] if item["pane_id"] == pane)["agent_status"] = status
        temporary = path.with_suffix(f".{os.getpid()}.tmp")
        temporary.write_text(json.dumps(state))
        os.replace(temporary, path)
update("working")
time.sleep(0.1)
if lost == "1":
    update(None)
    raise SystemExit(0)
update("idle")
time.sleep(0.1)
update("working")
time.sleep(1)
Path(result_file).write_text(f"{pane}: fixture worker complete\n")
update("idle")
"""
        for k, (pane, result_file) in enumerate(zip(panes, worker_files), 1):
            child = subprocess.Popen([sys.executable, "-c", worker_code, pane, str(result_file),
                                      "1" if mode == "delegate-lost" and k == 2 else "0"],
                                     start_new_session=True, stdout=subprocess.DEVNULL,
                                     stderr=subprocess.DEVNULL)
            with Path(os.environ["FAKE_PROCESSES"]).open("a") as registry:
                registry.write(str(child.pid) + "\n")
        if mode == "delegate":
            raise SystemExit(0)
    waiter = [os.environ["FAKE_WORKER_WAIT"]]
    for pane, result_file in zip(panes, worker_files):
        waiter += ["--worker", f"{pane}={result_file}"]
    waiter += ["--interval", "0.05", "--stall", "3", "--timeout", "20"]
    waited = subprocess.run(waiter, capture_output=True, text=True)
    assert waited.returncode == (1 if mode == "delegate-lost" else 0), (waited.stdout, waited.stderr)
    worker_output = waited.stdout
    print(worker_output, flush=True)
status = mode if mode in ("done", "blocked", "failed") else "done"
report = {"schema": "aoc.dispatch.report/v1", "version": 1, "run_id": meta["run_id"],
          "assignmentId": meta["run_id"], "repo": meta["repo"], "issue": meta["issue"], "status": status,
          "summary": "Plan complete" if status == "done" else "Need decision" if status == "blocked" else "Master failed",
          "needsDecision": "Use option A or B?" if status == "blocked" else None,
          "decision_id": meta["run_id"] + "-d1" if status == "blocked" else None,
          "evidence": "Read issue and repository", "timestamp": os.environ.get("AOC_DISPATCH_NOW") or dt.datetime.now(dt.timezone.utc).isoformat()}
if mode == "delegate":
    report.update(summary="Delegated workers integrated", tests="aoc-worker-wait: 2 done; fixture check: passed")
elif mode == "delegate-lost":
    report.update(status="failed", summary="Delegated worker fixture:p2 missing", evidence=worker_output)
if meta["mode"] == "code" and mode != "delegate-lost":
    def git(*args):
        return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout.strip()
    behaviour = os.environ.get("FAKE_CODE_BEHAVIOUR", "commit")
    if behaviour == "wrong-branch":
        git("checkout", "-b", "wrong-branch")
    elif behaviour == "unrelated":
        git("checkout", "--orphan", meta["code"]["branch"] + "-unrelated")
        git("rm", "-rf", ".")
    if behaviour != "no-commits":
        with Path("change.txt").open("a") as changed:
            changed.write(meta["run_id"] + "\n")
            if mode == "delegate":
                for result_file in worker_files:
                    changed.write(result_file.read_text())
        git("add", "change.txt")
        git("commit", "-m", "Implement fixture issue")
    if behaviour == "unrelated":
        git("branch", "-M", meta["code"]["branch"])
    elif behaviour == "dirty":
        Path("change.txt").write_text("uncommitted change\n")
    elif behaviour == "untracked":
        Path("untracked.txt").write_text("untracked change\n")
    report.update(branch=git("rev-parse", "--abbrev-ref", "HEAD"),
                  commit=git("rev-parse", "HEAD"), tests=report.get("tests", "fixture check: passed"))
    if behaviour == "wrong-commit":
        report["commit"] = meta["code"]["base_sha"]
    elif behaviour == "wrong-report-branch":
        report["branch"] = "aoc/issue-2-wrong"
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
    herdr_state.write_text(json.dumps({"tabs": [], "panes": [], "runs": []}))
    herdr_text.write_text("Yes, I trust this folder\n❯ No, exit\n")
    for name in list(environment):
        if name.startswith("HERDR_"):
            environment.pop(name)
    environment.pop("FAKE_HERDR_FAIL", None)
    environment["AOC_DISPATCH_PRISM_BIN"] = str(fake_bin / "aoc-prism")
    for name in ("FAKE_GH_CLAIM_NOOP", "FAKE_CLAUDE_EXIT", "FAKE_GH_ETAG",
                 "FAKE_GH_FORCE_304", "FAKE_GH_304_EXIT", "FAKE_GH_COMMENT_OUTPUT",
                 "FAKE_PRISM_EXIT", "FAKE_PRISM_SLEEP", "AOC_DISPATCH_PRISM_TIMEOUT",
                 "FAKE_GH_HEALTH_CONTROL", "FAKE_GH_ENFORCE_LABELS", "FAKE_GH_FAIL_LABEL",
                 "FAKE_GH_FAIL_COMMENT_EVENT", "FAKE_CLAUDE_NO_SESSION"):
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
assert "Master lifecycle" not in packet
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

# j: label creation/update is idempotent; invalid mode rejected before any GitHub call.
reset()
dispatch("labels")
expected_colors = {"agent-ready": "0E8A16", "agent-running": "1D76DB", "agent-review": "5319E7",
                   "needs-alex": "D93F0B", "risk-review": "FBCA04", "agent-failed": "B60205"}
assert {name: label["color"] for name, label in github()["labels"].items()} == expected_colors
dispatch("labels")
assert len(github()["labels"]) == 6
before = github()
proc = dispatch("tick", "--mode", "real", expected=2)
assert "mode must be dry-run or code" in proc.stderr and github() == before

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


def start_seat(root, pane, command_name="seat", frozen_clock=False):
    output = temporary / (re.sub(r"[^A-Za-z0-9_.-]", "_", pane) + ".log")
    seat_env = dict(environment, HERDR_PANE_ID=pane)
    if not frozen_clock:
        seat_env.pop("AOC_DISPATCH_NOW", None)
    proc = subprocess.Popen([str(command), command_name, "--root", str(root)], env=seat_env,
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

# z: isolated real Git repositories; only gh, claude, Prism and handshake are fixtures.
real_git = shutil.which("git")
environment.update(GIT_AUTHOR_NAME="Dispatch Fixture", GIT_AUTHOR_EMAIL="fixture@example.invalid",
                   GIT_COMMITTER_NAME="Dispatch Fixture", GIT_COMMITTER_EMAIL="fixture@example.invalid",
                   GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull, GIT_TERMINAL_PROMPT="0")
environment.pop("FAKE_GH_HEALTH_CONTROL", None)
dry_project, dry_state_dir = project, state_dir
git_trace = temporary / "git-trace.log"
environment["GIT_TRACE"] = str(git_trace)


def git_at(cwd, *args, expected=0):
    proc = subprocess.run([real_git, *map(str, args)], cwd=cwd, env=environment,
                          capture_output=True, text=True, timeout=10)
    assert proc.returncode == expected, (cwd, args, proc.stdout, proc.stderr)
    return proc.stdout.strip()


def code_fixture(name, extra_config=""):
    global project, state_dir, bare, base_sha
    directory = temporary / name
    directory.mkdir()
    bare = directory / "origin.git"
    seed = directory / "seed"
    git_at(directory, "init", "--bare", bare)
    git_at(directory, "init", "-b", "main", seed)
    (seed / "initial.txt").write_text("base\n")
    git_at(seed, "add", "initial.txt")
    git_at(seed, "commit", "-m", "Initial fixture")
    base_sha = git_at(seed, "rev-parse", "HEAD")
    git_at(seed, "push", bare, "main:refs/heads/main")
    git_at(directory, "--git-dir", bare, "symbolic-ref", "HEAD", "refs/heads/main")
    project = directory / "root"
    git_at(directory, "clone", bare, project)
    (project / ".aoc").mkdir()
    (project / ".aoc/dispatch.toml").write_text(
        f'inbox = "fixture/dispatch"\nmode = "code"\npush_url = {json.dumps(str(bare))}\n' + extra_config)
    state_dir = temporary / "state/aoc/dispatch" / hashlib.sha256(str(project.resolve()).encode()).hexdigest()[:16]
    environment.update(FAKE_DISPATCH_STATE=str(state_dir / "state.json"), FAKE_PRISM_ROOT=str(project))
    reset()
    environment.pop("FAKE_CODE_BEHAVIOUR", None)
    environment.pop("FAKE_GH_PR_FAIL", None)
    git_trace.write_text("")
    return directory


def code_record():
    return local()["issues"]["2"]["code"]


def assert_no_publication():
    assert git_at(project, "ls-remote", "--heads", bare) == f"{base_sha}\trefs/heads/main"
    assert not any(entry["argv"][0] == "pr" for entry in github()["log"])
    assert "built-in: git push" not in git_trace.read_text()


# z1: permissions/guards, worktree base, PR content and explicit safe push.
code_fixture("z1")
title = "  Fix: Workflow / files!!!  "
state = github()
state["issues"]["2"]["title"] = title
state_file.write_text(json.dumps(state))
environment.update(GH_TOKEN="fixture-token", GITHUB_TOKEN="fixture-token",
                   GH_ENTERPRISE_TOKEN="fixture-token", GITHUB_ENTERPRISE_TOKEN="fixture-token",
                   GIT_CONFIG_COUNT="1",
                   GIT_CONFIG_KEY_0="color.ui", GIT_CONFIG_VALUE_0="false")
dispatch("tick")
code = code_record()
assert code == {"default_branch": "main", "base_sha": base_sha,
                "branch": "aoc/issue-2-fix-workflow-files", "worktree": str(state_dir / "worktrees/issue-2")}
launch = calls()[0]
argv, env = launch["argv"], launch["env"]
assert launch["cwd"] == code["worktree"] and env["AOC_DISPATCH_WORKTREE"] == code["worktree"]
assert "--allowedTools" not in argv and argv[argv.index("--permission-mode") + 1] == "bypassPermissions"
assert "--no-session-persistence" not in argv
assert argv[argv.index("--disallowedTools") + 1:argv.index("--append-system-prompt")] == ["Bash(git push:*)", "Bash(gh:*)"]
assert all(token not in env for token in ("GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"))
assert env["GIT_TERMINAL_PROMPT"] == "0" and env["GIT_CONFIG_COUNT"] == "4"
assert env["GIT_CONFIG_KEY_0"] == "color.ui" and env["GIT_CONFIG_VALUE_0"] == "false"
assert Path(env["GH_CONFIG_DIR"]) == Path(launch["run_dir"]) / "gh-config"
assert list(Path(env["GH_CONFIG_DIR"]).iterdir()) == []
for index, url in enumerate(("https://github.com/", "git@github.com:", "ssh://git@github.com/"), 1):
    assert env[f"GIT_CONFIG_KEY_{index}"] == "url.aoc-push-blocked:///.pushInsteadOf"
    assert env[f"GIT_CONFIG_VALUE_{index}"] == url
    # Read effective push URL; no network request.
    git_at(project, "remote", "add", f"guard-{index}", url + "fixture/dispatch.git")
    guarded = subprocess.run([real_git, "remote", "get-url", "--push", f"guard-{index}"],
                             cwd=code["worktree"], env=dict(environment, **env),
                             capture_output=True, text=True, check=True)
    assert guarded.stdout.strip() == "aoc-push-blocked:///fixture/dispatch.git"
head = git_at(code["worktree"], "rev-parse", "HEAD")
assert git_at(project, "ls-remote", bare, f"refs/heads/{code['branch']}") == f"{head}\trefs/heads/{code['branch']}"
assert git_at(project, "ls-remote", bare, "refs/heads/main") == f"{base_sha}\trefs/heads/main"
assert git_at(code["worktree"], "rev-parse", "HEAD^") == base_sha
assert git_at(code["worktree"], "status", "--porcelain") == ""
pr = next(entry for entry in github()["log"] if entry["argv"][:2] == ["pr", "create"])
assert pr["argv"][5] == code["branch"] and pr["argv"][7] == "main" and pr["argv"][9] == title + " (#2)"
run_id = local()["issues"]["2"]["last_run_id"]
assert f"<!-- aoc-dispatch run_id={run_id} event=pr -->" in pr["body"]
assert "Closes #2" in pr["body"] and "## Tests\n\nfixture check: passed" in pr["body"]
assert f"Commits: 1\nHEAD: {head}" in pr["body"]
assert result()["pr_url"] == github()["prs"][0]["url"] and result()["head"] == head
assert result()["branch"] == code["branch"] and issue_labels() == {"agent-review"}
assert result()["pr_url"] in comments("result")[0] and head in comments("result")[0]
assert "fixture check: passed" in comments("result")[0]
assert comments("claim")[0].endswith(
    f"Claimed for code mode. The master works on branch `{code['branch']}`; Dispatch opens a pull request when it reports done.\n")
assert activities()[0]["summary"].endswith("(code)") and activities()[-1]["issue-url"] == comment_url("result")
meta = json.loads((Path(launch["run_dir"]) / "meta.json").read_text())
assert meta["mode"] == "code" and meta["code"] == code
packet = (Path(launch["run_dir"]) / "packet.md").read_text()
assert all(f"{key}: {value}\n" in packet for key, value in code.items())
assert f"## Repo state\nBranch: {code['branch']}\nHEAD: {base_sha}" in packet
assert f"including {project}" in packet and "with `--tests` describing the commands" in packet
assert "--branch/--commit are auto-filled" in packet and "do not delegate to workers" not in packet
assert "aoc-worker-wait" in packet
assert "never wait for them with run_in_background" in packet
assert f"built-in: git push {bare} {code['branch']}:refs/heads/{code['branch']}" in git_trace.read_text()
assert json.loads(dispatch("status", "--json").stdout)["mode"] == "code"

# Readable status: idle seat, the 10 most recently claimed issues, PR column from result.json.
record = local()["issues"]["2"]
assert dispatch("status").stdout.splitlines() == [
    "Seat: IDLE", "Active: none",
    f"#2  review  last run {record['last_run_id']}  claimed {record['claimed_at']}  PR {result()['pr_url']}"]
saved = (state_dir / "state.json").read_text()
state = local()
for number in range(10, 21):
    state["issues"][str(number)] = {"state": "failed", "last_run_id": f"run-{number}",
                                    "claimed_at": f"2026-09-{number:02d}T00:00:00Z"}
(state_dir / "state.json").write_text(json.dumps(state))
lines = dispatch("status").stdout.splitlines()
assert len(lines) == 12 and lines[2].startswith("#2  review  ") and lines[2].endswith(f"  PR {result()['pr_url']}")
assert lines[3:] == [f"#{number}  failed  last run run-{number}  claimed 2026-09-{number:02d}T00:00:00Z"
                     for number in range(20, 11, -1)], lines
assert len(json.loads(dispatch("status", "--json").stdout)["issues"]) == 12
(state_dir / "state.json").write_text(saved)
for key in ("GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN",
            "GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0", "HERDR_WORKSPACE_ID"):
    environment.pop(key, None)

# Config/CLI precedence, defaults and invalid config stay local.
import argparse
for mode, configured, cli_timeout, timeout in (
        ("code", None, None, 7200), ("dry-run", None, None, 1800),
        ("code", 41, None, 41), ("code", 41, 7, 7)):
    (project / ".aoc/dispatch.toml").write_text(
        f'mode = "{mode}"\n' + (f"timeout = {configured}\n" if configured else ""))
    instance = module["Dispatcher"](argparse.Namespace(root=str(project), repo="fixture/dispatch", timeout=cli_timeout))
    instance.configure()
    assert instance.args.timeout == timeout and instance.args.fetch_remote == "origin"
    assert instance.args.push_url == "git@github.com:fixture/dispatch.git"
(project / ".aoc/dispatch.toml").write_text('mode = "invalid"\n')
before = github()
assert "mode must be dry-run or code" in dispatch("tick", expected=1).stderr
assert github() == before

# z2: ordered check failures never push or call PR APIs.
for behaviour, check in (("dirty", "clean worktree"), ("untracked", "clean worktree"),
                         ("wrong-branch", "branch"), ("no-commits", "commits ahead"),
                         ("wrong-commit", "report identity"), ("unrelated", "base ancestry"),
                         ("wrong-report-branch", "report identity")):
    code_fixture("z2-" + behaviour)
    environment["FAKE_CODE_BEHAVIOUR"] = behaviour
    dispatch("tick")
    assert result()["outcome"] == "failed" and f"PR checks failed: {check}:" in result()["reason"]
    assert code_record()["branch"] in comments("failed")[0] and issue_labels() == {"agent-failed"}
    assert_no_publication()

# z3: blocked work persists; continuation reuses identity, pushes another commit, reuses PR.
code_fixture("z3")
environment["FAKE_CLAUDE_MODE"] = "blocked"
dispatch("tick")
original = local()["issues"]["2"]
code = code_record()
first_head = git_at(code["worktree"], "rev-parse", "HEAD")
assert result()["outcome"] == "needs_alex" and issue_labels() == {"needs-alex"}
assert code["branch"] in comments("blocked")[0]
assert_no_publication()
state = github()
state["prs"] = [{"head": code["branch"], "number": 99, "url": "https://github.com/fixture/dispatch/pull/99"}]
state_file.write_text(json.dumps(state))
environment.update(AOC_DISPATCH_NOW="2026-10-01T12:01:00Z", FAKE_CLAUDE_MODE="done")
answer(f"AOC-DECISION {original['decision_id']}\nImplement it")
human("--remove-label", "needs-alex", "--add-label", "agent-ready")
dispatch("tick")
assert code_record() == code and len(calls()) == 2 and calls()[0]["cwd"] == calls()[1]["cwd"]
assert git_at(code["worktree"], "rev-parse", "HEAD^") == first_head
assert git_at(code["worktree"], "rev-list", "--count", f"{base_sha}..HEAD") == "2"
assert result()["pr_url"] == state["prs"][0]["url"] and issue_labels() == {"agent-review"}
assert not any(entry["argv"][:2] == ["pr", "create"] for entry in github()["log"])
assert sum(entry["argv"][:2] == ["repo", "view"] for entry in github()["log"]) == 1
continued = json.loads((Path(calls()[1]["run_dir"]) / "meta.json").read_text())
assert continued["code"] == code and continued["continuation_of"] == original["last_run_id"]

# z3 missing worktree: restore recorded branch without fetching/changing its base.
code_fixture("z3-missing")
environment["FAKE_CLAUDE_MODE"] = "blocked"
dispatch("tick")
original = local()["issues"]["2"]
code = code_record()
first_head = git_at(code["worktree"], "rev-parse", "HEAD")
shutil.rmtree(code["worktree"])
environment.update(AOC_DISPATCH_NOW="2026-10-01T12:01:00Z", FAKE_CLAUDE_MODE="done")
answer(f"AOC-DECISION {original['decision_id']}\nImplement it")
human("--remove-label", "needs-alex", "--add-label", "agent-ready")
dispatch("tick")
assert result()["outcome"] == "review", result()
assert code_record() == code and git_at(code["worktree"], "rev-parse", "HEAD^") == first_head

# z4: a missing local push destination fails before any PR request.
directory = code_fixture("z4")
(project / ".aoc/dispatch.toml").write_text(
    f'mode = "code"\npush_url = {json.dumps(str(directory / "missing.git"))}\n')
dispatch("tick")
assert result()["outcome"] == "failed" and result()["reason"].startswith("push failed:")
assert not any(entry["argv"][0] == "pr" for entry in github()["log"])
assert git_at(project, "ls-remote", "--heads", bare) == f"{base_sha}\trefs/heads/main"

# z5: missing fetch remote fails before packet/master.
code_fixture("z5", 'fetch_remote = "missing"\n')
dispatch("tick")
assert result()["outcome"] == "failed" and result()["reason"].startswith("code setup failed:")
assert not calls() and issue_labels() == {"agent-failed"}
assert comments("claim") and not (state_dir / "runs" / local()["issues"]["2"]["last_run_id"] / "packet.md").exists()
assert_no_publication()

# Existing local branches are never reused for a fresh issue; slug fallback is safe.
code_fixture("z1-collision")
git_at(project, "branch", "aoc/issue-2-work")
state = github()
state["issues"]["2"]["title"] = "???"
state_file.write_text(json.dumps(state))
dispatch("tick")
run_id = local()["issues"]["2"]["last_run_id"]
assert code_record()["branch"] == "aoc/issue-2-work-" + run_id[-4:]
assert git_at(project, "rev-parse", "aoc/issue-2-work") == base_sha
assert result()["outcome"] == "review"

# PR failure cannot become agent-review after a successful push.
code_fixture("z1-pr-failure")
environment["FAKE_GH_PR_FAIL"] = "1"
dispatch("tick")
assert result()["outcome"] == "failed" and result()["reason"].startswith("pr failed:")
assert issue_labels() == {"agent-failed"}
assert git_at(project, "ls-remote", bare, f"refs/heads/{code_record()['branch']}")
environment.pop("FAKE_GH_PR_FAIL")

# Active status and the actual seat surface show code mode and branch.
code_fixture("z1-active")
environment["FAKE_CLAUDE_MODE"] = "sleep"
dispatch("tick", wait=False)
until(lambda: len(calls()) == 1)
active = local()["active"]
status = json.loads(dispatch("status", "--json").stdout)
assert status["mode"] == "code" and status["branch"] == code_record()["branch"]
assert dispatch("status").stdout.splitlines()[:2] == [
    "Seat: RUNNING", f"Active: issue 2 run {active['run_id']} elapsed 0s branch {code_record()['branch']}"]
proc, output = start_seat(project, "z1:active", frozen_clock=True)
until(lambda: f"Branch: {code_record()['branch']}" in output.read_text())
assert "Mode: code" in output.read_text()
stop_seat(proc, "z1:active")
stop_master(active["pid"])
environment["FAKE_CLAUDE_MODE"] = "done"
dispatch("tick")
assert result()["outcome"] == "review" and issue_labels() == {"agent-review"}, result()
assert len(calls()) == 2 and calls()[1]["argv"][calls()[1]["argv"].index("--resume") + 1] == "fixture-session"
assert result()["pr_url"] == github()["prs"][0]["url"]
assert git_at(project, "ls-remote", bare, f"refs/heads/{code_record()['branch']}")

# z6: CLI can override configured code mode without repo-view/worktree/push/PR calls.
code_fixture("z6-override")
dispatch("tick", "--mode", "dry-run")
assert result()["outcome"] == "review" and calls()[0]["cwd"] == str(project)
assert not any(entry["argv"][0] in ("repo", "pr") for entry in github()["log"])
assert "built-in: git worktree" not in git_trace.read_text() and "built-in: git push" not in git_trace.read_text()
# z7: an early-exiting master resumes in-session and waits for both delegated workers.
code_fixture("z7")
environment["FAKE_CLAUDE_MODE"] = "delegate"
dispatch("tick")
assert result()["outcome"] == "review" and issue_labels() == {"agent-review"}
assert result()["pr_url"] == github()["prs"][0]["url"]
assert any(entry["argv"][:2] == ["pr", "create"] for entry in github()["log"])
assert len(calls()) == 2
initial, resumed = calls()
assert "--resume" not in initial["argv"] and "--no-session-persistence" not in initial["argv"]
argv = resumed["argv"]
assert argv[argv.index("--resume"):argv.index("--resume") + 2] == ["--resume", "fixture-session"]
assert "resumed run" in argv[argv.index("-p") + 1]
herdr = json.loads(herdr_state.read_text())
assert [tab["label"] for tab in herdr["tabs"]] == ["issue-2-w1", "issue-2-w2"]
assert [run["pane_id"] for run in herdr["runs"]] == ["fixture:p1", "fixture:p2"]
assert all(run["command"].startswith("aoc-omp ") for run in herdr["runs"])
run_dir = Path(initial["run_dir"])
for k in (1, 2):
    text = (run_dir / "workers" / f"issue-2-w{k}.md").read_text()
    assert text in (Path(code_record()["worktree"]) / "change.txt").read_text()
log_text = (run_dir / "claude.log").read_text()
assert "fake master output: initial" in log_text and "fake master output: resumed" in log_text
assert result()["reason"] == "Delegated workers integrated"
assert "aoc-worker-wait: 2 done" in comments("result")[0]
assert local()["seat"] == "IDLE" and local()["active"] is None

# z8: a foreground wait reports a missing worker, not a missing master report.
code_fixture("z8")
environment["FAKE_CLAUDE_MODE"] = "delegate-lost"
dispatch("tick")
assert result()["outcome"] == "failed" and "fixture:p2" in result()["reason"]
assert len(calls()) == 1 and "without a report" not in result()["reason"]
report = json.loads((Path(calls()[0]["run_dir"]) / "report.json").read_text())
assert "fixture:p2 missing" in report["evidence"]
assert_no_publication()

# z9: successful no-report turns exhaust the bounded same-session resumes.
code_fixture("z9")
environment["FAKE_CLAUDE_MODE"] = "noreport"
dispatch("tick")
assert len(calls()) == 4
for launch in calls()[1:]:
    argv = launch["argv"]
    assert argv[argv.index("--resume"):argv.index("--resume") + 2] == ["--resume", "fixture-session"]
assert result()["reason"] == "master process ended without a report after 3 resumes"
assert result()["outcome"] == "failed"

# z9b: a nonzero master exit never resumes.
code_fixture("z9b")
environment.update(FAKE_CLAUDE_MODE="noreport", FAKE_CLAUDE_EXIT="1")
dispatch("tick")
assert len(calls()) == 1 and "master exited 1 without a report" in result()["reason"]

# z9c: missing session metadata cannot resume.
code_fixture("z9c")
environment.update(FAKE_CLAUDE_MODE="noreport", FAKE_CLAUDE_NO_SESSION="1")
dispatch("tick")
assert len(calls()) == 1 and result()["reason"] == "master process ended without a report"

# z9d: an exited master beyond the original deadline cannot resume after restart.
code_fixture("z9d")
environment["FAKE_CLAUDE_MODE"] = "noreport"
dispatch("tick", wait=False)
until(lambda: len(calls()) == 1 and not pid_running(local()["active"]["pid"]))
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:00:02Z"
dispatch("tick", "--timeout", "1")
assert len(calls()) == 1 and result()["outcome"] == "failed"
assert "master process ended without a report" in result()["reason"]

project, state_dir = dry_project, dry_state_dir
environment.update(FAKE_DISPATCH_STATE=str(state_dir / "state.json"), FAKE_PRISM_ROOT=str(project))
reset()
git_trace.write_text("")
dispatch("tick")
assert not any(entry["argv"][0] in ("repo", "pr") for entry in github()["log"])
assert "built-in: git worktree" not in git_trace.read_text() and "built-in: git push" not in git_trace.read_text()
assert json.loads(dispatch("status", "--json").stdout)["mode"] == "dry-run"

# aa: a partial label edit removes ready, then fails; failure still releases the seat.
for available in (["agent-ready", "agent-failed"], ["agent-ready"]):
    reset()
    environment["FAKE_GH_ENFORCE_LABELS"] = "1"
    state = github()
    state["labels"] = {name: {} for name in available}
    state_file.write_text(json.dumps(state))
    proc = dispatch("tick")
    record = local()["issues"]["2"]
    assert record["state"] == "failed" and local()["seat"] == "IDLE" and local()["active"] is None
    assert not calls() and not comments("claim") and "claim failed:" in proc.stderr
    claim = next(entry for entry in github()["log"] if "local_state_before_claim" in entry)
    assert claim["local_state_before_claim"]["seat"] == "CLAIMED"
    assert "agent-ready" not in issue_labels()
    if "agent-failed" in available:
        assert issue_labels() == {"agent-failed"} and result()["outcome"] == "failed"
        assert result()["reason"].startswith("claim failed:") and "label not found: agent-running" in result()["reason"]
        assert result()["reason"] in comments("failed")[0]
    else:
        assert "finalization failed:" in proc.stderr and not comments("failed")
    events, issue_comments = github()["events"], github()["issues"]["2"]["comments"]
    dispatch("tick")
    assert not calls() and local()["issues"]["2"] == record and local()["seat"] == "IDLE"
    assert github()["events"] == events and github()["issues"]["2"]["comments"] == issue_comments

# Claim comment errors, and errors posting the failure, also release the seat.
for event in ("claim", "failed"):
    reset()
    environment["FAKE_GH_FAIL_COMMENT_EVENT"] = event
    if event == "failed":
        environment["FAKE_GH_ENFORCE_LABELS"] = "1"
        state = github()
        state["labels"] = {"agent-ready": {}, "agent-failed": {}}
        state_file.write_text(json.dumps(state))
    proc = dispatch("tick")
    assert local()["issues"]["2"]["state"] == "failed"
    assert local()["seat"] == "IDLE" and local()["active"] is None and not calls()
    assert issue_labels() == {"agent-failed"} and "claim failed:" in proc.stderr
    if event == "claim":
        assert "fixture comment failed" in result()["reason"] and comments("failed")
    else:
        assert "finalization failed:" in proc.stderr
    dispatch("tick")
    assert not calls() and local()["seat"] == "IDLE"

# ab: in-process self claims recover before either timeout signal path.
from unittest.mock import patch


def interrupted_state(seat_name, pid, terminating=False):
    dispatch("status", "--json")
    state = local()
    run_id = "2-interrupted"
    state.update(seat=seat_name, active={"run_id": run_id, "issue": 2, "pid": pid})
    if terminating:
        state["active"]["terminating_at"] = time.time() - 20
    state["issues"]["2"] = {"state": "running", "last_run_id": run_id, "mode": "dry-run",
        "claimed_at": "2026-10-01T11:00:00Z", "decision_id": None,
        "blocked_at": None, "ready_seen_at": None, "runs": [run_id]}
    (state_dir / "state.json").write_text(json.dumps(state))


for terminating in (False, True):
    reset([(2, ["agent-running"])])
    interrupted_state("CLAIMED", os.getpid(), terminating)
    with patch.dict(os.environ, environment, clear=True):
        instance = module["Dispatcher"](argparse.Namespace(root=str(project), repo="fixture/dispatch"))
        instance.configure()
        with patch.object(os, "killpg", side_effect=AssertionError("dispatcher must not be signaled")) as killpg:
            instance.tick()
            killpg.assert_not_called()
    assert result()["reason"] == "claim interrupted" and result()["outcome"] == "failed"
    assert local()["seat"] == "IDLE" and local()["active"] is None
    assert issue_labels() == {"agent-failed"} and "claim interrupted" in comments("failed")[0]
    assert not calls()
    dispatch("tick")
    assert not calls() and local()["seat"] == "IDLE"

# A different PID in our group must be protected from both SIGTERM and SIGKILL.
for terminating in (False, True):
    reset([(2, ["agent-running"])])
    child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
    background.append(child)
    try:
        assert os.getpgid(child.pid) == os.getpgid(0)
        interrupted_state("RUNNING", child.pid, terminating)
        with patch.dict(os.environ, environment, clear=True):
            instance = module["Dispatcher"](argparse.Namespace(root=str(project), repo="fixture/dispatch", timeout=1))
            instance.configure()
            with patch.object(os, "killpg", side_effect=AssertionError("own group must not be signaled")) as killpg:
                instance.tick()
                killpg.assert_not_called()
        assert child.poll() is None
        assert result()["reason"] == "master shares dispatcher process group"
        assert local()["seat"] == "IDLE" and local()["active"] is None
        assert issue_labels() == {"agent-failed"} and not calls()
    finally:
        child.terminate()
        child.wait(timeout=5)

# ac: owner seat/watch create every label once, continue on failure, retry next start.
for command_name in ("seat", "watch"):
    for fail_label in (None, "agent-running"):
        reset([(2, ["risk-review"])])
        config_path.write_text('inbox = "fixture/dispatch"\ninterval = 0.05\n')
        if fail_label:
            environment["FAKE_GH_FAIL_LABEL"] = fail_label
        pane = f"ac:{command_name}:{fail_label or 'success'}"
        proc, output = start_seat(project, pane, command_name)
        until(lambda: (heartbeat(pane) or {}).get("role") == "owner")
        until(lambda: sum(entry["argv"][:2] == ["api", "-i"] for entry in github()["log"]) >= 3)
        creations = [entry for entry in github()["log"] if entry["argv"][:2] == ["label", "create"]]
        assert [entry["argv"][2] for entry in creations] == list(expected_colors)
        assert all(entry["argv"][-1] == "--force" for entry in creations)
        assert proc.poll() is None and local()["seat"] == "IDLE" and not calls()
        if fail_label:
            assert "label creation failed:" in output.read_text()
            assert set(github()["labels"]) == set(expected_colors) - {fail_label}
        else:
            assert set(github()["labels"]) == set(expected_colors)
        stop_seat(proc, pane)
        if fail_label:
            environment.pop("FAKE_GH_FAIL_LABEL")
            start = len(github()["log"])
            proc, output = start_seat(project, pane, command_name)
            until(lambda: set(github()["labels"]) == set(expected_colors))
            until(lambda: sum(entry["argv"][:2] == ["api", "-i"] for entry in github()["log"][start:]) >= 3)
            stop_seat(proc, pane)
            creations = [entry for entry in github()["log"][start:] if entry["argv"][:2] == ["label", "create"]]
            assert [entry["argv"][2] for entry in creations] == list(expected_colors)
        config_path.unlink()

# ad: visible master, real launch script, isolated publication and one trust answer.
code_fixture("ad space'quote", "interval = 0.05\n")
environment.update(HERDR_WORKSPACE_ID="fixture-workspace", FAKE_CLAUDE_MODE="delayed",
                   GH_TOKEN="fixture-token", GITHUB_TOKEN="fixture-token",
                   GH_ENTERPRISE_TOKEN="fixture-token", GITHUB_ENTERPRISE_TOKEN="fixture-token",
                   GIT_CONFIG_COUNT="1", GIT_CONFIG_KEY_0="color.ui", GIT_CONFIG_VALUE_0="false")
dispatch("tick", wait=False)
until(lambda: len(calls()) == 1)
dispatch("tick", wait=False)
active = local()["active"]
run_dir = state_dir / "runs" / active["run_id"]
code = code_record()
assert active["surface"] == "tab" and active["tab_label"] == "issue-2"
assert active["pane_id"] == "fixture:p1" and active["tab_id"] == "fixture:t1"
assert active["pid"] == int((run_dir / "master.pid").read_text())
import uuid
assert str(uuid.UUID(active["session_id"])) == active["session_id"]
launch = calls()[0]
argv, env = launch["argv"], launch["env"]
assert "-p" not in argv and "--output-format" not in argv and "--verbose" not in argv
assert argv[argv.index("--session-id") + 1] == active["session_id"]
assert argv[argv.index("--permission-mode") + 1] == "bypassPermissions"
assert argv[argv.index("--disallowedTools") + 1:argv.index("--append-system-prompt")] == [
    "Bash(git push:*)", "Bash(gh:*)"]
assert argv[argv.index("--append-system-prompt") + 1] == "Fixture communication contract."
assert launch["cwd"] == code["worktree"] and env["AOC_DISPATCH_WORKTREE"] == code["worktree"]
assert not any(key in env for key in ("GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"))
assert env["GH_CONFIG_DIR"] == str(run_dir / "gh-config") and env["GIT_TERMINAL_PROMPT"] == "0"
assert env["GIT_CONFIG_COUNT"] == "4" and env["GIT_CONFIG_KEY_0"] == "color.ui"
for index, url in enumerate(("https://github.com/", "git@github.com:", "ssh://git@github.com/"), 1):
    assert env[f"GIT_CONFIG_KEY_{index}"] == "url.aoc-push-blocked:///.pushInsteadOf"
    assert env[f"GIT_CONFIG_VALUE_{index}"] == url
herdr = json.loads(herdr_state.read_text())
assert herdr["tabs"] == [{"tab_id": "fixture:t1", "label": "issue-2", "cwd": code["worktree"]}]
assert herdr["calls"][0] == ["tab", "create", "--workspace", "fixture-workspace",
                             "--cwd", code["worktree"], "--label", "issue-2", "--no-focus"]
assert [row[3] for row in herdr["calls"] if row[:2] == ["pane", "send-keys"]] == ["Down", "Enter"]
assert active["trust_answered"]
assert json.loads(dispatch("status", "--json").stdout)["active"] == active
assert "Surface: tab  Tab: issue-2  Pane: fixture:p1" in dispatch("status").stdout
proc, output = start_seat(project, "ad:master", frozen_clock=True)
until(lambda: "Tab: issue-2 pane fixture:p1" in output.read_text())
assert "Surface: tab" in output.read_text() and "tool Read:" not in output.read_text()
stop_seat(proc, "ad:master")
# A report wins even while the interactive master remains alive, without signaling it.
(run_dir / "release").touch()
until(lambda: (run_dir / "report.json").exists())
child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"], start_new_session=True)
background.append(child)
(run_dir / "master.pid").write_text(str(child.pid))
dispatch("tick", wait=False)
assert child.poll() is None and result()["outcome"] == "review"
head = git_at(code["worktree"], "rev-parse", "HEAD")
assert git_at(project, "ls-remote", bare, f"refs/heads/{code['branch']}") == f"{head}\trefs/heads/{code['branch']}"
assert issue_labels() == {"agent-review"} and result()["pr_url"] == github()["prs"][0]["url"]
assert len(calls()) == 1
herdr = json.loads(herdr_state.read_text())
assert not any("close" in row for row in herdr["calls"])
assert [row[3] for row in herdr["calls"] if row[:2] == ["pane", "send-keys"]] == ["Down", "Enter"]
stop_master(child.pid)
for key in ("GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN",
            "GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0"):
    environment.pop(key, None)

# The same six publication checks reject invalid tab reports before pushing.
for behaviour, check in (("dirty", "clean worktree"), ("wrong-branch", "branch"),
                         ("unrelated", "base ancestry"), ("no-commits", "commits ahead"),
                         ("wrong-commit", "report identity"), ("wrong-report-branch", "report identity")):
    code_fixture("ad-check-" + behaviour)
    environment.update(HERDR_WORKSPACE_ID="fixture-workspace", FAKE_CODE_BEHAVIOUR=behaviour)
    dispatch("tick")
    assert result()["outcome"] == "failed" and f"PR checks failed: {check}:" in result()["reason"]
    assert_no_publication()

# Both Herdr launch failures release the claim, never closing the inspection tab.
for fault in ("tab create", "pane run"):
    code_fixture("ad-fail-" + fault.replace(" ", "-"))
    environment.update(HERDR_WORKSPACE_ID="fixture-workspace", FAKE_HERDR_FAIL=fault)
    dispatch("tick")
    assert result()["reason"].startswith("claim failed:") and "fixture herdr failure" in result()["reason"]
    assert local()["seat"] == "IDLE" and issue_labels() == {"agent-failed"} and not calls()
    assert not any("close" in row for row in json.loads(herdr_state.read_text())["calls"])

# ae: dead interactive masters resume in the same pane/session; all exit codes qualify.
code_fixture("ae")
environment.update(HERDR_WORKSPACE_ID="fixture-workspace", FAKE_CLAUDE_MODE="noreport", FAKE_CLAUDE_EXIT="7")
dispatch("tick")
assert len(calls()) == 4 and result()["reason"] == "issue master exited without a report after 3 resumes"
sid = calls()[0]["argv"][calls()[0]["argv"].index("--session-id") + 1]
for launch in calls()[1:]:
    argv = launch["argv"]
    assert argv[argv.index("--resume") + 1] == sid and "--session-id" not in argv and "-p" not in argv
herdr = json.loads(herdr_state.read_text())
assert len(herdr["tabs"]) == 1 and all(row["pane_id"] == "fixture:p1" for row in herdr["runs"])
assert len(herdr["runs"]) == 4 and all(row["command"].endswith(" --resume") for row in herdr["runs"][1:])
assert result()["outcome"] == "failed"
assert_no_publication()

# af: persisted first-idle clock resets on work; each nudge gets a fresh interval.
code_fixture("af", "nudge_after = 1\n")
environment.update(HERDR_WORKSPACE_ID="fixture-workspace", FAKE_CLAUDE_MODE="sleep")
dispatch("tick", wait=False)
until(lambda: len(calls()) == 1)
dispatch("tick", wait=False)
active = local()["active"]
def pane_status(status):
    state = json.loads(herdr_state.read_text())
    state["panes"][0]["agent_status"] = status
    herdr_state.write_text(json.dumps(state))
pane_status("idle")
dispatch("tick", wait=False)
assert local()["active"]["idle_since"] == "2026-10-01T12:00:00Z"
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:00:01Z"
dispatch("tick", wait=False)
assert local()["active"]["nudges"] == 0
pane_status("unknown")
dispatch("tick", wait=False)
assert local()["active"]["idle_since"] == "2026-10-01T12:00:00Z"
pane_status("working")
dispatch("tick", wait=False)
assert local()["active"]["idle_since"] is None
pane_status("idle")
dispatch("tick", wait=False)
for second in (3, 5, 7):
    environment["AOC_DISPATCH_NOW"] = f"2026-10-01T12:00:{second:02d}Z"
    dispatch("tick", wait=False)
    assert local()["active"]["nudges"] == (second - 1) // 2
herdr = json.loads(herdr_state.read_text())
nudges = [row for row in herdr["calls"] if row[:2] == ["pane", "send-text"]]
assert len(nudges) == 3 and all(row[2] == active["pane_id"] for row in nudges)
assert all(row[3] == (
    f"AOC Dispatch: run {active['run_id']} has no aoc-report yet. Nobody answers in this tab. "
    "Continue the work, or call aoc-report with blocked or failed now.") for row in nudges)
assert sum(row[:2] == ["pane", "send-keys"] and row[-1] == "Enter" for row in herdr["calls"]) == 4
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:00:09Z"
dispatch("tick", wait=False)
assert result()["reason"] == "issue master idle without a report"
assert pid_running(active["pid"])  # No timeout signal and no tab close.
stop_master(active["pid"])
assert_no_publication()

# ai: an idle master whose worker tab is still busy (or "unknown") is waiting, not stalled.
code_fixture("ai", "nudge_after = 1\n")
environment.update(HERDR_WORKSPACE_ID="fixture-workspace", FAKE_CLAUDE_MODE="sleep")
dispatch("tick", wait=False)
until(lambda: len(calls()) == 1)
dispatch("tick", wait=False)
active = local()["active"]
state = json.loads(herdr_state.read_text())
state["panes"][0]["agent_status"] = "idle"
state["tabs"].append({"tab_id": "fixture:tw1", "label": "issue-2-w1", "cwd": "/"})
state["panes"].append({"pane_id": "fixture:pw1", "tab_id": "fixture:tw1", "agent_status": "unknown"})
herdr_state.write_text(json.dumps(state))
for second in (1, 3, 5, 7, 9):
    environment["AOC_DISPATCH_NOW"] = f"2026-10-01T12:00:{second:02d}Z"
    dispatch("tick", wait=False)
    assert local()["active"]["idle_since"] is None and local()["active"]["nudges"] == 0
assert not any(row[:2] == ["pane", "send-text"] for row in json.loads(herdr_state.read_text())["calls"])
state = json.loads(herdr_state.read_text())
state["panes"][-1]["agent_status"] = "idle"
herdr_state.write_text(json.dumps(state))
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:00:10Z"
dispatch("tick", wait=False)
assert local()["active"]["idle_since"] == "2026-10-01T12:00:10Z"
stop_master(active["pid"])

# ag: disappeared panes fail; seats restart from disk and reports win over missing panes.
code_fixture("ag-missing")
environment.update(HERDR_WORKSPACE_ID="fixture-workspace", FAKE_CLAUDE_MODE="sleep")
dispatch("tick", wait=False)
until(lambda: len(calls()) == 1)
dispatch("tick", wait=False)
active = local()["active"]
state = json.loads(herdr_state.read_text())
state["panes"] = []
herdr_state.write_text(json.dumps(state))
dispatch("tick", wait=False)
assert result()["reason"] == "issue master tab closed"
stop_master(active["pid"])

code_fixture("ag-restart", "interval = 0.05\n")
environment.update(HERDR_WORKSPACE_ID="fixture-workspace", FAKE_CLAUDE_MODE="delayed")
proc, output = start_seat(project, "ag:first", frozen_clock=True)
until(lambda: (state_dir / "state.json").exists() and (state := local())["seat"] == "RUNNING"
      and state["active"].get("surface") == "tab" and state["active"].get("pid") is not None)
active = local()["active"]
stop_seat(proc, "ag:first")
assert pid_running(active["pid"])
run_dir = state_dir / "runs" / active["run_id"]
proc, output = start_seat(project, "ag:second", frozen_clock=True)
until(lambda: "Tab: issue-2 pane fixture:p1" in output.read_text())
assert len(calls()) == 1 and local()["active"]["session_id"] == active["session_id"]
(run_dir / "release").touch()
until(lambda: local()["seat"] == "IDLE")
stop_seat(proc, "ag:second")
assert result()["outcome"] == "review" and len(calls()) == 1

# A late trust dialog is handled by the restarted dispatcher, once.
code_fixture("ag-late-trust")
environment.update(HERDR_WORKSPACE_ID="fixture-workspace", FAKE_CLAUDE_MODE="delayed")
herdr_text.write_text("")
started = time.monotonic()
dispatch("tick", wait=False)
assert time.monotonic() - started < 15
until(lambda: len(calls()) == 1)
assert not local()["active"]["trust_answered"]
herdr_text.write_text("Yes, I trust this folder\n❯ No, exit\n")
dispatch("tick", wait=False)
dispatch("tick", wait=False)
assert local()["active"]["trust_answered"]
assert [row[-1] for row in json.loads(herdr_state.read_text())["calls"]
        if row[:2] == ["pane", "send-keys"]] == ["Down", "Enter"]
run_dir = Path(calls()[0]["run_dir"])
(run_dir / "release").touch()
until(lambda: (run_dir / "report.json").exists())
state = json.loads(herdr_state.read_text())
state["panes"] = []
herdr_state.write_text(json.dumps(state))
dispatch("tick", wait=False)
assert result()["outcome"] == "review"  # Report precedes the missing-pane failure.

# ag startup and timeout: no PID gets 90s; SIGTERM/SIGKILL keep the tab open.
code_fixture("ag-start")
environment.update(HERDR_WORKSPACE_ID="fixture-workspace", FAKE_CLAUDE_MODE="noreport")
dispatch("tick", wait=False)
until(lambda: len(calls()) == 1)
run_dir = Path(calls()[0]["run_dir"])
until(lambda: not pid_running(int((run_dir / "master.pid").read_text())))
(run_dir / "master.pid").unlink()
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:01:30Z"
dispatch("tick", wait=False)
assert result()["reason"] == "issue master did not start"

code_fixture("ag-timeout")
environment.update(HERDR_WORKSPACE_ID="fixture-workspace", FAKE_CLAUDE_MODE="stubborn")
dispatch("tick", wait=False)
until(lambda: len(calls()) == 1)
dispatch("tick", wait=False)
active = local()["active"]
environment["AOC_DISPATCH_NOW"] = "2026-10-01T12:00:02Z"
dispatch("tick", "--timeout", "1", wait=False)
assert pid_running(active["pid"]) and "terminating_at" in local()["active"]
state = local()
state["active"]["terminating_at"] = time.time() - 11
(state_dir / "state.json").write_text(json.dumps(state))
dispatch("tick", "--timeout", "1", wait=False)
until(lambda: not pid_running(active["pid"]))
assert result()["reason"] == "timeout"
assert not any("close" in row for row in json.loads(herdr_state.read_text())["calls"])

# ah: continuation gets a new tab; no workspace and dry-run keep headless argv.
code_fixture("ah")
environment.update(HERDR_WORKSPACE_ID="fixture-workspace", FAKE_CLAUDE_MODE="blocked")
dispatch("tick")
original = local()["issues"]["2"]
environment.update(AOC_DISPATCH_NOW="2026-10-01T12:01:00Z", FAKE_CLAUDE_MODE="done")
answer(f"AOC-DECISION {original['decision_id']}\nImplement it")
human("--remove-label", "needs-alex", "--add-label", "agent-ready")
dispatch("tick")
assert [tab["label"] for tab in json.loads(herdr_state.read_text())["tabs"]] == ["issue-2", "issue-2-run2"]
assert calls()[0]["cwd"] == calls()[1]["cwd"] and result()["outcome"] == "review"
code_fixture("ah-headless")
dispatch("tick")
assert calls()[0]["argv"][0] == "-p" and "--session-id" not in calls()[0]["argv"]
assert not json.loads(herdr_state.read_text())["tabs"]
code_fixture("ah-dry")
environment["HERDR_WORKSPACE_ID"] = "fixture-workspace"
dispatch("tick", "--mode", "dry-run")
assert calls()[0]["argv"][0] == "-p" and "--no-session-persistence" in calls()[0]["argv"]
assert not json.loads(herdr_state.read_text())["tabs"]
code_fixture("ah-no-herdr")
environment.update(HERDR_WORKSPACE_ID="fixture-workspace", AOC_DISPATCH_HERDR_BIN=str(fake_bin / "missing-herdr"))
dispatch("tick")
assert calls()[0]["argv"][0] == "-p" and not json.loads(herdr_state.read_text())["tabs"]

print("AOC Dispatch smoke passed (a-y, z1-z9, aa-ac, ad-ai)")
PY
