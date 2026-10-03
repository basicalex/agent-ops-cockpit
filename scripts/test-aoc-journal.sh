#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

python3 - "$root/bin/aoc-journal" "$tmp_dir" <<'PY'
import copy
import json
import os
from pathlib import Path
import subprocess
import sys

cli = Path(sys.argv[1])
tmp = Path(sys.argv[2]).resolve()
fake_bin = tmp / "bin"
fake_bin.mkdir()
state_file = tmp / "github.json"
repo = "owner/project"
issue_url = f"https://github.com/{repo}/issues/9"
pr_url = f"https://github.com/{repo}/pull/27"
sha = "a1" * 20
head = "b2" * 20
short = "c3" * 6
run_id = "9-20261003T120000Z-abcd"
env = dict(os.environ, PATH=f"{fake_bin}:{os.environ['PATH']}",
           AOC_JOURNAL_GH_BIN=str(fake_bin / "gh"), FAKE_GH_STATE=str(state_file))

(fake_bin / "gh").write_text(r'''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys

path = Path(os.environ["FAKE_GH_STATE"])
state = json.loads(path.read_text())
argv = sys.argv[1:]
entry = {"argv": argv}
state["log"].append(entry)
repo = state["repo"]
output = None
if state.get("error"):
    path.write_text(json.dumps(state))
    print(state["error"], file=sys.stderr)
    sys.exit(1)
if argv == ["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"]:
    output = repo
elif argv[:2] == ["issue", "view"]:
    assert argv[3:6] == ["--repo", repo, "--json"] and len(argv) == 7, argv
    assert argv[6] in ("state", "number,title,body,state,labels,comments,url,closedAt"), argv
    issue = state["issues"][argv[2]]
    output = {key: issue[key] for key in argv[6].split(",")}
elif argv[:2] == ["issue", "comment"]:
    assert argv[3:6] == ["--repo", repo, "--body-file"] and len(argv) == 7, argv
    entry["body"] = Path(argv[6]).read_text(encoding="utf-8")
    comments = state["issues"][argv[2]]["comments"]
    url = f"https://github.com/{repo}/issues/{argv[2]}#issuecomment-{len(comments) + 1}"
    comments.append({"body": entry["body"], "url": url,
                     "createdAt": "2026-10-03T13:00:00Z", "author": {"login": "agent"}})
    output = state.get("comment_output", "Comment posted\nhttps://earlier.invalid/comment\n" + url + "\nDone\n")
elif argv[:2] == ["issue", "create"]:
    assert argv[2:5] == ["--repo", repo, "--title"] and argv[6] == "--body-file" and len(argv) == 8, argv
    entry["body"] = Path(argv[7]).read_text(encoding="utf-8")
    number = max(map(int, state["issues"])) + 1
    url = f"https://github.com/{repo}/issues/{number}"
    state["issues"][str(number)] = {"number": number, "title": argv[5], "body": entry["body"],
                                    "state": "OPEN", "labels": [], "comments": [],
                                    "url": url, "closedAt": None}
    output = state.get("create_output", "Created\nhttps://earlier.invalid/issue\n" + url + "\nDone\n")
else:
    raise AssertionError(argv)
path.write_text(json.dumps(state))
print(json.dumps(output) if isinstance(output, dict) else output)
''', encoding="utf-8")
(fake_bin / "gh").chmod(0o755)


def comment(body, minute):
    return {"body": body, "createdAt": f"2026-10-03T12:{minute:02d}:00Z",
            "url": f"{issue_url}#issuecomment-{minute}", "author": {"login": "agent"}}


def dispatch(event, body, minute, attrs=""):
    return comment(f"<!-- aoc-dispatch run_id={run_id} event={event}{attrs} -->\n\n{body}", minute)


claim = dispatch("claim", "Claimed for code mode. The master works on branch `aoc/issue-9`.", 1)
blocker = dispatch("progress", "**Blocker**\n\n**State:** Waiting\n**Change:** Found missing token\n"
                   "**Evidence:** gh authentication failed\n**Next:** Supply token\n**Blockers:** Missing token",
                   2, " kind=blocker seq=1")
unblocked = dispatch("progress", "**Blocker resolved**\n\n**State:** Running\n**Change:** Token supplied",
                     3, " kind=unblocked seq=2")
commit = comment(f"<!-- aoc-journal event=progress kind=commit -->\n\n**Commit**\n\n"
                 f"**State:** Ready\n**Change:** Added journal\n**Evidence:** commit `{sha}`; smoke passed", 4)
completion = dispatch("result", f"Journal ready\n\n## Decisions\nUse gh.\n\n## Tests\nSmoke passed\n\n"
                      f"## References\nPull request: {pr_url}\nBranch: `aoc/issue-9`\nHEAD: `{head}`\n"
                      f"Commits:\n- `{sha[:12]}` Add journal\n- `{short}` Test journal\n\n"
                      "## Follow-up\nNone reported.\n\n## Blockers\nNone remain.", 5)
review = comment(f"<!-- aoc-journal event=progress kind=review -->\n\n**Review addressed**\n\n"
                 f"**State:** Ready\n**Change:** Review resolved\n**Evidence:** commit `{sha}`; {pr_url}", 6)
base_issue = {"number": 9, "title": "Issue journal", "body": "Intro paragraph.\n\n## Goal\n"
              "Build a compact\nissue snapshot.\n\n## Acceptance\nDo not include this.",
              "state": "OPEN", "labels": [{"name": "agent-review"}, {"name": "feature"}],
              "comments": [claim, blocker, unblocked, commit, completion, review,
                           comment(f"Ordinary comment: HEAD: `{ 'd4' * 20 }` {pr_url}/99", 7),
                           comment("Text before marker\n<!-- aoc-journal event=follow-up -->\nIgnored", 8)],
              "url": issue_url, "closedAt": None}


def reset(issue=None, **options):
    state_file.write_text(json.dumps({"repo": repo, "issues": {"9": copy.deepcopy(issue or base_issue)},
                                     "log": [], **options}), encoding="utf-8")


def saved():
    return json.loads(state_file.read_text(encoding="utf-8"))


def invoke(*args, overrides=None):
    return subprocess.run([str(cli), *args], env={**env, **(overrides or {})},
                          capture_output=True, text=True, check=False)


def success(*args):
    result = invoke(*args)
    assert result.returncode == 0, (args, result.returncode, result.stderr)
    assert not result.stderr, result.stderr
    return result.stdout


def failure(message, *args, code=1, overrides=None):
    before = saved()
    result = invoke(*args, overrides=overrides)
    assert result.returncode == code, (args, result.returncode, result.stderr)
    assert message in result.stderr, (message, result.stderr)
    if code == 1:
        assert result.stderr.startswith("aoc-journal: "), result.stderr
    assert not result.stdout, result.stdout
    assert saved()["issues"] == before["issues"], "failed command mutated issues"
    return result


def snapshot(issue=None, events=None):
    reset(issue)
    args = ["state", "9", "--repo", repo, "--json"]
    if events is not None:
        args += ["--events", str(events)]
    return json.loads(success(*args))


value = snapshot()
assert value == {
    "schema": "aoc.issue.state/v1", "issue": 9, "url": issue_url, "title": "Issue journal",
    "objective": "Build a compact issue snapshot.", "state": "review", "labels": ["agent-review", "feature"],
    "closedAt": None, "events": value["events"], "blockers": [],
    "commits": [sha, head, sha[:12], short], "pull_requests": [pr_url],
    "completion": {"complete": True, "url": completion["url"], "at": completion["createdAt"]},
}, value
assert [event["event"] for event in value["events"]] == ["claim", "progress", "progress", "progress", "result", "progress"]
assert value["events"][1] == {
    "source": "dispatch", "event": "progress", "kind": "blocker", "run_id": run_id, "seq": 1,
    "at": blocker["createdAt"], "url": blocker["url"], "author": "agent", "state": "Waiting",
    "change": "Found missing token", "evidence": "gh authentication failed", "next": "Supply token",
    "blockers": "Missing token", "summary": "**State:** Waiting",
}, value["events"][1]
assert value["events"][0]["summary"] == "Claimed for code mode. The master works on branch `aoc/issue-9`."
assert all(value["events"][0][key] is None for key in ("kind", "seq", *["state", "change", "evidence", "next", "blockers"]))
assert value["events"][3]["source"] == "journal" and value["events"][3]["run_id"] is None
assert value["events"][4]["summary"] == "Journal ready"
assert snapshot(events=2)["events"] == value["events"][-2:]
zero = snapshot(events=0)
assert zero["events"] == [] and zero["commits"] == value["commits"] and zero["completion"] == value["completion"]
assert snapshot({**base_issue, "comments": list(reversed(base_issue["comments"]))}) == value

for label, expected in (("agent-running", "running"), ("needs-alex", "needs-decision"),
                        ("agent-review", "review"), ("agent-failed", "failed"), ("agent-ready", "ready")):
    mapped = snapshot({**base_issue, "labels": [{"name": label}]})
    assert mapped["state"] == expected, mapped
assert snapshot({**base_issue, "labels": []})["state"] == "open"
all_labels = [{"name": label} for label in ("agent-ready", "agent-failed", "agent-review", "needs-alex", "agent-running")]
assert snapshot({**base_issue, "labels": all_labels})["state"] == "running"
closed_issue = {**base_issue, "state": "CLOSED", "closedAt": "2026-10-03T12:30:00Z", "labels": all_labels}
closed = snapshot(closed_issue)
assert closed["state"] == "closed" and closed["closedAt"] == closed_issue["closedAt"]
assert snapshot({**base_issue, "comments": [claim, blocker]}, events=0)["blockers"] == ["Missing token"]
blocked = dispatch("blocked", "Acceptance unclear\n\nWhich target?", 9)
assert snapshot({**base_issue, "comments": [blocked]})["blockers"] == ["Acceptance unclear"]
assert snapshot({**base_issue, "comments": [blocker, unblocked]})["blockers"] == []
assert snapshot({**base_issue, "comments": [blocker, {**claim, "createdAt": "2026-10-03T12:10:00Z"}]})["blockers"] == []
assert snapshot({**base_issue, "labels": [{"name": "needs-alex"}]})["blockers"] == ["needs-alex label is set"]
assert snapshot({**base_issue, "labels": [{"name": "needs-alex"}], "comments": [blocker]})["blockers"] == ["Missing token"]
for event in ("claim", "blocked", "failed"):
    newer = dispatch(event, "New run", 10)
    assert snapshot({**base_issue, "comments": [completion, newer]})["completion"] == {"complete": False, "url": None, "at": None}
dry_result = dispatch("result", "Plan ready\n\nNo code changes.", 11)
assert snapshot({**base_issue, "comments": [dry_result]})["completion"] == {"complete": False, "url": None, "at": None}
assert snapshot({**base_issue, "comments": []})["completion"] == {"complete": False, "url": None, "at": None}
assert snapshot({**base_issue, "body": "\n\nFirst\nparagraph café.\n\nSecond paragraph."})["objective"] == "First paragraph café."
assert snapshot({**base_issue, "body": ""})["objective"] == ""
assert snapshot({**base_issue, "body": "## Goal\n" + "é" * 501 + "\n### Detail\nExcluded"})["objective"] == "é" * 499 + "…"
assert snapshot({**base_issue, "body": "x" * 500})["objective"] == "x" * 500
long_summary = comment("<!-- aoc-journal event=follow-up -->\n\n" + "x" * 201, 12)
assert snapshot({**base_issue, "comments": [long_summary]})["events"][0]["summary"] == "x" * 199 + "…"

reset()
text = success("state", "9", "--events", "2")
assert text == (f"Issue: #9 Issue journal {issue_url}\nObjective: Build a compact issue snapshot.\nState: review\n"
                f"Blockers: None\nCommits: {sha}, {head}, {sha[:12]}, {short}\nPRs: {pr_url}\n"
                f"Completion: complete {completion['url']} {completion['createdAt']}\nEvents:\n"
                "- 2026-10-03T12:05:00Z result: Journal ready\n"
                "- 2026-10-03T12:06:00Z review: Review resolved\n"), text
assert saved()["log"][0]["argv"] == ["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"]

post_args = ["post", "9", "--repo", repo, "--kind", "checkpoint", "--state", "Ready", "--change", "Code ready"]
reset()
url = success(*post_args, "--evidence", " smoke \n\tpassed café ", "--next", " review\r\nnow ",
              "--blockers", " none ", "--commit", sha)
posted = saved()["issues"]["9"]["comments"][-1]
assert url == posted["url"] + "\n"
assert posted["body"] == ("<!-- aoc-journal event=progress kind=checkpoint -->\n\n**Checkpoint**\n\n"
                          "**State:** Ready\n**Change:** Code ready\n"
                          f"**Evidence:** commit `{sha}`; smoke passed café\n**Next:** review now\n**Blockers:** none"), posted
assert not posted["body"].endswith("\n")
assert saved()["log"][0]["argv"] == ["issue", "view", "9", "--repo", repo, "--json", "state"]

kinds = (("discovery", "Discovery"), ("decision", "Decision"), ("scope", "Scope change"),
         ("checkpoint", "Checkpoint"), ("blocker", "Blocker"), ("unblocked", "Blocker resolved"),
         ("validation", "Validation"), ("commit", "Commit"), ("review", "Review addressed"),
         ("deploy", "Deployment"), ("regression", "Regression"), ("correction", "Correction"), ("incident", "Incident"))
for kind, label in kinds:
    reset()
    args = ["post", "9", "--kind", kind, "--state", " Ready\n\tfor  review ", "--change", " Changed "]
    if kind == "blocker":
        args += ["--blockers", "Token"]
    success(*args)
    expected = (f"<!-- aoc-journal event=progress kind={kind} -->\n\n**{label}**\n\n"
                "**State:** Ready for  review\n**Change:** Changed" +
                ("\n**Blockers:** Token" if kind == "blocker" else ""))
    assert saved()["issues"]["9"]["comments"][-1]["body"] == expected

reset()
success(*post_args, "--commit", sha, "--evidence", " \n\t", "--next", "", "--blockers", "")
assert saved()["issues"]["9"]["comments"][-1]["body"] == (
    "<!-- aoc-journal event=progress kind=checkpoint -->\n\n**Checkpoint**\n\n"
    f"**State:** Ready\n**Change:** Code ready\n**Evidence:** commit `{sha}`")
for output in ("", "Comment posted\nnot a URL\n"):
    reset(comment_output=output)
    assert success(*post_args) == issue_url + "\n"

for kind, _ in kinds:
    reset(closed_issue)
    args = ["post", "9", "--repo", repo, "--kind", kind, "--state", "Closed", "--change", "Observed"]
    if kind == "blocker":
        args += ["--blockers", "Token"]
    if kind in ("deploy", "regression", "correction", "incident"):
        success(*args)
        assert saved()["issues"]["9"]["comments"][-1]["body"].startswith(f"<!-- aoc-journal event=progress kind={kind} -->")
    else:
        failure("issue #9 is closed; record new scope with: aoc-journal follow-up 9 --title TITLE", *args)
        assert [entry["argv"][:2] for entry in saved()["log"]] == [["issue", "view"]]

for field, limit in (("state", 500), ("change", 500), ("next", 500), ("blockers", 500), ("evidence", 1000)):
    reset()
    success(*post_args, "--" + field, "é" * limit)
    assert f"**{field.title()}:** " + "é" * limit in saved()["issues"]["9"]["comments"][-1]["body"]
    reset()
    failure(f"{field} must be at most {limit} characters", *post_args, "--" + field, "é" * (limit + 1))
    assert saved()["log"] == []
for field in ("state", "change"):
    for empty in ("", " \n\t"):
        reset()
        failure(f"{field} must be non-empty", *post_args, "--" + field, empty)
for invalid in ("", "a" * 39, "a" * 41, "g" * 40, sha + "\n"):
    reset()
    failure("commit must be a 40-hex SHA", *post_args, "--commit", invalid)
reset()
failure("kind blocker requires non-empty blockers", *post_args, "--kind", "blocker", "--blockers", " \n\t")
failure("invalid choice", *post_args, "--kind", "unknown", code=2)
failure("required", "post", "9", "--kind", "checkpoint", code=2)
failure("must be non-negative", "state", "9", "--events", "-1", code=2)
failure("invalid nonnegative value", "state", "9", "--events", "many", code=2)
failure("repo must be an owner/name string", "state", "9", "--repo", "wrong")

for original in (base_issue, closed_issue):
    reset(original)
    url = success("follow-up", "9", "--repo", repo, "--title", "Additional scope", "--body", "New goal\n\ncafé")
    state = saved()
    created = state["issues"]["10"]
    assert url == created["url"] + "\n"
    assert created["title"] == "Additional scope" and created["body"] == "Follow-up to #9.\n\nNew goal\n\ncafé"
    assert state["issues"]["9"]["comments"][-1]["body"] == (
        f"<!-- aoc-journal event=follow-up -->\n\nNew scope moved to {created['url']}.")
    assert [entry["argv"][:2] for entry in state["log"]] == [["issue", "create"], ["issue", "comment"]]

body_file = tmp / "body with spaces.txt"
body_file.write_text("File goal café\n", encoding="utf-8")
reset()
success("follow-up", "9", "--title", "File scope", "--body-file", str(body_file))
assert saved()["issues"]["10"]["body"] == "Follow-up to #9.\n\nFile goal café\n"
assert saved()["log"][0]["argv"][:2] == ["repo", "view"]
reset()
success("follow-up", "9", "--repo", repo, "--title", "No body")
assert saved()["issues"]["10"]["body"] == "Follow-up to #9."
reset()
failure("not allowed with argument --body", "follow-up", "9", "--title", "Scope",
        "--body", "Text", "--body-file", str(body_file), code=2)
failure("No such file", "follow-up", "9", "--repo", repo, "--title", "Scope",
        "--body-file", str(tmp / "absent"))
reset(create_output="Created\nnot a URL\n")
result = invoke("follow-up", "9", "--repo", repo, "--title", "Missing URL")
assert result.returncode == 1 and result.stderr == "aoc-journal: gh issue create returned no issue URL\n", result
assert not result.stdout
assert [entry["argv"][:2] for entry in saved()["log"]] == [["issue", "create"]]
assert saved()["issues"]["9"]["comments"] == base_issue["comments"]

reset(error="fixture GitHub unavailable")
failure("fixture GitHub unavailable", "state", "9", "--repo", repo)
reset()
failure("No such file", "state", "9", "--repo", repo,
        overrides={"AOC_JOURNAL_GH_BIN": str(tmp / "absent-gh")})
print("AOC Journal smoke passed")
PY
