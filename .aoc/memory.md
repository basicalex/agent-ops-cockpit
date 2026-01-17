# Agent Memory for Project: agent-ops-cockpit
This file contains persistent context, decisions, and knowledge for the AI agent.
Agents should read this to understand project history and append new decisions here.

## Core Decisions
- [2026-01-11 21:00] Implemented native Gemini support in Zellij (bypassed tmux) to fix redraw issues.
- [2026-01-11 21:02] Integrated Claude Code, Gemini, and OpenCode shims with tmux-backed scrollback support (except Gemini).
- [2026-01-11 21:02] Fixed infinite recursion in aoc-agent-wrap by adding robust is_wrapper and path resolution logic.
- [2026-01-11 21:02] Modified aoc-gemini/cc/oc to use AOC_AGENT_ID directly for new tabs to avoid overriding global user defaults.
- [2026-01-11 21:02] Created aoc-mem tool for project-local, markdown-based long-term memory in .gemini/memory.md.
- [2026-01-12 14:59] Taskmaster plugin now resolves root via the shared project_root file (and only falls back to env), so new tabs don't inherit stale AOC_PROJECT_ROOT.
- [2026-01-14 09:58] Taskmaster plugin: aoc-new-tab now creates a unique per-tab project_root file (mktemp) and plugin render throttles based on data changes to avoid scrollback growth; tasks.json parsing accepts numeric IDs.
- [2026-01-14 10:31] Per-tab root/star isolation: aoc-launch and aoc-new-tab now create sanitized tab+timestamp root/star files, inject root_file into per-tab temp layouts, and plugin reads root file directly; cleanup removes state files older than 30 days.
- [2026-01-15 22:35] Added aoc-session-watch idle watcher: sessions named aoc-<tag> rename to idle-<tag> on detach, prune non-agent processes by session pid, and auto-resurrect by opening a new full layout tab on reattach. aoc-launch now starts watcher and names sessions; aoc-new-tab realigns current pane to per-tab root; aoc-star uses per-tab AOC_STAR_FILE.
- [2026-01-17 09:15] Fixed per-tab root isolation: layout templates now use __AOC_PROJECT_ROOT_FILE__ placeholders that get replaced at layout generation; aoc-align and aoc-star now derive per-tab root files from session name (aoc-<root_tag> pattern).
- [2026-01-17 10:08] Fixed per-tab root discovery: aoc-align and aoc-star now discover root_tag from pane names (name="aoc:<root_tag>") via dump-layout instead of session name, enabling multi-tab sessions with different project roots per tab.
- [2026-01-17 10:08] Fixed aoc-launch session creation: now uses --new-session-with-layout flag (Zellij 0.43+) to always create new sessions instead of attempting to attach to non-existent sessions.
- [2026-01-17 10:08] Fixed Taskmaster plugin write-back: replaced non-existent exec_cmd() calls with run_command_with_env_variables_and_cwd() so status changes via TUI actually persist to tasks.json.
- [2026-01-17 10:08] KNOWN ISSUE: Orphaned opencode processes accumulate (14 sessions using 18GB RAM observed). Session/pane cleanup needs optimization - aoc-session-watch may not be killing processes properly on tab close.
- [2026-01-17 17:52] Per-tab root isolation v2: yazi pane now has name="aoc:<root_tag>" attribute; aoc-align and aoc-star use dump-layout to discover root_tag from pane names in current tab (not session name), enabling multi-tab sessions with different project roots.
- [2026-01-17 18:23] Refined Zellij layouts to use explicit placeholder injection (__AOC_PROJECT_ROOT_FILE__, __AOC_STAR_FILE__) for robust per-tab environment isolation, solving the limitation of Zellij not propagating env vars to new tabs.
- [2026-01-17 18:30] Fixed Taskmaster plugin scrollback growth and redundant rendering. Implemented render throttling (cache check) and cursor homing (ANSI Home) in the plugin render loop. Optimized state updates to avoid marking dirty on identical data reads.
- [2026-01-17 18:40] Refactored aoc-session-watch to strictly enforce 'Kill on Detach'. Sessions are now immediately deleted when the client disconnects, solving the memory leak issue caused by zombie agent processes.
- [2026-01-17 18:47] Created aoc-cleanup utility to identify and kill orphaned agent processes (zombies) that are not descendants of any active Zellij session.
- [2026-01-17 18:52] Hardened Tmux configuration (codex-tmux.conf) with 'destroy-unattached on' and 'exit-empty on' to ensure agent sessions self-destruct when Zellij panes close. Reduced history-limit to 10k to conserve RAM.
- [2026-01-17 20:10] Switched default editor to `micro` to improve developer UX and prevent terminal glitching caused by Vim/Neovim modal confusion. Implemented enforcement via `EDITOR` in `.bashrc`, wrapper script `bin/tm-editor`, and explicit environment injection in Zellij layout (`aoc.kdl`).
- [2026-01-17 20:30] Hardened `aoc-session-watch` with safety timeouts and consecutive idle requirements (3x) to prevent premature session termination during slow Zellij responses.
- [2026-01-17 20:30] Updated `aoc-cleanup` with a protected whitelist for cockpit helper scripts and more specific agent process regex to prevent accidental termination of active session management components.
- [2026-01-17 20:53] Refactored context system: migrated .gemini/ to .aoc/, implemented universal context handshake/injection in aoc-agent-wrap, and added .cursorrules symlink support.
