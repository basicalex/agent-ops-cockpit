# Prime memory rail — Level 2 plan

Status: Level 1 live on this machine since 2026-08-08. Level 2 built and installed 2026-08-08; the rail is aoc-managed. Claude-side auto-link (SessionStart hook) added 2026-08-08 — repos join the rail on first session, and a plain `install.sh` sets up the whole rail on a fresh machine.

## Built (2026-08-08)

- Repo assets under `config/prime-memory-rail/`: `extensions/aoc-memory.ts`, `skills/refine/**` (shadowed skill), `APPEND_SYSTEM.md` (contract template). Installed to `~/.prime/agent/` by `bin/aoc-prime-memory-install` (cmp-keep, overwrite-on-change — same pattern as `aoc-claude-codex-install`), wired into `install.sh` in the "Generating configurations" block.
- `bin/aoc-memory-link <repo-path>`: creates `~/.aoc/memory/<basename>/`, migrates the repo's Claude Code memory dir into it (store wins conflicts; diverging copies go to `memory.pre-link.bak`), and symlinks the Claude path. Idempotent; relinks stale symlinks, which covers the Mac path-slug migration case.
- `bin/aoc-claude-memory-hook`: Claude Code `SessionStart` hook that runs `aoc-memory-link` on the git root of the session's cwd. Every repo joins the rail the first time a Claude session starts in it — no manual link step. Silent (hook stdout would leak into session context), always exits 0, no-op outside git repos. `aoc-claude-install` merges the hook entry into `~/.claude/settings.json` surgically (replaces only entries referencing the hook script, keeps all other hooks).
- Second store rolled out: `~/.aoc/memory/agent-ops-cockpit/` (7 files migrated from this repo's Claude memory dir).
- Deviation from the plan: the builtin refine skill is a *python-kind* skill (`pyproject.toml` + `src/refine/__init__.py` provide the kernel-side `refine.run()/status()` API). A SKILL.md-only shadow would win the name collision and drop that package, breaking the IPython API. The shadow therefore carries verbatim copies of the builtin's python files alongside the extended SKILL.md; re-sync them on prime upgrades if the builtin's wrapper changes.
- `refine_complete` carries no cwd, so the validator resolves the store from the extension process's `process.cwd()`; `before_agent_start` uses `systemPromptOptions.cwd`.
- `autoRefine` settings untouched (defaults in effect); tune after observing refine output quality.

## What exists (Level 1)

- Canonical store: `~/.aoc/memory/<repo>/` — one markdown file per fact plus a `MEMORY.md` index (one line per memory). Prism migrated first.
- Claude Code: its fixed per-project memory path is a symlink into the store (`~/.claude/projects/<encoded-project-path>/memory → ~/.aoc/memory/<repo>`). The harness notices nothing.
- Prime: `~/.prime/agent/APPEND_SYSTEM.md` carries the contract — read the index at session start, read a fact file only when its index line is relevant, write curated one-fact files with `author: prime` in frontmatter and `(prime)` on the index line, dedupe before creating, delete proven-wrong entries.
- Author audit: grep the store for `author: prime` or scan `(prime)` index lines.

Level 1's known gap: the contract is prose. Prime follows it as well as it follows instructions — nothing enforces the session-start index read, and /refine still defaults to writing prime's native harness state.

## Level 2 — make the rail structural

Three pieces, all in supported prime extension points, all outside the installed package, all upgrade-surviving. Verified against prime-agent v0.7.1 (`dist/` paths cited below are evidence, not patch targets).

### 1. Extension: `aoc-memory.ts`

Location: `~/.prime/agent/extensions/aoc-memory.ts` (auto-discovered; hot-reloads via `/reload`).

- **`before_agent_start`** (types.d.ts:475-486; runner.js:698-750 chains rewrites): resolve the git root of cwd, read `~/.aoc/memory/<repo>/MEMORY.md`, and append it to `systemPrompt` under a `# Shared memory index` heading. Guarantees the index is in context every session — no reliance on prime obeying a "read this file first" instruction. If no store exists, append the one-line "no shared memories yet; create on first write" note instead.
- Optionally in the same handler: strip or summarize the verbose `memory`-kind entries from the injected harness-state block, leaving prime's native `prompt`/`subagent` entries untouched. Defer this until native memory entries actually bloat (4 entries today).
- **`refine_complete`** (types.d.ts:495-507; fires for auto and manual refine): validate the store after each refine — every memory file has an index line, every index line has a file, frontmatter parses, `author:` present on prime-touched files. Log discrepancies to the session; regenerate missing index lines mechanically from frontmatter `name`/`description`.

### 2. Shadowed refine skill

Location: `~/.prime/agent/skills/refine/SKILL.md`. User skills outrank builtins (package-manager.js:42-62 precedence; skills.js:400-437 first-wins), and the name `refine` keeps the host-side machinery wired (system-prompt.js:25 gates on `hasRefineSkill`).

Content: the builtin refine doc, plus the routing rule — durable user/project/feedback/reference knowledge goes to the shared store as memory files (with index update and author flag); prime's harness state keeps only operating policy, subagent specs, and prompt tweaks. The /refine *command* cannot be shadowed (session commands parse before extension commands — agent-session.js:3202-3216); shadowing the skill text is sufficient because it changes what the model is told refining means.

### 3. Settings

`~/.prime/agent/settings.json` → `autoRefine` block (settings-manager.js:538-547): keep `enabled: true`; tune `turnInterval` (default 25) and `cooldownMs` (default 20 min) once we see how often prime's refine actually produces shared-store writes vs noise.

## Rollout

1. Build `aoc-memory.ts` with the `before_agent_start` injection only; verify on a real repo (start prime, confirm index in prompt, no store → graceful note).
2. Add the shadowed refine SKILL.md; run a manual `/refine` after a session with obvious durable facts; confirm files + index lines + author flags land in the store.
3. Add the `refine_complete` validator.
4. Fold all three into the aoc repo as managed assets (install.sh copies them like `aoc-prime-agent-install` artifacts) so every aoc machine gets the rail on setup.
5. Per-repo rollout is automatic: the `SessionStart` hook links each repo into the rail the first time Claude starts there (prime's global extension already resolves the store from cwd). Promote rule-shaped memories into each repo's AGENTS.md as they harden (memory = evolving knowledge; AGENTS.md = settled law).

## Fresh machine

`install.sh` gives a new machine the whole rail machinery:

- creates the store base dir `~/.aoc/memory/`
- copies `aoc-memory-link` + `aoc-claude-memory-hook` into `~/.local/bin` (the bin sweep; both are in the required-scripts check)
- `aoc-claude-install` merges the `SessionStart` hook into `~/.claude/settings.json`
- `aoc-prime-memory-install` installs the prime-side extension, shadowed refine skill, and contract into `~/.prime/agent/`

Each step is safe on a machine without prime-agent or Claude installed — they pre-seed config that takes effect once the tool arrives. Memory *data* is not the installer's job: `~/.aoc/memory/` contents sync between machines via dotfiles. Repos relink themselves on first session (the hook relinks stale slugs, which covers path differences between machines).

## Risks

- Extension API drift on prime upgrades: `before_agent_start`/`refine_complete` are documented public events; if a major version changes them, the extension fails loud at load, not silently.
- Two writers, one store: mitigated by one-fact-per-file (conflicts are per-file, rare) and author flags. No locking needed at current scale.
- Prime writing junk memories: the curation rules are prose; the validator catches format drift but not judgment drift. Periodic audit of `(prime)` entries by the orchestrator is the backstop — cheap, on request.
