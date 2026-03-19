---
name: web-research
description: Search-first web research workflow for AI agents. Use when the task needs finding sources, comparing documentation, investigating errors, gathering external references, or doing general web research before opening pages in the browser.
allowed-tools: Bash(aoc-search:*), Bash(agent-browser:*), Bash(curl:*), Bash(wget:*)
---

# Web Research with `aoc-search` + `agent-browser`

## Core Rule

Use a **search-first, browse-second** workflow:

1. **Search first** with `aoc-search query ...`
2. **Review top results** and choose promising sources
3. **Browse second** with `agent-browser open <url>` when page interaction or deeper inspection is needed
4. **Cite what you used** in your final answer when relevant

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

## Browse workflow

Once you have candidate URLs:

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

# 2) Open the best source
agent-browser open https://vercel.com/docs
agent-browser wait --load networkidle
agent-browser snapshot -i

# 3) Inspect or interact further if needed
agent-browser get text body
```

## Good operating behavior

- Prefer official docs and primary sources first
- Use `--mode docs` for framework/library docs
- Use `--mode error` for stack traces and exception messages
- Keep queries specific; refine instead of opening many weak results
- If one page is enough, stop early
- If search is unhealthy, report that clearly instead of pretending browsing is equivalent

## Failure guidance

Typical failures:

- `search is not configured for this repo; enable it via Alt+C`
- `search is disabled in .aoc/search.toml`
- `managed search is stopped and auto-start is disabled`
- `search backend returned invalid JSON`

When search fails but browsing can still help, explain the limitation and ask whether to continue with manual browser-based research.
