---
name: herdr-orchestrate
description: Orchestrate parallel work across omp worker agents in herdr panes — spawn or discover workers, dispatch assignment packets, monitor liveness, and verify results before committing. Use whenever a code change should be delegated instead of implemented firsthand (delegation-first policy), e.g. /herdr-orchestrate 3 to spawn three workers, or /herdr-orchestrate p17,p18,p19 to use existing panes.
argument-hint: "[N workers to spawn | comma-separated pane IDs or tab names]"
---

# herdr-orchestrate

Coordinate a campaign of parallel work across omp agents running in herdr panes. You are the **orchestrator**: you design the work contracts and verify the results. The workers only execute. The judgment work — designing contracts, splitting scopes into non-overlapping file sets, verifying diffs — is yours; this skill only encodes the spawn/dispatch/monitor/verify *protocol*.

**Slice routing rule:** omp workers handle mechanical/parallelizable slices only (sweeps, migrations, typecheck fixes, audits). Visual/design slices — UI polish, component composition, spacing/tone/palette, anything where "does this look right" is the acceptance criterion — must NOT be packeted to omp workers; dispatch those via the Agent tool with `model: "opus"` instead. When splitting a campaign, separate visual slices from mechanical slices up front.

## Project parameters

Before dispatching, resolve these from the project's CLAUDE.md / AGENTS.md (or ask the user if absent):

- **TEST_CMD** — the project's test command
- **TYPECHECK_CMD** — the project's typecheck/lint command
- **Guard conventions** — any repo-specific rules workers must follow

Bake these into every assignment packet.

## 1. Worker acquisition

The skill argument decides the mode:

- **A number** (e.g. `/herdr-orchestrate 3`) → **spawn** that many omp workers.
- **Pane IDs or tab names** (e.g. `/herdr-orchestrate p17,p18,p19` or `workers-a,workers-b`) → **use existing panes**.
- **No argument** → default to spawning one worker per work slice you designed; tell the user how many you're spawning and why.

### Spawning workers (preferred — omp is the workhorse)

**One tab per worker — never split panes into an existing tab.** Cramming workers as splits clutters the workspace; each worker gets a full tab. `herdr agent start` can only split, so use tab create + pane run:

```bash
# 1. Create a dedicated tab; capture root_pane.pane_id from the JSON response
herdr tab create --workspace <workspace-id> --cwd <repo-root> --label <campaign>-w<N> --no-focus
# 2. Launch omp in that tab's root pane
herdr pane run <root-pane-id> "omp"
```

One tab per work slice, labeled `<campaign>-w<N>` so the sidebar shows what each worker is doing. Get the current workspace ID from `herdr pane current` or `herdr pane list`. Spawned omp agents register with herdr's agent detector (the omp integration is installed), so their `agent_status` in `herdr pane list` and `herdr agent wait <target> --status idle` are **reliable**.

Wait for each worker to reach `idle` (finished booting) before dispatching.

### Using existing panes

- **Pane IDs must be workspace-qualified** (`w653a789c697dc2:p17`, not bare `p17`). Dispatching to a bare ID can silently go to the wrong workspace — a dispatch has been lost to this before.
- Tab names: `herdr tab list` to map label → tab_id, then `herdr pane list` and take panes whose `tab_id` matches. If a named tab has multiple panes, confirm with the user which is the worker.
- Manually-started agents may NOT be registered with herdr's agent detector, so `herdr agent wait` / `agent_status` can be unreliable for them. The robust fallback liveness check:
  ```
  herdr pane read <qualified-id> | grep "(esc"
  ```
  Spinner present ⇒ busy; absent ⇒ idle.
- Never conscript panes the user didn't name — panes may carry other in-progress work.

## 2. Packet protocol

Write each assignment to `/tmp/<campaign>-<pane>.txt` with these fixed sections:

```
GOAL
CONTEXT
STEPS
CONSTRAINTS
ACCEPTANCE
```

Hard rules to bake into every packet's CONSTRAINTS:

- **Non-overlapping file scopes per worker** — no two workers may touch the same file.
- "Touch ONLY these files: <explicit list>."
- "Do NOT commit, stage, or revert anything."
- "No interactive questions — if blocked, report the blocker in your final response and stop."
- ACCEPTANCE includes the verification commands (TEST_CMD / TYPECHECK_CMD) with the framing: "pre-existing errors are acceptable; errors in YOUR files must be clean."

Dispatch with:

```
herdr pane run <qualified-id> "Read /tmp/<campaign>-<pane>.txt and execute the MASTER ASSIGNMENT exactly as written. Report results when done."
```

## 3. Monitor phase

Run the watcher loop with `run_in_background` so you get notified instead of blocking.

For **spawned** workers (reliable agent detection), poll `agent_status`:

```bash
while true; do
  busy=$(herdr pane list | jq -r '.result.panes[] | select(.pane_id as $p | ["<id1>","<id2>"] | index($p)) | select(.agent_status == "working") | .pane_id')
  if [ -z "$busy" ]; then echo "all idle"; exit 0; fi
  echo "busy: $busy"
  sleep 30
done
```

For **manual** panes, fall back to the spinner check per pane:

```bash
while true; do
  busy=""
  for id in <qualified-ids>; do
    if herdr pane read "$id" | grep -q "(esc"; then busy="$busy $id"; fi
  done
  if [ -z "$busy" ]; then echo "all idle"; exit 0; fi
  echo "busy:$busy"
  sleep 30
done
```

## 4. Verify phase (trust-but-verify — most of the value)

Worker summaries describe what they *intended* to do. Verify everything yourself:

1. Collect final reports via `herdr pane read <qualified-id>` (or `herdr agent read`).
2. Independently run `git status` and `git diff` on **every touched file**. Check for:
   - **Scope creep** — files outside the worker's assigned list.
   - **Mixed files** — uncommitted user work sharing a file with worker output. If a file mixes user and worker hunks, use partial-hunk staging (`git add -p`) to stage only the worker's hunks.
3. Run TEST_CMD and TYPECHECK_CMD yourself. **Never trust worker summaries.**
4. Commit only worker-scoped files, grouped per coherent slice — never `git add -A`.

## 5. Cleanup

- Close tabs **you spawned** after verification succeeds: `herdr tab close <tab-id>`. Keep a failed worker's tab open for diagnosis until its slice is resolved.
- Never close tabs or panes the user provided.
- Remove the campaign's `/tmp/<campaign>-*.txt` packets.

## Failure handling

- Worker reports a blocker → resolve it yourself or re-packet with clarified CONSTRAINTS; don't converse interactively in the worker pane.
- Worker made a small, obvious mistake → fixing it directly during verification is allowed (the minor-fix exception); anything larger gets re-packeted.
- Worker touched out-of-scope files → do not commit those hunks; note it and revert only the out-of-scope worker changes after confirming they aren't user work.
- Worker pane unresponsive → `herdr pane read` its scrollback to diagnose before re-dispatching.
