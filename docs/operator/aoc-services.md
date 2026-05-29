# AOC Services

`aoc-services` is the project-local service supervisor intended for the dedicated Mission Control tab.

## Runtime model

Mission Control owns shared long-lived services so individual agents do not scatter runtime startup across sessions.

Default centralized services:

| Service | Kind | Default action |
|---|---|---|
| `search` | server | started by `aoc-services up`; backs `aoc-search` via local SearXNG |
| `browser` | daemon | reported centrally; starts lazily through `agent-browser` when needed |
| `mind` | daemon-or-cold | reported centrally; remains cold/lazy unless warm mode is explicitly used |
| `render` | oneshot | reported centrally; no server required for Obscura-backed `aoc-render` |

One-shot tools such as `aoc-fetch` and `aoc-render` should not become persistent servers by default.

## Commands

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

## Mission Control integration

The dedicated Mission Control layout includes an **AOC Services** pane running:

```bash
aoc-services up --watch --interval 30
```

This makes Mission Control the visible owner of project runtime health while agents consume services through normal commands:

```bash
aoc-search query --mode docs "..."
aoc-fetch https://example.com --format markdown
aoc-render https://example.com --format text
agent-browser open https://example.com
```

## Agent guidance

Agents should prefer checking shared state instead of starting ad-hoc services:

```bash
aoc-services status --json
```

If search is down and web discovery is required, start it centrally:

```bash
aoc-services start search
```
