# AOC Architecture & Agent Guidelines

This document details the **Distributed Cognitive Architecture** of the Agent Ops Cockpit. If you are an agent working in this environment, this is your operating manual.

## The Cognitive Stack

AOC separates information into three layers to prevent context window overflow and ensure long-term coherence.

### 1. Context (Orientation)
*   **File:** `.aoc/context.md`
*   **Nature:** Reactive, Auto-generated.
*   **Content:** File tree, `README.md` snapshot, basic operational rules.
*   **Agent Action:** **Read Only.** This file is updated in real-time by the `aoc-watcher` background service. You do not need to run `aoc-init` manually unless the watcher is stopped.

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

### 3. Tasks (Execution)
*   **File:** `.taskmaster/tasks/tasks.json`
*   **Nature:** Dynamic, State-driven.
*   **Content:** The immediate work queue.
*   **Agent Action:**
    *   **Read:** `task-master list` to find work.
    *   **Write:** `task-master add-task` to plan steps. `task-master set-status` to report progress.

## Standard Workflow

When you receive a high-level request (e.g., "Refactor the login system"):

1.  **Orient:** Run `aoc-init` (if needed) or check `.aoc/context.md` to see the files involved.
2.  **Recall:** Run `aoc-mem read` (or `aoc-mem search "login"`) to see past decisions about the login system.
3.  **Plan:** Run `task-master add-task --prompt "Refactor login"` to break it down.
4.  **Execute:** Edit files, run tests.
5.  **Update:** `task-master set-status <id> done`.
6.  **Record:** `aoc-mem add "Refactored login to use OAuth2 provider X."`

## Toolchain Reference

*   `aoc-init`: Bootstraps the `.aoc` and `.taskmaster` folders.
*   `aoc-watcher`: Background service that keeps `context.md` updated in real-time.
*   `aoc-mem`: CLI for managing `memory.md`.
*   `task-master` / `aoc-taskmaster`: CLI for managing tasks.
*   `aoc-doctor`: Validates system dependencies.
