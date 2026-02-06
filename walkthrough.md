# AOC Walkthrough - Screen Recording Script

**Project:** AOC (Agent Ops Cockpit) Walkthrough Video
**Format:** Screen recording + Remotion overlays
**Target Duration:** 4-5 minutes
**Resolution:** 1920x1080 (Full HD)
**Frame Rate:** 30fps

---

## Overview

This walkthrough is built from real screen recordings of AOC in use. Remotion overlays (labels, arrows, callouts, and timing highlights) are added on top of the footage. No simulated UI or mockups are required.

Recording sources:
- AOC repo session (`agent-ops-cockpit`)
- MoreMotion repo session (Remotion project)

---

## Scene Structure Template

Each scene follows this format:
- **Scene ID:** Unique identifier
- **Duration:** Estimated seconds
- **Recording:** What to capture on screen
- **Narration:** Voiceover script
- **On-Screen Text:** Overlay text for Remotion
- **Actions:** User interactions in the recording
- **Technical Notes:** Implementation notes for overlays

---

## ACT I: Launch and Orientation (0:00-0:35)

### Scene 1: Cold Open + Launch (0:00-0:12)
**Duration:** 12 seconds

**Recording:**
- Terminal starts in a non-repo directory
- Show `pwd`, then `cd /path/to/agent-ops-cockpit`
- Run `aoc`

**Narration:** *"We start from any directory, jump into the repo, and launch AOC."*

**On-Screen Text:**
```
Start from any root
cd into project
launch AOC
```

**Actions:**
- `pwd`
- `cd /path/to/agent-ops-cockpit`
- `aoc`

**Technical Notes:**
- Use overlay arrows to emphasize the current working directory change
- Hold 1-2 seconds after `aoc` to let layout settle

---

### Scene 2: Layout Overview (0:12-0:22)
**Duration:** 10 seconds

**Recording:**
- Full AOC layout visible after launch

**Narration:** *"This is the cockpit: files, agents, tasks, widgets, and a terminal, all in one workspace."*

**On-Screen Text:**
- "Yazi - File Manager"
- "Agent"
- "Taskmaster"
- "Widgets"
- "Terminal"

**Actions:**
- None (static shot)

**Technical Notes:**
- Label each pane with a subtle callout

---

### Scene 3: Status Bars + Help (0:22-0:35)
**Duration:** 13 seconds

**Recording:**
- Show Zellij status bars
- Open the help/cheat sheet
- Demonstrate Smart Enter behavior

**Narration:** *"Zellij status bars show active panes and layout context, and help is always one key away."*

**On-Screen Text:**
- "Status bars: layout + active pane"
- "Help and Smart Enter"

**Actions:**
- Open help overlay
- Trigger Smart Enter in the current pane

**Technical Notes:**
- Keep overlays short and non-blocking

---

## ACT II: Navigation and Core Workflows (0:35-2:30)

### Scene 4: Tab Navigation + Reorder (0:35-0:50)
**Duration:** 15 seconds

**Recording:**
- Use the new Alt keybinds

**Narration:** *"Tabs are fast: switch with Alt i/o and reorder with Alt u/p."*

**On-Screen Text:**
```
Alt i/o = prev/next tab
Alt u/p = move tab left/right
Alt [ = toggle pane grouping
```

**Actions:**
- `Alt i` (prev tab)
- `Alt o` (next tab)
- `Alt u` (move tab left)
- `Alt p` (move tab right)
- `Alt [` (toggle group)

**Technical Notes:**
- Add keystroke callouts for each action

---

### Scene 5: Yazi File Flow (0:50-1:20)
**Duration:** 30 seconds

**Recording:**
- Navigate in Yazi, open folder
- Preview a file
- Open a file in micro
- Send media path to widgets

**Narration:** *"Yazi is the file hub: navigate, preview, edit, and send media paths to widgets."*

**On-Screen Text:**
```
Enter = open
p = preview
e = edit in micro
y = send media path
```

**Actions:**
- Arrow keys
- `Enter`
- `p`
- `e`
- `y`

**Technical Notes:**
- Avoid any deprecated star action

---

### Scene 6: Agent Context Awareness (1:20-1:40)
**Duration:** 20 seconds

**Recording:**
- Open `.aoc/context.md`
- Send a prompt to the agent and show context-aware response

**Narration:** *"Agents read project context automatically, so you do not need to paste it in."*

**On-Screen Text:**
- "Context auto-loaded from .aoc/context.md"

**Actions:**
- Open `.aoc/context.md`
- Prompt: "Summarize the project structure"

**Technical Notes:**
- Overlay highlight on the context file heading

---

### Scene 7: Taskmaster Basics (1:40-2:05)
**Duration:** 25 seconds

**Recording:**
- Toggle a task done
- Expand subtasks
- Switch tags
- Filter view

**Narration:** *"Taskmaster tracks real work: status, subtasks, tags, and filters."*

**On-Screen Text:**
```
x = toggle done
Space = expand
t = switch tag
f = filter
? = help
```

**Actions:**
- `x`, `Space`, `t`, `f`, `?`

**Technical Notes:**
- Keep the task list readable; pause after each action

---

### Scene 8: Memory Workflow (2:05-2:20)
**Duration:** 15 seconds

**Recording:**
- `aoc-mem read`
- `aoc-mem add "decision"`

**Narration:** *"Decisions live in memory, so they persist across sessions."*

**On-Screen Text:**
- "aoc-mem read"
- "aoc-mem add"

**Actions:**
- Run two commands

---

### Scene 9: Widgets Deep Dive (2:20-2:30)
**Duration:** 10 seconds

**Recording:**
- Cycle widget modes and styles

**Narration:** *"Widgets handle media, calendar, and quick reference data."*

**On-Screen Text:**
```
m / g / s / C / +/-
```

**Actions:**
- `m`, `g`, `s`, `C`, `+` or `-`

---

## ACT III: Advanced Features (2:30-3:15)

### Scene 10: RLM Quick Demo (2:30-2:45)
**Duration:** 15 seconds

**Recording:**
- Run `aoc-rlm scan`, `aoc-rlm peek`, `aoc-rlm chunk`

**Narration:** *"RLM scans large repos and chunks context for targeted analysis."*

**On-Screen Text:**
```
aoc-rlm scan
aoc-rlm peek "auth"
aoc-rlm chunk --pattern "src/**/*.rs"
```

**Actions:**
- Run the three commands with brief pauses

---

### Scene 11: Custom Layouts (2:45-3:00)
**Duration:** 15 seconds

**Recording:**
- `aoc-new-tab --layout minimal` or `aoc-layout --set minimal`

**Narration:** *"Layouts adapt AOC to your task: focused, review, or full cockpit."*

**On-Screen Text:**
- "aoc-new-tab --layout minimal"

**Actions:**
- Show layout switch

---

### Scene 12: Mission Control (Optional) (3:00-3:15)
**Duration:** 15 seconds

**Recording:**
- Toggle Mission Control (`Alt+a`)
- Show summary list and a patch preview

**Narration:** *"Mission Control surfaces key updates and quick actions."*

**On-Screen Text:**
- "Alt+a Mission Control"

**Actions:**
- Toggle on, then off

---

## ACT IV: MoreMotion Integration (3:15-4:05)

### Scene 13: MoreMotion Intro (3:15-3:25)
**Duration:** 10 seconds

**Recording:**
- Switch to a Remotion repo terminal

**Narration:** *"MoreMotion plugs into AOC to accelerate animation workflows."*

**On-Screen Text:**
- "MoreMotion: AOC + Remotion"

---

### Scene 14: MoreMotion Init + Agent (3:25-3:55)
**Duration:** 30 seconds

**Recording:**
- Run `aoc-momo init`
- Show `.opencode/agents/momo.md`
- Use `@momo` with a short prompt

**Narration:** *"Initialize MoreMotion, then ask the momo agent for animation guidance."*

**On-Screen Text:**
```
@momo
```

**Actions:**
- Run command and open the agent file
- Type a short prompt in OpenCode

**Technical Notes:**
- Keep the prompt small so the response fits on screen

---

## ACT V: Wrap (4:05-4:30)

### Scene 15: Recap + CTA (4:05-4:30)
**Duration:** 25 seconds

**Recording:**
- Return to the AOC full layout
- Hold on a clean shot

**Narration:** *"AOC unifies context, memory, tasks, and tools. MoreMotion extends the workflow into animation. Launch it, use it, and keep momentum."*

**On-Screen Text:**
```
Context + Memory + Tasks
Multi-agent workflow
MoreMotion for animation
```

**Actions:**
- None; use static frame

---

## Technical Implementation Notes

### Recording Requirements
- Record at 1920x1080, 30fps
- Use a consistent terminal theme and font size
- Pause after key actions to allow overlay readability
- Avoid fast scrolling

### Remotion Overlay Guidelines
- Labels and keybind callouts should appear for 1.5-2s
- Use the AOC palette for callouts (blue, green, orange, purple)
- Prefer light overlays over the recording; avoid blocking key UI

### Fonts
- Terminal: JetBrains Mono or Fira Code
- UI overlays: Inter or SF Pro

---

## Assets Needed

### Recordings
- AOC session recording (agent-ops-cockpit repo)
- MoreMotion session recording (Remotion repo)

### Overlays
- Pane labels
- Keybind callouts
- MoreMotion callouts

---

## Version History

- **v2.0** (2026-02-04) - Shift to real screen recording and add MoreMotion integration

---

**End of Walkthrough Script**
