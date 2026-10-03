#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

python3 - "$root/bin/aoc-progress" "$tmp_dir" <<'PY'
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

cli, tmp = Path(sys.argv[1]), Path(sys.argv[2]).resolve()
env = {k: v for k, v in os.environ.items() if not k.startswith(("AOC_DISPATCH_", "GIT_"))}
run_dir = tmp / "run with spaces"
run_dir.mkdir()
meta = {"run_id": "2-fixture", "repo": "fixture/progress", "issue": 2, "mode": "dry-run"}
(run_dir / "meta.json").write_text(json.dumps(meta))
base = ["--run-dir", str(run_dir), "--kind", "discovery", "--state", "Found", "--change", "Plan changed"]


def invoke(*args, overrides=None):
    return subprocess.run([str(cli), *args], env={**env, **(overrides or {})},
                          capture_output=True, text=True, cwd=tmp)


def success(*args, overrides=None):
    result = invoke(*args, overrides=overrides)
    assert result.returncode == 0 and not result.stderr, (args, result.stderr)
    return result.stdout


def failure(message, *args, overrides=None):
    before = (run_dir / "progress.jsonl").read_bytes() if (run_dir / "progress.jsonl").exists() else None
    result = invoke(*args, overrides=overrides)
    assert result.returncode == 2 and message in result.stderr and not result.stdout, (args, result.stderr)
    after = (run_dir / "progress.jsonl").read_bytes() if (run_dir / "progress.jsonl").exists() else None
    assert before == after


before = datetime.now(timezone.utc).replace(microsecond=0)
assert success(*base) == f"{run_dir / 'progress.jsonl'}\n"
entry = json.loads((run_dir / "progress.jsonl").read_text())
assert entry == {"schema": "aoc.dispatch.progress/v1", "run_id": meta["run_id"], "repo": meta["repo"],
                 "issue": 2, "seq": 1, "kind": "discovery", "state": "Found", "change": "Plan changed",
                 "evidence": None, "next": None, "blockers": None, "commit": None, "timestamp": entry["timestamp"]}
stamp = datetime.strptime(entry["timestamp"], "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
assert before <= stamp <= datetime.now(timezone.utc)
failure("duplicate progress transition", *base, "--evidence", "New evidence does not change transition")
entry = json.loads(success(*base, "--kind", "decision", "--state", "  A \n\t B  ",
                           "--change", " C\r\n D ", "--evidence", " E  \n F ",
                           "--next", " G\n H ", "--blockers", " I\n J ", "--json"))
assert entry["seq"] == 2 and entry["state"] == "A B" and entry["change"] == "C D"
assert (entry["evidence"], entry["next"], entry["blockers"]) == ("E F", "G H", "I J")
for field in ("state", "change", "evidence", "next", "blockers"):
    limit = 1000 if field == "evidence" else 500
    success(*base, "--" + field, "é" * limit, "--change", field if field != "change" else "é" * limit)
    failure(f"--{field} must be at most {limit}", *base, "--" + field, "é" * (limit + 1))
for field in ("state", "change"):
    for value in ("", " \n\t"):
        failure(f"--{field} must be non-empty", *base, "--" + field, value)
for value in (None, "", " \n"):
    failure("blocker requires non-empty --blockers", *base, "--kind", "blocker",
            *(["--blockers", value] if value is not None else []))
success(*base, "--kind", "blocker", "--blockers", "Need access")
for sha in ("", "a" * 39, "a" * 41, "g" * 40, "a" * 40 + "\n"):
    failure("--commit must be a 40-hex SHA", *base, "--commit", sha)
failure("commit requires a resolvable", *base, "--kind", "commit")
sha = "aB" * 20
entry = json.loads(success(*base, "--kind", "commit", "--commit", sha, "--json"))
assert entry["commit"] == sha
for required in ("kind", "state", "change"):
    args = [arg for index, arg in enumerate(base) if index not in (base.index("--" + required), base.index("--" + required) + 1)]
    failure("--" + required, *args)
failure("invalid choice", *base, "--kind", "invalid")
failure("--run-dir or AOC_DISPATCH_RUN_DIR is required", *base[2:])
failure("run directory does not exist", *base, "--run-dir", str(tmp / "missing"))
failure("not a directory", *base, "--run-dir", str(run_dir / "meta.json"))
empty = tmp / "empty"
empty.mkdir()
fallback = {"AOC_DISPATCH_RUN_DIR": str(empty), "AOC_DISPATCH_RUN_ID": "fallback",
            "AOC_DISPATCH_REPO": "fixture/fallback", "AOC_DISPATCH_ISSUE": "7"}
entry = json.loads(success(*base[2:], "--json", overrides=fallback))
assert (entry["run_id"], entry["repo"], entry["issue"]) == ("fallback", "fixture/fallback", 7)
for key, message in (("AOC_DISPATCH_RUN_ID", "run_id is required"), ("AOC_DISPATCH_REPO", "repo is required"),
                     ("AOC_DISPATCH_ISSUE", "must be an integer")):
    failure(message, *base[2:], overrides={k: v for k, v in fallback.items() if k != key})
failure("must be an integer", *base[2:], overrides={**fallback, "AOC_DISPATCH_ISSUE": "two"})
for contents, message in (("{", "invalid meta.json"), ("[]", "must contain an object"),
                          (json.dumps({**meta, "issue": True}), "issue in meta.json must be an integer"),
                          (json.dumps({**meta, "issue": "2"}), "issue in meta.json must be an integer"),
                          (json.dumps({**meta, "run_id": " "}), "run_id is required"),
                          (json.dumps({**meta, "repo": None}), "repo is required")):
    (empty / "meta.json").write_text(contents)
    failure(message, *base, "--run-dir", str(empty), overrides=fallback)
git_dir = tmp / "git worktree"
git_dir.mkdir()


def git(*args):
    result = subprocess.run(["git", *args], cwd=git_dir, env=env, capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


git("init", "-b", "fixture")
(git_dir / "file.txt").write_text("fixture\n")
git("add", "file.txt")
git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false",
    "-c", f"core.hooksPath={tmp / 'no-hooks'}", "commit", "-m", "Fixture")
(run_dir / "meta.json").write_text(json.dumps({**meta, "mode": "code"}))
entry = json.loads(success(*base, "--kind", "commit", "--change", "Committed", "--json",
                           overrides={"AOC_DISPATCH_WORKTREE": str(git_dir)}))
assert entry["commit"] == git("rev-parse", "HEAD")
failure("commit requires a resolvable", *base, "--kind", "commit", overrides={"AOC_DISPATCH_WORKTREE": str(tmp)})
(run_dir / "meta.json").write_text(json.dumps(meta))
for kind in ("scope", "checkpoint", "unblocked", "validation", "review", "deploy", "regression", "correction", "incident"):
    success(*base, "--kind", kind)
with ThreadPoolExecutor(max_workers=4) as pool:
    list(pool.map(lambda n: success(*base, "--change", f"Concurrent {n}"), range(3)))
entries = [json.loads(line) for line in (run_dir / "progress.jsonl").read_text().splitlines()]
assert [e["seq"] for e in entries] == list(range(1, len(entries) + 1))
for n in range(len(entries), 25):
    success(*base, "--change", f"Milestone {n}")
failure("progress journal limit reached (25); record only meaningful transitions", *base, "--change", "Overflow")
recovery = tmp / "recovery"
recovery.mkdir()
(recovery / "meta.json").write_text(json.dumps(meta))
journal = recovery / "progress.jsonl"
prefix = json.dumps(entries[0]) + "\n"
for tail in ('{"schema":', "not-json\n", "[]\n", "null\n"):
    journal.write_text(prefix + tail)
    entry = json.loads(success(*base, "--run-dir", str(recovery), "--json"))
    contents = journal.read_text()
    lines = contents.splitlines()
    assert contents.endswith("\n") and lines[:2] == [prefix.rstrip("\n"), tail.rstrip("\n")]
    assert entry["seq"] == 3 and json.loads(lines[2]) == entry and len(lines) == 3
for tail in ('{"schema":', "not-json\n"):
    journal.write_text(prefix * 24 + tail)
    result = invoke(*base, "--run-dir", str(recovery), "--json")
    assert result.returncode == 2 and "progress journal limit reached (25)" in result.stderr
    assert not result.stdout and len(journal.read_text().splitlines()) == 25
PY

printf 'AOC progress smoke passed\n'
