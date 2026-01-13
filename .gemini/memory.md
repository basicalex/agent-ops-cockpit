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
