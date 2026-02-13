# Custom Layouts & "AOC Modes"

AOC isn't just one static layout. It's a platform for running different "Modes" (layouts) that automatically adapt to your current project. This allows you to have a `writing` mode, a `coding` mode, or a `review` mode, all powered by the same AOC engine.

## Quick Start

AOC comes with a `minimal` layout out of the box.

**Try it now:**
```bash
# Open a new tab in Minimal Mode
aoc-new-tab --layout minimal

# Or set it as your default for all future tabs
aoc-layout --set minimal
```

## Creating Your Own Layout

Layouts can live in either location:

- **Project shared (recommended for teams):** `.aoc/layouts/` (commit to git)
- **Personal global:** `~/.config/zellij/layouts/`

Any standard [Zellij KDL layout](https://zellij.dev/documentation/layouts.html) works, but AOC adds a special layer of "Context Injection" that makes them powerful.

When a layout name exists in both places, AOC resolves it in this order:
1. `.aoc/layouts/<name>.kdl`
2. `~/.config/zellij/layouts/<name>.kdl`

### The Magic Placeholders

When you launch a tab, AOC reads your layout and replaces these tokens with real values from your current project:

| Placeholder | Replaced With | Example |
|-------------|---------------|---------|
| `__AOC_TAB_NAME__` | The name of the tab | "Agent" or "MyProject" |
| `__AOC_PROJECT_ROOT__` | Absolute path to the project | `/home/user/dev/my-app` |
| `__AOC_AGENT_ID__` | Unique ID for the project/tab | `my-app` |
| `__AOC_SESSION_ID__` | Current Zellij session ID | `otter-debugs` |
| `__AOC_HUB_ADDR__` | Hub host:port for this session | `127.0.0.1:42017` |
| `__AOC_HUB_URL__` | Hub websocket URL for this session | `ws://127.0.0.1:42017/ws` |

### Example: The "Review" Layout

Create `.aoc/layouts/review.kdl`:

```kdl
layout {
    tab name="__AOC_TAB_NAME__ [Review]" focus=true {
        pane split_direction="vertical" {
            // Left: Git status
            pane name="Git" size="30%" command="bash" {
                args "-lc" "cd \"__AOC_PROJECT_ROOT__\" && git status"
            }
            // Right: Editor
            pane name="Review" size="70%" command="bash" {
                 args "-lc" "cd \"__AOC_PROJECT_ROOT__\" && ${EDITOR:-micro} ."
            }
        }
        // Essential status bar
        pane size=1 borderless=true {
             plugin location="zellij:status-bar"
        }
    }
}
```

Now you can run: `aoc-new-tab --layout review` inside any project, and it will open rooted in that project!

## Managing Layouts (`aoc-layout`)

The `aoc-layout` tool is your dashboard for managing these modes. It lists both project and global layouts.

### Commands

| Command | Description |
|---------|-------------|
| `aoc-layout --set [name]` | Sets the **default** layout for new tabs/sessions. |
| `aoc-layout --tab [name]` | Opens a **single** new tab with the specified layout. |
| `aoc-layout --current` | Prints the name of the currently active default layout. |
| `aoc-layout` | Interactive menu to select any of the above. |

### "AOC Mode" vs. Standard Zellij Layouts

Why use `aoc-layout` instead of just `zellij action new-tab --layout ...`?

1.  **Context Awareness:** Standard Zellij layouts can't easily inherit the "project root" of your current specific tab. AOC injects `__AOC_PROJECT_ROOT__` dynamically, so your terminals start in the right place.
2.  **Persistence:** `aoc-layout` remembers your preference across reboots.
3.  **Integration:** It works seamlessly with `aoc-launch` and `aoc-new-tab`.

## Community Layouts

We encourage sharing layouts! If you create a useful mode (e.g., for Rust dev, Python data science, or writing markdown), share it in the [Discussions](https://github.com/basicalex/agent-ops-cockpit/discussions).

## Troubleshooting

*   **Layout not found?** Ensure it ends in `.kdl` and is located in `.aoc/layouts/` or `~/.config/zellij/layouts/`.
*   **Terminals starting in home dir?** Make sure you used the `__AOC_PROJECT_ROOT__` placeholder in your `args`.
    *   *Correct:* `args "-lc" "cd \"__AOC_PROJECT_ROOT__\" && ..."`
    *   *Incorrect:* `cwd "__AOC_PROJECT_ROOT__"` (Zellij 0.43+ supports `cwd`, but our injection method ensures robust variable expansion even inside command arguments).
