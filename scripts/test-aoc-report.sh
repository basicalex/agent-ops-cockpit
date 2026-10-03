#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

python3 - "$root/bin/aoc-report" "$tmp_dir" <<'PY'
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys

cli = Path(sys.argv[1])
tmp = Path(sys.argv[2]).resolve()
env = {key: value for key, value in os.environ.items() if not key.startswith("AOC_DISPATCH_")}
run_id = "2-20261001T121500Z-a3f9"
run_dir = tmp / "run with spaces"
run_dir.mkdir()
meta = {
    "schema": "aoc.dispatch.meta/v1",
    "run_id": run_id,
    "repo": "basicalex/agent-ops-cockpit",
    "issue": 2,
    "root": str(tmp),
    "mode": "dry-run",
    "continuation_of": None,
    "decision_id": None,
    "created_at": "2026-10-01T12:15:00Z",
}
(run_dir / "meta.json").write_text(json.dumps(meta), encoding="utf-8")
report_path = run_dir / "report.json"


def invoke(*args, overrides=None, cwd=None):
    result = subprocess.run(
        [str(cli), *args], env={**env, **(overrides or {})},
        capture_output=True, text=True, check=False, cwd=cwd,
    )
    return result


def success(*args, overrides=None, cwd=None):
    result = invoke(*args, overrides=overrides, cwd=cwd)
    assert result.returncode == 0, (args, result.returncode, result.stderr)
    assert not result.stderr, result.stderr
    return result.stdout


def snapshot(directory):
    if not directory.exists():
        return None
    return {str(path.relative_to(directory)): path.read_bytes()
            for path in directory.rglob("*") if path.is_file()}


def failure(message, *args, directory=run_dir, overrides=None):
    before = snapshot(directory)
    result = invoke(*args, overrides=overrides)
    assert result.returncode == 2, (args, result.returncode, result.stderr)
    assert message in result.stderr, (message, result.stderr)
    assert not result.stdout, result.stdout
    assert snapshot(directory) == before, (args, "validation changed files")


before = datetime.now(timezone.utc).replace(microsecond=0)
output = success("--run-dir", str(run_dir), "--status", "done", "--summary", "Plan ready",
                 "--evidence", "Reviewed issue #2",
                 overrides={"AOC_DISPATCH_RUN_ID": "wrong", "AOC_DISPATCH_REPO": "wrong/repo",
                            "AOC_DISPATCH_ISSUE": "999", "AOC_DISPATCH_RUN_DIR": str(tmp / "wrong")})
assert output == f"{report_path}\n", output
report = json.loads(report_path.read_text(encoding="utf-8"))
assert report == {
    "schema": "aoc.dispatch.report/v1", "version": 1, "run_id": run_id,
    "assignmentId": run_id, "repo": meta["repo"], "issue": 2, "status": "done",
    "summary": "Plan ready", "needsDecision": None, "decision_id": None,
    "evidence": "Reviewed issue #2", "timestamp": report["timestamp"],
    "branch": None, "commit": None, "tests": None,
    "decisions": None, "followUp": None,
}, report
stamp = datetime.strptime(report["timestamp"], "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
assert before <= stamp <= datetime.now(timezone.utc), report

# Repeated calls replace the record, including fields from an earlier blocked report.
success("--run-dir", str(run_dir), "--status", "blocked", "--summary", "Acceptance unclear",
        "--needs-decision", "Which branch is the target?")
report = json.loads(report_path.read_text(encoding="utf-8"))
assert report["status"] == "blocked", report
assert report["needsDecision"] == "Which branch is the target?", report
assert report["decision_id"] == f"{run_id}-d1", report
assert report["evidence"] is None, report

output = success("--run-dir", str(run_dir), "--status", "failed", "--summary", "Cannot read source", "--json")
report = json.loads(report_path.read_text(encoding="utf-8"))
assert json.loads(output) == report, output
assert report["status"] == "failed" and report["summary"] == "Cannot read source", report
assert report["needsDecision"] is None and report["decision_id"] is None, report

# Environment fallback is only used when meta.json is absent.
fallback_dir = tmp / "fallback"
fallback_dir.mkdir()
fallback_env = {
    "AOC_DISPATCH_RUN_DIR": str(fallback_dir), "AOC_DISPATCH_RUN_ID": "7-20261001T121600Z-b4e1",
    "AOC_DISPATCH_REPO": "owner/project", "AOC_DISPATCH_ISSUE": "7",
}
output = success("--status", "done", "--summary", "Fallback ready", "--json", overrides=fallback_env)
fallback = json.loads(output)
assert fallback == json.loads((fallback_dir / "report.json").read_text()), fallback
assert fallback["run_id"] == fallback_env["AOC_DISPATCH_RUN_ID"], fallback
assert fallback["assignmentId"] == fallback["run_id"], fallback
assert fallback["repo"] == "owner/project" and fallback["issue"] == 7, fallback

assert all(fallback[key] is None for key in ("branch", "commit", "tests")), fallback

# Explicit code fields also work in dry-run; omitted fields never retain old values.
explicit_commit = "aB" * 20
output = success("--run-dir", str(run_dir), "--status", "done", "--summary", "Code ready",
                 "--branch", "aoc/issue-2-report", "--commit", explicit_commit,
                 "--tests", "python3 smoke.py\nPassed café case", "--json")
report = json.loads(output)
assert report["branch"] == "aoc/issue-2-report", report
assert report["commit"] == explicit_commit, report
assert report["tests"] == "python3 smoke.py\nPassed café case", report
success("--run-dir", str(run_dir), "--status", "done", "--summary", "Dry-run ready")
report = json.loads(report_path.read_text())
assert all(report[key] is None for key in ("branch", "commit", "tests")), report

for value in ("Use the existing lock", "é" * 1200, ""):
    output = success("--run-dir", str(run_dir), "--status", "done", "--summary", "Ready",
                     "--decisions", value, "--follow-up", value, "--json")
    report = json.loads(output)
    assert report["decisions"] == value and report["followUp"] == value, report
success("--run-dir", str(run_dir), "--status", "done", "--summary", "Ready")
report = json.loads(report_path.read_text())
assert report["decisions"] is None and report["followUp"] is None, report

# Code mode reads the caller's checkout, not the run directory; flags override independently.
git_dir = tmp / "worktree with spaces"
git_dir.mkdir()
git_env = {key: value for key, value in env.items() if not key.startswith("GIT_")}


def git(*args):
    result = subprocess.run(["git", *args], cwd=git_dir, env=git_env,
                            capture_output=True, text=True, check=False)
    assert result.returncode == 0, (args, result.stderr)
    return result.stdout.strip()


git("init", "--initial-branch=aoc/issue-2-autofill")
(git_dir / "source.txt").write_text("Report fixture\n")
git("add", "source.txt")
git("-c", "user.name=AOC Test", "-c", "user.email=aoc-test@example.invalid",
    "-c", "commit.gpgsign=false", "-c", f"core.hooksPath={tmp / 'no-hooks'}",
    "commit", "-m", "Create report fixture")
head = git("rev-parse", "HEAD")
success("--run-dir", str(run_dir), "--status", "done", "--summary", "Dry-run in git",
        cwd=git_dir)
report = json.loads(report_path.read_text())
assert report["branch"] is None and report["commit"] is None, report
(run_dir / "meta.json").write_text(json.dumps({**meta, "mode": "code"}))
output = success("--run-dir", str(run_dir), "--status", "done", "--summary", "Auto-filled",
                 "--json", cwd=git_dir)
report = json.loads(output)
assert report["branch"] == "aoc/issue-2-autofill" and report["commit"] == head, report
assert report["tests"] is None, report
output = success("--run-dir", str(run_dir), "--status", "done", "--summary", "Branch override",
                 "--branch", "aoc/issue-2-explicit", "--json", cwd=git_dir)
report = json.loads(output)
assert report["branch"] == "aoc/issue-2-explicit" and report["commit"] == head, report
output = success("--run-dir", str(run_dir), "--status", "done", "--summary", "Commit override",
                 "--commit", explicit_commit, "--json", cwd=git_dir)
report = json.loads(output)
assert report["branch"] == "aoc/issue-2-autofill" and report["commit"] == explicit_commit, report
output = success("--run-dir", str(run_dir), "--status", "done", "--summary", "No git checkout",
                 "--json", cwd=tmp)
report = json.loads(output)
assert report["branch"] is None and report["commit"] is None, report
(run_dir / "meta.json").write_text(json.dumps(meta))

# Tests text follows evidence semantics: Unicode character limit and verbatim file content.
tests_file = tmp / "tests.txt"
tests = "python3 smoke.py\nPassed café case\n"
tests_file.write_text(tests, encoding="utf-8")
success("--run-dir", str(run_dir), "--status", "done", "--summary", "Tests ready",
        "--tests-file", str(tests_file))
assert json.loads(report_path.read_text())["tests"] == tests
success("--run-dir", str(run_dir), "--status", "done", "--summary", "Tests boundary",
        "--tests", "é" * 4000)
assert json.loads(report_path.read_text())["tests"] == "é" * 4000
tests_file.write_text("é" * 4000, encoding="utf-8")
success("--run-dir", str(run_dir), "--status", "done", "--summary", "Tests file boundary",
        "--tests-file", str(tests_file))
assert json.loads(report_path.read_text())["tests"] == "é" * 4000

# Character limits include the boundary; evidence-file preserves multiline Unicode text.
evidence_file = tmp / "evidence.txt"
evidence = "Read café\nReviewed diff\n"
evidence_file.write_text(evidence, encoding="utf-8")
success("--run-dir", str(run_dir), "--status", "done", "--summary", "Evidence ready",
        "--evidence-file", str(evidence_file))
assert json.loads(report_path.read_text())["evidence"] == evidence
success("--run-dir", str(run_dir), "--status", "blocked", "--summary", "é" * 1200,
        "--needs-decision", "?" * 1200, "--evidence", "é" * 4000)
report = json.loads(report_path.read_text())
assert report["summary"] == "é" * 1200 and report["evidence"] == "é" * 4000, report
assert report["needsDecision"] == "?" * 1200, report

# Each rejected call leaves an existing report byte-for-byte intact and no temporary files.
base = ["--run-dir", str(run_dir), "--status", "done"]
for commit in ("", "a" * 39, "a" * 41, "g" * 40, "a" * 40 + "\n"):
    failure("--commit must be a 40-hex SHA", *base, "--summary", "Ready", "--commit", commit)
for name in ("decisions", "follow-up"):
    failure(f"--{name} must be at most 1200", *base, "--summary", "Ready", "--" + name, "é" * 1201)
failure("tests must be at most 4000", *base, "--summary", "Ready", "--tests", "é" * 4001)
tests_file.write_text("é" * 4001, encoding="utf-8")
failure("tests must be at most 4000", *base, "--summary", "Ready",
        "--tests-file", str(tests_file))
failure("not allowed with argument --tests", *base, "--summary", "Ready",
        "--tests", "Direct", "--tests-file", str(tests_file))
failure("No such file", *base, "--summary", "Ready", "--tests-file", str(tmp / "absent-tests.txt"))
failure("--summary must be non-empty", *base, "--summary", "")
failure("--summary must be non-empty", *base, "--summary", " \n\t")
failure("--summary must be at most 1200", *base, "--summary", "x" * 1201)
failure("--needs-decision must be at most 1200", "--run-dir", str(run_dir),
        "--status", "blocked", "--summary", "Blocked", "--needs-decision", "x" * 1201)
failure("evidence must be at most 4000", *base, "--summary", "Ready", "--evidence", "x" * 4001)
evidence_file.write_text("x" * 4001)
failure("evidence must be at most 4000", *base, "--summary", "Ready",
        "--evidence-file", str(evidence_file))
failure("blocked status requires non-empty --needs-decision", "--run-dir", str(run_dir),
        "--status", "blocked", "--summary", "Blocked")
for question in ("", " \n"):
    failure("blocked status requires non-empty --needs-decision", "--run-dir", str(run_dir),
            "--status", "blocked", "--summary", "Blocked", "--needs-decision", question)
for status in ("done", "failed"):
    failure("--needs-decision is only valid with blocked status", "--run-dir", str(run_dir),
            "--status", status, "--summary", "Ready", "--needs-decision", "Question?")
failure("not allowed with argument --evidence", *base, "--summary", "Ready",
        "--evidence", "Direct", "--evidence-file", str(evidence_file))
failure("No such file", *base, "--summary", "Ready", "--evidence-file", str(tmp / "absent.txt"))
failure("invalid choice", "--run-dir", str(run_dir), "--status", "invalid", "--summary", "Ready")
failure("--summary", "--run-dir", str(run_dir), "--status", "done")
failure("--status", "--run-dir", str(run_dir), "--summary", "Ready")
failure("--run-dir or AOC_DISPATCH_RUN_DIR is required", "--status", "done", "--summary", "Ready")
missing_dir = tmp / "missing-run"
failure("run directory does not exist", "--run-dir", str(missing_dir), "--status", "done",
        "--summary", "Ready", directory=missing_dir, overrides=fallback_env)
assert not missing_dir.exists()
failure("not a directory", "--run-dir", str(evidence_file), "--status", "done", "--summary", "Ready")

# An absent identity or malformed meta never silently falls back to different metadata.
empty_dir = tmp / "empty"
empty_dir.mkdir()
failure("AOC_DISPATCH_ISSUE is required", "--run-dir", str(empty_dir), "--status", "done",
        "--summary", "Ready", directory=empty_dir)
for key, message in (("AOC_DISPATCH_RUN_ID", "run_id is required"),
                     ("AOC_DISPATCH_REPO", "repo is required"),
                     ("AOC_DISPATCH_ISSUE", "AOC_DISPATCH_ISSUE is required")):
    incomplete = {name: value for name, value in fallback_env.items() if name != key}
    failure(message, "--run-dir", str(empty_dir), "--status", "done", "--summary", "Ready",
            directory=empty_dir, overrides=incomplete)
failure("must be an integer", "--run-dir", str(empty_dir), "--status", "done", "--summary", "Ready",
        directory=empty_dir, overrides={**fallback_env, "AOC_DISPATCH_ISSUE": "two"})
for contents, message in (("{", "invalid meta.json"), ("[]", "must contain an object"),
                          (json.dumps({**meta, "issue": "2"}), "issue in meta.json must be an integer"),
                          (json.dumps({**meta, "issue": True}), "issue in meta.json must be an integer"),
                          (json.dumps({**meta, "run_id": ""}), "run_id is required"),
                          (json.dumps({**meta, "repo": None}), "repo is required")):
    (empty_dir / "meta.json").write_text(contents)
    failure(message, "--run-dir", str(empty_dir), "--status", "done", "--summary", "Ready",
            directory=empty_dir, overrides=fallback_env)
assert not (empty_dir / "report.json").exists()
assert not list(tmp.rglob("*.tmp")), "temporary report files were left behind"
PY

printf 'AOC report smoke passed\n'
