# Taskmaster Plugin

A Zellij WASM plugin that provides a real-time, interactive task management TUI within the AOC (Agent Ops Cockpit) environment. It automatically discovers the project root for each tab and displays tasks from that project's `.taskmaster/tasks/tasks.json` file.

## Key Features

### Per-Tab Project Isolation
- Each Zellij tab has its own Taskmaster plugin instance
- Plugin discovers project root by parsing the **Agent pane's title** (`Agent [<project-name>]`)
- Derives root path as: `projects_base/<project-name>` (default: `~/dev/<project-name>`)
- Supports multiple projects open in different tabs simultaneously

### Flexible Task File Format
- Reads from `.taskmaster/tasks/tasks.json`
- Supports both string and numeric task IDs
- Supports multiple tags (branches/contexts) with `master` as default
- Handles extra/unknown fields gracefully via `#[serde(flatten)]`

### Real-Time Updates
- Configurable refresh interval (default: 1 second)
- Automatically reloads tasks when file changes
- Updates pane title with progress bar and statistics

## TUI Interface

```
+- Taskmaster [master] [========  ] 36/40 | Filter: all | ? Help -+
| +-Tasks------------------------------------------------------+  |
| | ID   S   P   Title                                         |  |
| | 1    v   *   Add aoc-doctor dependency checker             |  |
| | 2    v   *   Improve aoc-taskmaster interactivity          |  |
| | > 3  o   ^   Widget resilience and UX polish               |  |
| |   +- o       Fix widget crash on empty response            |  |
| |   +- v       Add loading spinner                           |  |
| | 4    o   *   Implement dark mode                           |  |
| | 5    x   v   Low priority cleanup                          |  |
| +------------------------------------------------------------+  |
+-----------------------------------------------------------------+
```

### Column Definitions

| Column | Description |
|--------|-------------|
| **ID** | Task identifier (blank for subtasks) |
| **S** | Status icon |
| **P** | Priority icon |
| **Title** | Task title with optional expand/agent indicators |

### Status Icons

| Icon | Status |
|------|--------|
| `v` | Done |
| `o` | In Progress |
| `o` | Pending |
| `x` | Blocked |
| `@` | Review |
| `x` | Cancelled |

### Priority Icons

| Icon | Priority |
|------|----------|
| `^` | High |
| `*` | Medium |
| `v` | Low |

### Special Indicators

| Icon | Meaning |
|------|---------|
| `>` | Task is expanded (showing subtasks) |
| `>` | Task has subtasks (collapsed) |
| `@` | Active agent working on task |
| `+-` | Subtask indent |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `Enter` | Toggle detail view |
| `Space` | Expand/collapse subtasks |
| `x` | Toggle task status (done/pending) |
| `f` | Cycle filter (all -> pending -> done) |
| `t` | Cycle through tags |
| `r` | Force refresh |
| `?` | Toggle help panel |
| `Tab` | Switch focus (list <-> details) |

## Mouse Support

| Action | Effect |
|--------|--------|
| Left click on task | Select task |
| Left click on selected task | Toggle details |
| Scroll up/down | Navigate list |

## Configuration

In `aoc.kdl` layout:

```kdl
plugin location="file:/path/to/aoc-taskmaster.wasm" {
  refresh_secs "1"              // Refresh interval in seconds
  projects_base "/home/user/dev" // Base directory for project lookup
}
```

## Project Root Discovery Flow

```
1. Plugin loads -> subscribes to PaneUpdate events
2. PaneUpdate fires -> receives PaneManifest
3. Find plugin's own pane ID in manifest
4. Identify which tab contains this plugin instance
5. Within that tab, find pane with title "Agent [<name>]"
6. Extract <name> from title pattern
7. Derive root = projects_base + <name>
8. Read tasks from root/.taskmaster/tasks/tasks.json
```

## Task File Format

```json
{
  "master": {
    "tasks": [
      {
        "id": "1",
        "title": "Task title",
        "description": "Description",
        "details": "Extended details",
        "status": "pending",
        "priority": "high",
        "dependencies": ["2", "3"],
        "subtasks": [
          {
            "id": 1,
            "title": "Subtask",
            "status": "pending"
          }
        ],
        "activeAgent": false,
        "updatedAt": "2024-01-01T00:00:00Z"
      }
    ]
  },
  "feature-branch": {
    "tasks": [...]
  }
}
```

### Field Types

- `id`: String or number (both supported)
- `status`: `pending` | `in-progress` | `done` | `cancelled` | `deferred` | `review` | `blocked`
- `priority`: `high` | `medium` | `low`
- `dependencies`: Array of strings or numbers
- `subtasks`: Array of subtask objects
- `activeAgent`: Boolean indicating if an agent is working on the task

## Pane Title Format

The plugin dynamically updates its pane title:

```
Taskmaster [<tag>] [<progress-bar>] <done>/<total> | Filter: <filter> | ? Help
```

Example:
```
Taskmaster [master] [========  ] 36/40 | Filter: all | ? Help
```

## Permissions Required

- `RunCommands` - For shell fallback when direct fs read fails
- `ChangeApplicationState` - For renaming pane title
- `ReadApplicationState` - For receiving PaneUpdate events

## Building

```bash
# From the repository root
./scripts/build-taskmaster-plugin.sh
```

This compiles the plugin to WASM and installs it to `~/.config/zellij/plugins/aoc-taskmaster.wasm`.

## Error States

The plugin shows debug information when tasks can't be loaded:

```
+-Tasks-----------------------------------------+
| Failed to parse tasks.json. Len: 2218452     |
|                                               |
| projects_base: "/home/ceii/dev"               |
| root: Some("/home/ceii/dev/voyager")          |
| tasks_path: Some("...")                       |
+-----------------------------------------------+
```

Common issues:
- **"Waiting for PaneUpdate event..."**: Plugin hasn't received pane manifest yet
- **"No Agent pane. Panes: [...]"**: No pane with title pattern `Agent [...]` found in tab
- **"Failed to parse tasks.json"**: JSON parsing error (check task file format)
