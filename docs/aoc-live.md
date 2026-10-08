# aoc-live: AOC context for ChatGPT

`aoc-live` is a local MCP server that lets ChatGPT read your herdr workspaces, agent conversations, issue journals (#9) and repositories. Its one write is `save_note`, which appends a note to `${XDG_STATE_HOME:-$HOME/.local/state}/aoc/live/notes.jsonl` (mode `0600`, capped at 1 MB); ChatGPT asks before calling it. It cannot run commands, type into panes or write to GitHub. Delegation still goes through a GitHub issue with `agent-ready`, which AOC Dispatch picks up.

## Run

`aoc-herdr-launch` starts it with herdr, next to the Master seats (`AOC_HERDR_LIVE=off` disables that). By hand:

```sh
aoc-live token init     # once: stores the URL token in the macOS Keychain
aoc-live start          # supervisor keeps the server (and tunnel, if set up) running
aoc-live status
aoc-live logs -f        # tool names and durations only
aoc-live stop           # cuts all access
```

The desktop listener stays on `127.0.0.1:8765` (`AOC_LIVE_PORT`) and answers only at `/mcp/<token>`. Treat the full URL as a password. `aoc-live token rotate` replaces it.

The tunnel uses a separate listener on `127.0.0.1:8766` (`AOC_LIVE_PUBLIC_PORT`). Every request needs a Cloudflare Access RS256 JWT for the configured team, application audience and owner email, plus the path token and the same Host/Origin guards. Without valid Access configuration, this listener returns **503 with an empty body for every request**; the desktop listener is unaffected.

## Connect from the phone (ChatGPT mobile, voice)

1. In Cloudflare Zero Trust, create a self-hosted Access application for the public hostname with an Allow policy for Alex's email only. New teams have no One-time PIN login method; add it under Integrations → Identity providers first. Keep App Launcher off (its clientless option is refused for public hostnames). Then edit the app, turn on **Managed OAuth** with redirect URIs `https://chatgpt.com/connector/oauth/*` and `https://chatgpt.com/connector_platform_oauth_redirect`, and note the team name and the app's AUD tag.
2. `aoc-live access set --team <team> --aud <64-char-aud> --email <owner-email>`. This writes `${XDG_CONFIG_HOME:-$HOME/.config}/aoc/live/access.json` with mode `0600` and refreshes any existing tunnel config. `aoc-live access show` displays the team, email and first eight AUD characters.
3. Once: `aoc-live tunnel setup aoc-live.intrface.eu`, then `aoc-live tunnel enable`. Setup creates the tunnel and DNS record but leaves a new config disabled. The tunnel forwards only to the public listener and also requires Access at cloudflared. Enable refuses missing or mismatched Access configuration; disable renames the config and restarts the supervisor, leaving desktop access available.
4. `aoc-live url --show` and copy the public URL. In ChatGPT on the web, turn on Developer mode and create an app with **Authentication = OAuth**. Leave client ID and client secret empty so ChatGPT uses dynamic registration; sign in with the allowed email.
5. The app syncs to the phone. In a chat, pick it under **+**, then ask "what are my agents doing?"; the model calls `workspace_overview` first. Verified 2026-10-04 from the Android app in voice mode.

`AOC_LIVE_ACCESS_TEAM`, `AOC_LIVE_ACCESS_AUD` and `AOC_LIVE_ACCESS_EMAIL` override service configuration for tests. The supervisor still requires `access.json` and a matching `required: true` tunnel Access block. `aoc-live status` reports both ports, Access configuration and tunnel enabled/disabled/blocked state.

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
