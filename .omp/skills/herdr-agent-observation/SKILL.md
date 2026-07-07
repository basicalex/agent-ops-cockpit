---
name: herdr-agent-observation
description: Observe Herdr peers safely with native read-only herdr commands.
---

## When to use
- You need to inspect peer agents, panes, tabs, or workspaces in a Herdr/OMP session.
- You need a peer's visible/recent terminal output, status, session metadata, or an explanation of a reported agent.
- You are coordinating with a master/operator and need observation before using higher-level orchestration.

## Read-only first
Use native `herdr` commands for observation. Default to read-only commands:

```bash
herdr agent list
herdr agent get <target>
herdr agent read <target> --source recent --lines 80
herdr agent explain <target>
herdr agent wait <target> --status idle --timeout 60000
herdr workspace list
herdr workspace get <workspace_id>
herdr tab list [--workspace <workspace_id>]
herdr tab get <tab_id>
herdr pane list [--workspace <workspace_id>]
herdr pane get <pane_id>
herdr pane read <pane_id> --source recent --lines 120
```

Prefer `agent` commands when Herdr has identified an OMP/agent label. Use `pane` commands for raw terminal observation, unknown processes, or when you already have a pane id.

## Transcript fallback
If Herdr reports an agent session path or session id, use `herdr agent explain <target>` first. Read OMP session files directly only when native `herdr agent read`, `herdr pane read`, and `herdr agent explain` do not provide the needed transcript context.

When reading OMP session files:
- Treat them as local transcript evidence, not a control surface.
- Read the smallest range needed.
- Do not edit, truncate, or move session files.
- Do not scrape unrelated sessions.

## Mutating commands require operator instruction
Do not use control or input commands unless the operator explicitly asks for that exact action and target:

```bash
herdr agent send <target> <text>
herdr agent focus <target>
herdr agent start <name> -- <argv...>
herdr pane send-text <pane_id> <text>
herdr pane send-keys <pane_id> <key> [...]
herdr pane run <pane_id> <command>
herdr pane focus --direction ...
herdr tab focus <tab_id>
herdr workspace focus <workspace_id>
```

Never start, focus, type into, or send keys to a peer just to observe it. Observation is list/get/read/explain/wait plus pane/tab/workspace listing and reads.
