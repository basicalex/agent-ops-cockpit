# AOC Services

`aoc-services` is the project-local service supervisor used by the Herdr **AOC Services** workspace and the `aoc services` command.

## Runtime model

The Herdr AOC Services workspace owns shared long-lived services so individual agents do not scatter runtime startup across sessions.

Default centralized services:

| Service | Kind | Default action |
|---|---|---|
| `search` | server | started by `aoc-services up`; backs `aoc-search` via local SearXNG |
| `browser` | daemon | reported centrally; starts lazily through `agent-browser` when needed |
| `mind` | daemon-or-cold | reported centrally; remains cold/lazy unless warm mode is explicitly used |
| `render` | oneshot | reported centrally; no server required for Obscura-backed `aoc-render` |

One-shot tools such as `aoc-fetch` and `aoc-render` should not become persistent servers by default.

## Commands

Preferred operator entrypoint:

```bash
aoc services              # open/focus the Herdr Services workspace
aoc services workspace    # same as above
aoc services status
aoc services up --watch
aoc services start search
aoc services stop search
aoc services doctor
```

Direct supervisor commands remain available:

```bash
aoc-services status
aoc-services status --json
aoc-services status --watch
aoc-services up --watch
aoc-services start search
aoc-services stop search
aoc-services doctor
```

`aoc-services up --watch` starts the shared search service and then displays a live status board. Watchers are singleton-scoped per service root; duplicate panes/sessions exit instead of polling Docker repeatedly.

## Herdr integration

`aoc services` creates or focuses a project-scoped Herdr workspace named `AOC Services · <project> · <hash>`. The workspace keeps the normal coding workspace uncluttered and creates:

- `Overview` tab: `AOC_SERVICES_ROOT=<project> aoc-services up --watch --interval 30`
- `Search` tab: managed-search status plus the next operator commands

`aoc` may best-effort ensure this workspace when a Herdr server is already running (`AOC_HERDR_SERVICES=auto`, the default), but it does not focus it. Use `AOC_HERDR_SERVICES=off` to disable that ensure step or `AOC_HERDR_SERVICES=focus` for explicit service-ops sessions.

This makes Herdr the visible owner of project runtime health while agents consume services through normal commands:

```bash
aoc-search query --mode docs "..."
aoc-fetch https://example.com --format markdown
aoc-render https://example.com --format text
agent-browser open https://example.com
```

## Agent guidance

Agents should use stable AOC surfaces:

```bash
aoc-search query --mode docs "..."
```

OMP agents should use the `aoc_web_search` tool, which delegates to `aoc-search`. Agents must not call Docker Compose or SearXNG directly.

`aoc-search` query auto-start remains a safety fallback when project policy allows it. The primary operator UX is still the Herdr AOC Services workspace:

```bash
aoc-services status --json
aoc services start search
```
