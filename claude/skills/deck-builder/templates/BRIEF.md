# <Deck> — shared brief for workers

Audience: <who, setting, duration, language>.
Deck = <one line on what it must achieve>.

## Paths
- Workspace: <absolute path>  (content.md, BRIEF.md, images/, charts/, project/, render/, dist/)
- ppt-master project: <absolute path under workspace/project/>
- Tools: ~/dev/tools/ppt-master (python: ~/dev/tools/ppt-venv/bin/python), ~/dev/tools/lieflat-charts
- Source repo (if screenshots come from a running app): <path> — read-only for workers; never edit, commit, or restart servers.

## Screenshot capture (if applicable)
- App URL: <http://localhost:PORT>, login route / personas: <…>
- Playwright: `import { chromium } from 'playwright'` run from ~/.claude/skills/deck-builder (bun resolves node_modules there), or copy the import path used by the render script.
- Run with `bun <script>.mjs`; no `timeout` binary on macOS, so add `setTimeout(() => process.exit(2), 600000)` inside the script. ALWAYS `await browser.close()` in `finally`.
- Shots: deviceScaleFactor 2, PNG. Desktop 1440×900. Phone 390×844 (isMobile, hasTouch).
- Hide dev overlays and consent banners before shooting; note anything that had to be dismissed.

## Brand
- <palette with hex and the one rule per colour>
- <typography>
- <what is forbidden: black outlines, heavy borders, emoji, icon clutter, stock illustration>

## Report format
Follow the communication contract at ~/.config/aoc/communication-contract.md: plain language, findings not narrative, no filler. List every output file with its absolute path and what it shows. Say plainly what you could not do and why.
You are the worker: never delegate, never spawn agents.
