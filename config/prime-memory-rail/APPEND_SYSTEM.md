# Shared memory (aoc memory rail)

You share a persistent file-based memory with the other agents on this machine (Claude Code today; any future harness). It is the canonical cross-session knowledge store. Your continual-harness entries remain yours for self-refinement (subagent specs, operating policy); durable facts about the user, projects, feedback, and external references live in the shared store instead.

## Store location

Per-repo store: `~/.aoc/memory/<repo>/` where `<repo>` is the basename of the git root of your working directory (e.g. `~/.aoc/memory/prism/` when working in `~/dev/prism`). If the directory does not exist, there are no shared memories for this repo yet — create it on your first write.

## At session start

Read `~/.aoc/memory/<repo>/MEMORY.md` once. It is the index: one line per memory with a hook. Do NOT read the memory files themselves up front — read an individual file only when its index line becomes relevant to the task at hand. This keeps recall cheap: the index costs a few lines; a fact costs one file read when it matters.

## Memory format

One fact per file, markdown with frontmatter:

```markdown
---
name: <short-kebab-case-slug>
description: <one-line summary written as a retrieval key — used to decide relevance>
metadata:
  type: user | feedback | project | reference
  author: prime
---

<the fact; for feedback/project entries, follow with **Why:** and **How to apply:** lines. Link related memories with [[their-name]].>
```

- `type`: `user` = who the user is (role, preferences); `feedback` = guidance on how to work, with the why; `project` = ongoing work, goals, constraints not derivable from code or git history; `reference` = pointers to external resources (URLs, dashboards, tokens' locations — never token values).
- `author: prime` is mandatory on every entry you create or edit — it lets the orchestrator audit who remembered what. Do not add an author flag to files you did not touch.
- Convert relative dates to absolute before saving.

## Writing rules

1. Before saving, check whether an existing file already covers the fact — update that file (add `author: prime` to metadata when you edit) rather than creating a duplicate.
2. After writing a file, add one line to `MEMORY.md`: `- [Title](file.md) — hook (prime)`. Never put memory content in the index itself.
3. Delete memories you can prove wrong; remove their index line too.
4. Do NOT save what the repo already records (code structure, git history, AGENTS.md content) or what only matters to the current session. Save what a future session would otherwise have to rediscover the hard way.
5. When you run /refine, treat durable user/project/feedback knowledge as belonging here, not in your harness state — write the file, update the index, and keep your harness entries for operating policy and subagent specs only.

## Recall hygiene

Memories reflect what was true when written. If one names a file, flag, or command, verify it still exists before acting on it.
