# aoc-live: read-only AOC context for ChatGPT

`aoc-live` is a local MCP server that lets ChatGPT read your herdr workspaces, agent conversations, issue journals (#9) and repositories. It cannot run commands, type into panes or write to GitHub. Delegation still goes through a GitHub issue with `agent-ready`, which AOC Dispatch picks up.

## Run

`aoc-herdr-launch` starts it with herdr, next to the Master seats (`AOC_HERDR_LIVE=off` disables that). By hand:

```sh
aoc-live token init     # once: stores the URL token in the macOS Keychain
aoc-live start          # supervisor keeps the server (and tunnel, if set up) running
aoc-live status
aoc-live logs -f        # tool names and durations only
aoc-live stop           # cuts all access
```

The server listens on 127.0.0.1:8765 and answers only at `/mcp/<token>`. Treat the full URL as a password. `aoc-live token rotate` replaces it.

## Connect from the phone (ChatGPT mobile, voice)

1. Once: `aoc-live tunnel setup aoc-live.intrface.eu`. This creates a Cloudflare tunnel named `aoc-live` and one DNS record; the supervisor runs it from then on.
2. `aoc-live url --show` and copy the public URL.
3. In ChatGPT on the web: Settings → Security and login → Developer mode on. Then Plugins → + → create app, paste the URL, choose **No authentication**.
4. The app is then available on the phone. Ask "what are my agents doing?"; the model calls `workspace_overview` first.

## Connect from the ChatGPT desktop app

`aoc-live connect` writes `[mcp_servers.aoc-live]` into `~/.codex/config.toml`; restart the desktop app. Nothing leaves the Mac on this route. `aoc-live disconnect` removes it.

## What it exposes

| Tool | Returns |
| --- | --- |
| `workspace_overview` | One line per herdr workspace |
| `get_workspace_state` | Branch, HEAD, changes, tabs and agents with their issue state, blockers, open issues, recent commits |
| `list_tabs`, `get_tab_context`, `read_pane` | Tabs, agents, conversation summary or screen tail |
| `search_conversations`, `get_conversation_slice` | Claude and omp messages by search or cursor; no tool output or thinking |
| `list_issues`, `get_issue_state` | GitHub issues; `aoc-journal state` snapshot per issue |
| `get_git_state`, `get_diff`, `read_file`, `search_code` | Repository state, bounded diffs, tracked files, `git grep` |

Only repositories behind a live herdr workspace are readable. Secret files (`.env`, keys, `~/.ssh`, gh and Prism credentials) are refused, and every returned text is scanned for tokens and masked.
