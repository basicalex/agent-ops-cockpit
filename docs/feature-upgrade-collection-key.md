# Feature & Upgrade Collection Key

This document defines the standard process for collecting, planning, and tracking new features or upgrades in AOC. Agents should follow this structure when the user proposes new functionality.

---

## Process Overview

```
User Idea
    |
    v
[1] Discovery & Analysis
    - Explore codebase for relevant code
    - Read memory.md for past decisions
    - Identify scope and constraints
    |
    v
[2] Scope Clarification (Questions)
    - Target platforms/shells/users?
    - Support level (minimal, full, rewrite)?
    - Priority driver (what problem does this solve)?
    |
    v
[3] Architecture Proposal
    - Current state analysis
    - Proposed solution with options
    - Files to modify
    - Trade-offs
    |
    v
[4] Task Breakdown
    - Parent task with description
    - Subtasks for implementation steps
    - Dependencies noted
    |
    v
[5] Roadmap Integration
    - Add to ROADMAP.md with phase/timeline
    - Note blockers and external dependencies
    - Include contribution opportunities
    |
    v
[6] Memory Recording
    - Log decision in memory.md
    - Reference task ID and roadmap phase
```

---

## Step 1: Discovery & Analysis

Before proposing anything, understand the current state:

```bash
# Read project memory for context
aoc-mem read

# Check existing tasks
aoc task list

# Explore relevant code (agent uses Task tool for deep exploration)
```

**Output:** A summary table of current architecture, affected files, and constraints.

### Example Output

```markdown
| Component | Current State | Impact |
|-----------|--------------|--------|
| Zellij layouts | Hardcoded bash | High - all panes |
| bin/* scripts | bash-only | Medium - internal |
| Taskmaster TUI | native (Ratatui) | Low |
```

---

## Step 2: Scope Clarification

Ask the user targeted questions to narrow scope:

| Question Type | Purpose |
|--------------|---------|
| **Target** | What platforms/shells/users? |
| **Depth** | Minimal fix vs full rewrite? |
| **Driver** | What problem are we solving? |

Keep questions concise with clear options. Avoid open-ended questions when possible.

---

## Step 3: Architecture Proposal

Present a structured proposal:

### Current State
Brief description of how things work now.

### Proposed Solution
- **Option A (Recommended):** Description
- **Option B:** Alternative approach

### Files to Modify

| File | Change | Effort |
|------|--------|--------|
| `path/to/file` | What changes | Low/Med/High |

### Trade-offs
- Pro: ...
- Con: ...

---

## Step 4: Task Breakdown

Create a parent task with subtasks using taskmaster:

```bash
# Add parent task
aoc task add "Feature Name" \
  --priority high \
  --description "One-line summary" \
  --details "Extended explanation"

# Add subtasks (use task ID from above)
aoc task sub add <ID> "Subtask title" --desc "Details"
```

### Subtask Structure

Typical subtasks for a feature:

1. **Core implementation** - The main code change
2. **Supporting utilities** - Helper scripts/functions
3. **Integration** - Connect to existing systems
4. **Configuration** - Env vars, settings
5. **Testing** - Validation with different scenarios
6. **Documentation** - README, inline docs

---

## Step 5: Roadmap Integration

Add the feature to `ROADMAP.md` with this structure:

```markdown
### Phase N: Feature Name (Status)

Brief description of the feature and its goal.

**Goal:** What success looks like.

**Approach:**
- Key implementation points
- Technical decisions

**Tracking:** Task #XX in `.taskmaster/tasks/tasks.json`
```

### Roadmap Conventions

| Status | Meaning |
|--------|---------|
| In Progress | Actively being worked on |
| Planned | Scheduled but not started |
| Research | Investigating feasibility |
| Blocked | Waiting on external dependency |
| Future | Long-term goal, no timeline |

### External Dependencies

If blocked by external projects, include:

```markdown
**We encourage contributors to participate in [Project Name] directly.**
- Link to relevant issue
- How contributing there helps AOC
```

---

## Step 6: Memory Recording

Log the decision for future agents:

```bash
aoc-mem add "Brief summary: what was decided, task ID, roadmap phase."
```

### Memory Entry Format

```
Added FEATURE_NAME: [brief description]. Created task #XX with N subtasks. 
Roadmap Phase N. [Any blockers or key decisions.]
```

---

## Template: New Feature Proposal

When presenting a new feature to the user, use this structure:

```markdown
## [Feature Name] Plan

### Current State Analysis
[Table of affected components]

### Scope
- **In scope:** What we're changing
- **Out of scope:** What stays the same

### Proposed Solution
[Description with options if applicable]

### Implementation Steps
1. [ ] Step one
2. [ ] Step two
...

### Files to Modify
[Table with file, change, effort]

### Questions
[Any decisions needed from user]
```

---

## Example: Multi-Shell Support Feature

This feature was planned using the above process:

### Discovery
- Explored 37+ bash scripts, layout templates, Taskmaster TUI
- Found hardcoded `bash -lc` patterns throughout

### Scope Clarification
- Target: All common shells (fish, zsh, powershell, nushell)
- Depth: Terminal pane only (internal scripts stay bash)
- Driver: Cross-platform support for diverse users

### Task Created
**Task #41:** Implement Multi-Shell Terminal Support
- 6 subtasks covering implementation through documentation

### Roadmap
- Added as Phase 1 in ROADMAP.md
- Noted Zellij Windows limitation as external blocker
- Included contribution encouragement for Zellij project

### Memory
```
Added ROADMAP.md documenting cross-platform vision: multi-shell terminal 
support (Phase 1), script portability (Phase 2), alternative multiplexer 
research (Phase 3), and native Windows (Phase 4, blocked by Zellij). 
Created task #41 for multi-shell implementation with 6 subtasks.
```

---

## Quick Reference

```bash
# Explore codebase
aoc-mem read
aoc task list

# Create feature task
aoc task add "Title" --priority high --description "..."

# Add subtasks
aoc task sub add <ID> "Subtask" --desc "..."

# Record decision
aoc-mem add "Summary of decision and task IDs"

# Update roadmap
# Edit ROADMAP.md with new phase
```

---

## Checklist for Agents

When the user proposes a new feature:

- [ ] Read `memory.md` for relevant past decisions
- [ ] Explore codebase to understand current architecture
- [ ] Ask clarifying questions (scope, priority, constraints)
- [ ] Present structured proposal with options
- [ ] Create parent task with subtasks in taskmaster
- [ ] Add to ROADMAP.md with appropriate phase
- [ ] Record decision in memory.md
- [ ] Summarize what was created for user
