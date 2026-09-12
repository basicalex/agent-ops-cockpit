# Machine-global agent rules

This file holds machine-global agent rules. Project rules live in each repo's `AGENTS.md`.

## Low-Token Default Mode
- Keep responses concise by default; do not print full files or raw logs unless explicitly requested.
- Start with the smallest viable step; use narrow, path-scoped searches before broad scans.
- Read files in bounded chunks and avoid rereading unchanged large files.
- Summarize command/tool output with actionable lines only (key errors, next actions).
- Run targeted checks/tests first; run full-suite commands only when required.
- If targeted inspection fails, escalate scope gradually and state why.
- Use fresh sessions after major milestones or context drift to reduce replay overhead.
- For narrow diagnostics/Q&A, use at most 3 tool calls before first answer; ask before broader escalation.
- Do not open/read image binaries unless the user explicitly asks to view/open one now.
- Use one narrow diagnostic path first; avoid retry spray with variant commands unless first attempt fails.

## Prose style (docs, PR text, commit messages, reports, UI/marketing copy)

These rules govern prose only. Never touch code, identifiers, or precise technical terms.

1. Cut every word that adds nothing; prefer the short word over the long one.
2. Use the active voice, not the passive.
3. Avoid stock metaphors and phrases you are used to seeing in print.
4. No achievement language or filler jargon — "comprehensive", "robust", "seamless", "leverage", "ensure". Say what it does in everyday words.
5. Break any of these rules sooner than write something awkward or imprecise.

- Commit messages and PR descriptions: state what changed and why in plain words. A reviewer should know what it does in one read.
- Progress reports: plain sentences — what changed, what failed, what comes next. No emoji checkmarks, no "Successfully", no walls of bullets.
- Marketing/landing copy: one concrete claim per line; if a competitor could paste the line unchanged onto their page, rewrite or delete it.

## Artifacts and pages

- Deliverable pages (reports, briefs, boards, dashboards, one-off HTML) are written as page bodies to `~/dev/artifact-library/<project>/<slug>.html` (`<title>` first, then `<style>`, then content; no doctype/html/head/body) and published with `artifact-pages add <project>/<slug> --title "…"` then `artifact-pages publish`. The page is then at `https://docs.intrface.eu/<project>/<slug>/`, private by default. `<project>` is the repo basename. "Give me an artifact" or "report this as an artifact" means this docs page; no other artifact tool is used unless the user explicitly asks for a claude.ai artifact. Pages are `research` by default; only the user marks a page as a `decision` (`artifact-pages decide`). Reading is local: `artifact-pages decisions <project>`, `artifact-pages show <project>/<slug>` (page as text), `artifact-pages find <words>` (search). Before proposing direction in a project, run `artifact-pages decisions <project>` and read what it lists. When the user says they left comments on a page, run `artifact-pages comments <project>`, act on each open thread, wrap the changed passages in `<ins class="rev" data-comment="<id>">`, publish, then `artifact-pages reply <id> "<what changed>" --addressed`; never mark a thread accepted. Full rules: `~/.claude/skills/artifact-pages/SKILL.md`.
- Sharing with a client is `artifact-pages share <project>/<slug> --days N` (signed link) or `artifact-pages visibility <project>/<slug> public` + `publish`. Never hand out the owner password.
