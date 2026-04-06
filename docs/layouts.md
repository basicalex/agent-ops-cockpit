# Layouts

AOC now treats **`aoc`** as the single official managed general-purpose layout.

That keeps the default experience consistent while still allowing custom layouts for project-specific workflows.

## Official managed layout

The official layout is:

- `aoc`

It is installed to:

- `~/.config/zellij/layouts/aoc.kdl`

`aoc-layout` always exposes this managed layout.

## Dedicated system layout

Mission Control uses its own explicit layout and launcher:

- `.aoc/layouts/mission-control.kdl`
- `aoc-new-tab --mission-control`
- `aoc-mission-control-tab`

This is a system/runtime layout, not a normal general-purpose mode.

## Deprecated managed layout names

These older managed names are deprecated and hidden from `aoc-layout`:

- `unstat`
- `minimal`
- `aoc-zjstatus-single`
- `aoc-zjstatus-test`
- `aoc.hybrid`

Older defaults using those names are normalized back to `aoc`.

## Custom layouts

Custom layouts can still live in either location:

- project shared: `.aoc/layouts/`
- personal global: `~/.config/zellij/layouts/`

When a custom layout name exists in both places, AOC resolves it in this order:

1. `.aoc/layouts/<name>.kdl`
2. `~/.config/zellij/layouts/<name>.kdl`

`aoc-layout` shows:

- the official managed layout: `aoc`
- project custom layouts
- global custom layouts

Internal/deprecated managed names are filtered out.

## Context injection

When AOC launches a layout, it replaces these placeholders with live values:

| Placeholder | Meaning |
|---|---|
| `__AOC_TAB_NAME__` | tab label |
| `__AOC_PROJECT_ROOT__` | absolute project path |
| `__AOC_AGENT_ID__` | repo/project slug |
| `__AOC_SESSION_ID__` | current Zellij session id |
| `__AOC_HUB_ADDR__` | hub host:port |
| `__AOC_HUB_URL__` | hub websocket URL |

## Example custom layout

Create `.aoc/layouts/review.kdl`:

```kdl
layout {
  tab name="__AOC_TAB_NAME__ [Review]" focus=true {
    pane split_direction="vertical" {
      pane name="Git" size="30%" command="bash" {
        args "-lc" "export AOC_PROJECT_ROOT=\"__AOC_PROJECT_ROOT__\"; if command -v aoc-tab-metadata >/dev/null 2>&1; then aoc-tab-metadata sync >/dev/null 2>&1 || true; fi; cd \"__AOC_PROJECT_ROOT__\" && git status"
      }
      pane name="Review" size="70%" command="bash" {
        args "-lc" "export AOC_PROJECT_ROOT=\"__AOC_PROJECT_ROOT__\"; if command -v aoc-tab-metadata >/dev/null 2>&1; then aoc-tab-metadata sync >/dev/null 2>&1 || true; fi; cd \"__AOC_PROJECT_ROOT__\" && ${EDITOR:-micro} ."
      }
    }
  }
}
```

## Commands

```bash
aoc-layout --list
aoc-layout --current
aoc-layout --set aoc
aoc-new-tab --layout review
```

## Notes

- Official AOC panes call `aoc-tab-metadata sync` so the managed top bar can group tabs using explicit project metadata rather than only tab-name inference.
- Future custom-layout creation/edit flows may be surfaced through `aoc-control`, but the file-based custom layout contract remains supported.
