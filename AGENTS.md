# AOC Architecture & Agent Guidelines

This document details the **Distributed Cognitive Architecture** of the Agent Ops Cockpit. If you are an agent working in this environment, this is your operating manual.

## The Cognitive Stack

AOC separates information into three layers to prevent context window overflow and ensure long-term coherence.

### 1. Context (Orientation)
*   **File:** `.aoc/context.md`
*   **Nature:** Auto-generated (reactive if `aoc-watcher` is installed).
*   **Content:** File tree, `README.md` snapshot, basic operational rules.
*   **Agent Action:** **Read Only.** Run `aoc-init` to refresh it manually. If `aoc-watcher` is installed, it updates automatically.
*   **Why it exists:** AGENTS.md is static policy; `.aoc/context.md` is a live project snapshot. Use `aoc-init` (or `aoc-watcher` when available) to keep it current.

### 2. Memory (Wisdom)
*   **File:** `.aoc/memory.md`
*   **Nature:** Persistent, Append-only (Log).
*   **Content:**
    *   **Decisions:** Why we chose library X over Y.
    *   **Evolution:** Major refactors or pivots (e.g., "Switched Gemini to native pane").
    *   **Preferences:** User specific constraints (e.g., "Strict Types only").
*   **Agent Action:**
    *   **Read:** At the start of every task (`aoc-mem read`).
    *   **Write:** When you make a decision that future agents need to know (`aoc-mem add "..."`).
    *   **Rule:** Do not edit `.aoc/memory.md` directly; all interactions must go through `aoc-mem`.

### 3. Tasks (Execution)
*   **File:** `.taskmaster/tasks/tasks.json`
*   **Nature:** Dynamic, State-driven.
*   **Content:** The immediate work queue.
*   **Agent Action:**
    *   **Read:** `aoc task list` to find work.
    *   **Write:** `aoc task add` to plan steps. `aoc task status` to report progress.
    *   **Rule:** Do not edit `.taskmaster/tasks/tasks.json` directly; all interactions must go through `aoc task`.

## Standard Workflow

When you receive a high-level request (e.g., "Refactor the login system"):

1.  **Orient:** Run `aoc-init` (if needed) or check `.aoc/context.md` to see the files involved.
2.  **Recall:** Run `aoc-mem read` (or `aoc-mem search "login"`) to see past decisions about the login system.
3.  **Plan:** Run `aoc task add "Refactor login"` to break it down.
4.  **Execute:** Edit files, run tests.
5.  **Update:** `aoc task status <id> done`.
6.  **Record:** `aoc-mem add "Refactored login to use OAuth2 provider X."`

## Toolchain Reference

*   `aoc-init`: Bootstraps the `.aoc` and `.taskmaster` folders.
*   `aoc-watcher` (optional): Background service that keeps `context.md` updated in real-time.
*   `aoc-mem`: CLI for managing `memory.md`.
*   `aoc task` / `aoc-taskmaster`: CLI for managing tasks.
*   `aoc-doctor`: Validates system dependencies.

## RLM Skill (Default for Large Codebases)
When repository size exceeds your context window, use the Rust-based RLM tool:
*   `aoc-rlm scan` (or `rlm scan`) for scale, `aoc-rlm peek` (or `rlm peek`) for snippets, `aoc-rlm chunk` (or `rlm chunk`) for slicing.
*   Treat this as the default approach for large codebases before deep analysis.
