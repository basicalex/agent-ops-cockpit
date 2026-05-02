---
name: web-research
description: Search-first web research workflow for AI agents. Use when the task needs finding sources, comparing documentation, investigating errors, gathering external references, or doing general web research before opening pages in the browser.
allowed-tools: Bash(aoc-search:*), Bash(aoc-fetch:*), Bash(aoc-render:*), Bash(aoc-obscura-install:*), Bash(agent-browser:*), Bash(curl:*), Bash(wget:*)
---

# Web Research with `aoc-search` + `aoc-fetch` + `aoc-render` + `agent-browser`

## Core Rule

Use a **search -> fetch -> render -> browser** workflow:

1. **Search first** with `aoc-search query ...`
2. **Fetch second** with `aoc-fetch <url>` for cheap static extraction
3. **Render third** with `aoc-render <url>` when static fetch misses JS-rendered content and Obscura is available
4. **Browse last** with `agent-browser open <url>` only when page interaction, screenshots, auth, or full Chromium behavior is needed
5. **Cite what you used** in your final answer when relevant

Do **not** start by opening random sites in the browser if a normal search step would narrow the problem first.

## When to use this skill

Use this skill when the user asks for:

- web research
- documentation lookup
- troubleshooting an external error/library/framework
- comparing third-party tools/services
- finding official docs, release notes, pricing pages, or support articles
- gathering source links before deeper browser automation

Use `agent-browser` alone when the task is primarily page interaction (forms, clicks, screenshots, app testing, auth flows).

## Search workflow

### Inspect search availability

```bash
aoc-search status
aoc-search health
```

If search is not configured, tell the user to enable it via:

- `Alt+C -> Settings -> Tools -> Agent Browser + Search`

### Query examples

```bash
aoc-search query --limit 5 "react useeffect docs"
aoc-search query --mode docs --limit 5 "nextjs caching"
aoc-search query --mode error --limit 8 "TypeError Cannot read properties of undefined"
aoc-search query --mode package --limit 5 "clap rust subcommands"
aoc-search query --mode package --direct --limit 5 "clap"
aoc-search query --mode github --limit 5 "h4ckf0r0day/obscura"
```

### JSON mode for structured inspection

```bash
aoc-search query --json --limit 5 "rust clap subcommands"
```

Normalized result fields:

- `title`
- `url`
- `snippet`
- `source`
- `rank`

## Fetch workflow

Once you have candidate URLs, try cheap static extraction first:

```bash
aoc-fetch https://example.com --format markdown
aoc-fetch https://example.com --format json --max-chars 8000
```

## Render workflow

If static fetch misses JS-rendered content, try lightweight rendering before Chromium:

```bash
aoc-render status
aoc-render https://example.com --format text
aoc-render https://example.com --format json --max-chars 8000
aoc-render https://example.com --format text --fallback agent-browser
```

If Obscura is missing, use `aoc-obscura-install` only when the user asks to enable lightweight rendering; otherwise skip directly to browser only if the task needs it.

## Browse workflow

Escalate to browser automation only when needed:

```bash
agent-browser open https://example.com
agent-browser wait --load networkidle
agent-browser snapshot -i
```

Use browser automation when you need to:

- inspect rendered content
- click tabs/buttons
- expand accordions
- capture screenshots/PDFs
- extract content hidden behind interaction

## Recommended pattern

```bash
# 1) Search
aoc-search query --limit 5 "vercel environment variables docs"

# 2) Fetch the best source cheaply
aoc-fetch https://vercel.com/docs --format markdown

# 3) Try lightweight rendering if static fetch is insufficient
aoc-render https://vercel.com/docs --format text

# 4) Escalate to browser only if needed
agent-browser open https://vercel.com/docs
agent-browser wait --load networkidle
agent-browser snapshot -i
```

## Good operating behavior

- Prefer official docs and primary sources first
- Use `--mode docs` for framework/library docs
- Use `--mode error` for stack traces and exception messages
- Use `--mode package` for package/library discovery
- Use `--mode github` for repositories and known `owner/repo` lookups
- Prefer `aoc-fetch` before rendering/browser for static pages
- Prefer `aoc-render` before `agent-browser` for JS-rendered read-only pages
- Keep queries specific; refine instead of opening many weak results
- If one fetched page is enough, stop early
- If search is unhealthy, report that clearly instead of pretending browsing is equivalent

## Failure guidance

Typical failures:

- `search is not configured for this repo; enable it via Alt+C`
- `search is disabled in .aoc/search.toml`
- `managed search is stopped and auto-start is disabled`
- `search backend returned invalid JSON`

When search fails but browsing can still help, explain the limitation and ask whether to continue with manual browser-based research.
