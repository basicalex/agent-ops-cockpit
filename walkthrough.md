# AOC Walkthrough - Animation Script

**Project:** AOC (Agent Ops Cockpit) Walkthrough Video  
**Format:** Remotion Animation  
**Target Duration:** 4-5 minutes  
**Resolution:** 1920x1080 (Full HD)  
**Frame Rate:** 30fps

---

## Overview

This document provides a scene-by-scene breakdown for creating an animated walkthrough of AOC. Each scene includes visual descriptions, narration scripts, on-screen text, and technical notes for Remotion implementation.

---

## Scene Structure Template

Each scene follows this format:
- **Scene ID:** Unique identifier
- **Duration:** Estimated seconds
- **Visual:** What appears on screen
- **Narration:** Voiceover script
- **On-Screen Text:** Text overlays
- **Actions:** User interactions to animate
- **Technical Notes:** Implementation details

---

## ACT I: Introduction & Hook (0:00-0:30)

### Scene 1: Title Card (0:00-0:05)
**Duration:** 5 seconds

**Visual:**
- Black screen
- AOC Logo fades in (centered, large)
- Subtle terminal cursor blink animation

**Narration:** *"Introducing AOC - the terminal-first AI workspace"*

**On-Screen Text:**
```
    ___    ____   ____
   /   \  /    \ /   /
  /  A  \/  O   /  C  /
 /_______/_____/_____/

Agent Ops Cockpit
```

**Technical Notes:**
- ASCII art animation: characters type in sequentially
- Cursor blink: 1-second interval
- Fade in from 0% to 100% opacity over 2 seconds

---

### Scene 2: The Problem (0:05-0:20)
**Duration:** 15 seconds

**Visual:**
- Split screen showing chaotic workflow:
  - Left: Browser with 5+ tabs open (GitHub, ChatGPT, docs)
  - Center: Terminal window with multiple disconnected sessions
  - Right: Code editor with comments scattered everywhere
- Frustration indicators: red X marks, clutter animations

**Narration:** *"Are you tired of context switching between your terminal, file manager, AI chat windows, and task lists? Every time you start a new AI conversation, you lose all context. Your project knowledge is scattered across browser tabs, editor comments, and sticky notes."*

**On-Screen Text:**
- "Lost Context" (appears with red X)
- "Fragmented Workflow" (appears with red X)
- "Manual Copy-Pasting" (appears with red X)

**Actions:**
- Tabs rapidly switching (every 0.5 seconds)
- Copy-paste animation between windows
- Windows overlapping chaotically

**Technical Notes:**
- Use window chrome/mockups, not actual screenshots
- Red X marks fade in sequentially
- Clutter effect: elements shake slightly

---

### Scene 3: The Solution Teaser (0:20-0:30)
**Duration:** 10 seconds

**Visual:**
- All chaotic windows fade out
- AOC interface fades in (simplified layout diagram)
- Four quadrants light up one by one with color highlights

**Narration:** *"What if everything was integrated? One terminal workspace where your AI agents remember everything, tasks stay organized, and files are always at your fingertips."*

**On-Screen Text:**
```
┌──────────┬──────────┬──────────┐
│  YAZI    │  AGENT   │  WIDGET  │
│  Files   │   AI     │Calendar/ │
│          │          │  Media   │
├──────────┼──────────┴──────────┤
│          │    TASKMASTER       │
│  CODE    │   Task Management   │
│          │                     │
└──────────┴─────────────────────┘
```

**Actions:**
- Quadrants highlight in sequence: Yazi (blue), Agent (green), Widget (purple), Taskmaster (orange)
- Smooth transition from chaos to order

**Technical Notes:**
- Use color coding: #3B82F6 (blue), #10B981 (green), #8B5CF6 (purple), #F59E0B (orange)
- Staggered highlight delays: 0.5s between each

---

## ACT II: Installation & First Launch (0:30-1:00)

### Scene 4: One-Line Install (0:30-0:40)
**Duration:** 10 seconds

**Visual:**
- Clean terminal window (centered, large)
- Command types in character by character
- Success indicators appear as installation completes

**Narration:** *"Getting started is incredibly simple. Just run one command to install AOC, initialize your project, and launch the workspace."*

**On-Screen Text:**
```bash
$ ./install.sh && aoc-init && aoc

>> Installing scripts...
>> Building Rust components...
>> AOC Installed Successfully!
>> Run 'aoc' to start.
```

**Actions:**
- Typing animation: 50ms per character
- Progress indicators animate: dots, checkmarks
- Final success message with green checkmark

**Technical Notes:**
- Use monospaced font (JetBrains Mono or similar)
- Green color: #10B981 for success
- Cursor blink during typing, solid after complete

---

### Scene 5: First Launch - The Layout (0:40-1:00)
**Duration:** 20 seconds

**Visual:**
- Full AOC layout appears (simulated/real screenshot)
- Panes slide in from edges to form layout
- Labels appear on each pane
- Zoom into each section briefly

**Narration:** *"This is your AOC workspace. On the left, Yazi file manager gives you keyboard-driven file navigation with rich previews. Center-top is your AI agent pane - this is where Codex, Gemini, Claude, or OpenCode live. Center-bottom is Taskmaster, your interactive task manager. And on the right, you have widgets for calendar, clock, and media. Everything is connected."*

**On-Screen Labels:**
- "📁 YAZI - File Manager" (left pane)
- "🤖 AGENT - AI Interface" (center-top)
- "📋 TASKMASTER" (center-bottom)
- "📅 WIDGETS" (right column)

**Actions:**
- Pane 1 slides in from left (0.3s)
- Pane 2 slides in from top (0.3s, 0.1s delay)
- Pane 3 slides in from bottom (0.3s, 0.2s delay)
- Pane 4 slides in from right (0.3s, 0.3s delay)
- Brief zoom into each pane (2s each)

**Technical Notes:**
- Use actual AOC screenshots or high-fidelity mockups
- Smooth easing functions for slides
- Labels fade in after panes settle
- Zoom: scale 1.0 → 1.2 → 1.0 with pan

---

## ACT III: Core Workflows (1:00-3:00)

### Scene 6: File Management with Yazi (1:00-1:25)
**Duration:** 25 seconds

**Visual:**
- Focus on Yazi pane (left side)
- File tree visible with syntax highlighting
- Demo project structure shown
- File selection and preview animations

**Narration:** *"Let's start with Yazi. Navigate with arrow keys or vim bindings. Press Enter to open a file - watch how the pane automatically expands for better visibility. You can preview images, PDFs, even LaTeX files. Press 'e' to edit directly in micro, a modern terminal editor. And here's the magic: press 'S' to star this directory, instantly re-anchoring all your panes to this project root."*

**On-Screen Text:**
```
YAZI SHORTCUTS:
↑↓ Navigate  Enter Open/Expand  e Edit
y Set Media  p Preview  S Star Directory
```

**Actions:**
- Arrow keys navigate through file tree (highlight moves)
- Press Enter: file opens, pane expands animation
- Press 'e': micro editor opens with syntax highlighting
- Press 'S': star icon appears, all panes briefly flash to show re-anchor

**Demo Project Structure:**
```
my-project/
├── src/
│   ├── main.rs
│   └── lib.rs
├── tests/
├── docs/
└── README.md
```

**Technical Notes:**
- Show actual file icons (nerd fonts style)
- Syntax highlighting visible in editor
- Pane expansion: width 25% → 40% → 25%
- Star animation: gold star icon spins and settles

---

### Scene 7: Working with AI Agents (1:25-1:55)
**Duration:** 30 seconds

**Visual:**
- Focus on Agent pane (center-top)
- Terminal with AI agent prompt visible
- Context file (.aoc/context.md) shown briefly
- Split view: agent pane + context file

**Narration:** *"Now the real power: AI agents that actually understand your project. When you start AOC, it automatically generates a context file with your project structure and README. Your AI agent can see this immediately. Watch - I'll ask Codex to refactor the main function, and notice how it already knows the project structure, dependencies, and coding style without me copying anything."*

**On-Screen Split:**
```
┌─────────────────────┬──────────────────────┐
│ 🤖 Agent Pane       │ 📄 .aoc/context.md   │
│                     │                      │
│ > Refactor main.rs  │ Project: my-project  │
│   to use async      │ Structure:           │
│                     │ - src/main.rs        │
│ [Agent thinks...]   │ - src/lib.rs         │
│                     │ Dependencies:        │
│ Here's the async    │ - tokio = "1.0"      │
│ version using       │ - serde = "1.0"      │
│ tokio::main...      │                      │
└─────────────────────┴──────────────────────┘
```

**Actions:**
- User types: "Refactor main.rs to use async/await"
- Context file highlights key sections
- Agent response types in progressively
- Code diff shows in side panel

**Technical Notes:**
- Typing speed: 100ms per character for user, 30ms for AI
- Context file scrolls to show relevant sections
- Code syntax highlighting with color
- Diff highlighting: green for additions

---

### Scene 8: Task Management (1:55-2:20)
**Duration:** 25 seconds

**Visual:**
- Focus on Taskmaster pane (center-bottom)
- Task list visible with checkboxes
- Subtasks expand/collapse
- Progress bars animate

**Narration:** *"While the agent works, let's track our progress in Taskmaster. This isn't just a todo list - it's a full project task manager with subtasks, priorities, and dependencies. Press 't' to switch between project contexts, 'x' to mark tasks done, and Space to expand subtasks. Everything persists automatically to tasks.json. Your AI agent can even read this to understand what you're working on."*

**On-Screen Task List:**
```
[MASTER] my-project v0.1.0
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
☐ Setup project structure          [high]
☑ Initialize repository            [high]
☐ Implement core features          [high]
  ├─ ☐ Add async main              [med]
  ├─ ☐ Setup error handling        [med]
  └─ ☐ Write tests                 [low]
☐ Documentation                    [low]

Progress: [██████░░░░] 25%
```

**Actions:**
- Press 't': tag switches from [master] to [feature/async]
- Press 'x': task marks complete with checkmark animation
- Press Space: subtasks expand with slide animation
- Press 'f': filter cycles through All → Pending → Done

**Technical Notes:**
- Checkbox animation: ☐ → ☑ with checkmark stroke animation
- Progress bar: fill animation 0% → actual%
- Color coding: high (red), med (yellow), low (green)
- Use Nerd Font icons for visual polish

---

### Scene 9: Multi-Agent Workflow (2:20-2:45)
**Duration:** 25 seconds

**Visual:**
- Split screen showing multiple agent panes
- Agent selector interface
- Different agents working on different tasks
- Context isolation visualization

**Narration:** *"AOC doesn't lock you into one AI. You can work with multiple agents, each in their own isolated context. Switch between Codex, Gemini, Claude, and OpenCode. Each agent gets its own project memory and task list. Watch me switch from Codex to Claude for a code review - Claude immediately sees the same context but brings its own perspective. Perfect for getting second opinions or using the best tool for each job."*

**On-Screen Interface:**
```
┌─────────────────────────────────────────┐
│ SELECT AGENT:                          │
│                                         │
│   🤖 Codex       - General coding       │
│   🔷 Gemini      - Architecture review  │
│   🟣 Claude      - Code review          │
│   🟢 OpenCode    - Quick fixes          │
│                                         │
│ Context: isolated per agent             │
│ Memory: .aoc/memory.md                  │
└─────────────────────────────────────────┘
```

**Actions:**
- Command: `aoc-agent --set` opens selector
- Select Claude: interface transitions
- New pane opens with Claude Code prompt
- Brief side-by-side view: Codex pane + Claude pane
- Both panes show same file structure but different agent responses

**Technical Notes:**
- Selector: fzf-style fuzzy finder interface
- Transition: smooth fade between agents
- Side-by-side: split screen 50/50
- Different color themes per agent (Codex: blue, Gemini: purple, Claude: orange)

---

### Scene 10: The Three-Layer Architecture (2:45-3:00)
**Duration:** 15 seconds

**Visual:**
- Diagram showing Context, Memory, and Task layers
- Data flow animations between layers and agent
- Each layer highlights as it's explained

**Narration:** *"This all works because of AOC's distributed cognitive architecture. Three layers: Context keeps your project map updated automatically. Memory stores your architectural decisions permanently. And Tasks track your dynamic work queue. Your AI agents can read all three, giving them complete project awareness that persists across sessions."*

**On-Screen Diagram:**
```
         AI AGENT
            │
    ┌───────┼───────┐
    │       │       │
    ▼       ▼       ▼
┌───────┬───────┬───────┐
│CONTEXT│MEMORY │ TASKS │
│       │       │       │
│Reactive│Permanent│Dynamic│
│       │       │       │
│Auto-  │Append-│Real-  │
│updated│only   │time   │
└───────┴───────┴───────┘
```

**Actions:**
- Data flow arrows animate: Agent ↔ each layer
- Each layer pulses when mentioned
- File icons appear in Context, text in Memory, checkboxes in Tasks

**Technical Notes:**
- Arrow animation: dashed lines with moving dots
- Pulse effect: scale 1.0 → 1.05 → 1.0
- Layer colors: Context (blue), Memory (purple), Tasks (green)

---

## ACT IV: Advanced Features (3:00-3:45)

### Scene 11: RLM - Large Codebase Analysis (3:00-3:15)
**Duration:** 15 seconds

**Visual:**
- Terminal showing RLM commands
- Codebase scan visualization
- File tree with size/line count metrics
- Chunking animation

**Narration:** *"Working with a massive codebase? AOC includes RLM - Recursive Language Model tooling. Instead of overwhelming your AI with thousands of files, RLM scans, peeks, and chunks intelligently. Measure repository scale, search across the codebase, and process only relevant files. Perfect for enterprise projects or monorepos."*

**On-Screen Commands:**
```bash
$ aoc-rlm scan
Repository Analysis:
━━━━━━━━━━━━━━━━━━━━
Total Files: 1,247
Lines of Code: 89,432
Languages: Rust (60%), TypeScript (25%), Python (15%)

$ aoc-rlm peek "authentication"
Found in:
- src/auth/mod.rs
- src/middleware/auth.rs
- tests/auth_tests.rs

$ aoc-rlm chunk --pattern "src/auth/*.rs"
Processing 12 files in 3 chunks...
```

**Actions:**
- Command typing animations
- Progress bars for scanning
- File tree with heatmap coloring (size = color intensity)
- Chunk visualization: files group into buckets

**Technical Notes:**
- Heatmap: green (small) → yellow (medium) → red (large)
- Chunk animation: files fly into groups
- Metrics count up: 0 → 1,247 with number animation

---

### Scene 12: Custom Layouts - AOC Modes (3:15-3:30)
**Duration:** 15 seconds

**Visual:**
- Layout switching animation
- Multiple layout presets shown
- Custom layout creation demonstration
- Before/after comparison

**Narration:** *"One size doesn't fit all. AOC supports custom layouts - we call them AOC Modes. Use the minimal layout for focused coding, the review layout for PR reviews, or create your own. Each layout automatically injects your project context so terminals start in the right directory, with the right agent, every time."*

**On-Screen Layouts:**
```
┌─────────────────────────────────────────┐
│ LAYOUT SELECTOR                         │
│                                         │
│ [aoc]      ████████████  Full cockpit   │
│ [minimal]  ████████░░░░  Focus mode     │
│ [review]   ██████░░░░░░  Code review    │
│ [writing]  ████░░░░░░░░  Documentation  │
│                                         │
│ Create custom: aoc-new-tab --layout     │
└─────────────────────────────────────────┘
```

**Actions:**
- Layout switch animation: morph between layouts
- Show 3-4 different layouts in quick succession
- Custom layout code: show KDL template
- Project root placeholder highlight: `__AOC_PROJECT_ROOT__`

**Technical Notes:**
- Layout morph: smooth transition between pane configurations
- Highlight color: yellow for placeholders
- Show layout files: `~/.config/zellij/layouts/*.kdl`

---

### Scene 13: Widget Deep Dive (3:30-3:45)
**Duration:** 15 seconds

**Visual:**
- Widget pane showcase
- Media rendering (image → ASCII art)
- Calendar and clock customization
- Gallery mode with image navigation

**Narration:** *"Don't forget the widgets. Render images as ASCII art right in your terminal - perfect for reference images or diagrams. Set a media path from Yazi with just 'y', or browse your gallery. Customize rendering styles, color depth, even font ratios. And yes, videos work too - animated ASCII playback."*

**On-Screen Widget Demo:**
```
┌────────────────────────────┐
│  WIDGET PANE               │
│                            │
│  🖼️  [Media Mode]          │
│                            │
│  +------------------+      │
│  |   ##%%%%%%%##    |      │
│  |  ##         ##   |      │
│  | ##   O   O   ##  |      │
│  | ##     ##     ## |      │
│  |  ##   ####   ##  |      │
│  |   ##%%%%%%%##    |      │
│  +------------------+      │
│                            │
│  m:media g:gallery p:path  │
│  s:style C:colors +/-:size │
└────────────────────────────┘
```

**Actions:**
- Press 'y' in Yazi: image path copies to widget
- Press 'm': switch to media mode
- Press 's': cycle through ASCII styles (different character sets)
- Press 'C': cycle color modes (mono → 16-color → 256-color)
- Press '+': image zooms in (ASCII gets larger)

**Technical Notes:**
- ASCII art: use actual chafa output or realistic simulation
- Style cycling: show different character sets (#, @, %, etc.)
- Color cycling: visible color changes in ASCII
- Smooth zoom: 0.5s transition

---

## ACT V: Real-World Scenario (3:45-4:30)

### Scene 14: Complete Development Workflow (3:45-4:30)
**Duration:** 45 seconds

**Visual:**
- Full AOC layout active
- Realistic development scenario played out
- Time-lapse effect showing progress
- All components working together

**Narration:** *"Let's see AOC in action with a real workflow. I'm building a new feature. First, I create a task in Taskmaster and add subtasks. Then I open the relevant files in Yazi and ask my AI agent to help implement the database layer. The agent reads the existing context, checks the memory for architectural decisions, and starts coding. I review the changes in the git diff, update the task status, and add a note to memory about the new pattern we're using. Everything stays synchronized - the agent knows the project state, the tasks track our progress, and the memory preserves our decisions for next time. This is AI-assisted development that actually works."*

**Scenario Steps:**
1. Create task: "Implement user authentication" [high]
2. Add subtasks: "Setup JWT", "Create middleware", "Write tests"
3. Navigate to auth files in Yazi
4. Ask agent: "Create JWT authentication middleware using the existing database schema"
5. Agent responds with code implementing AuthMiddleware
6. Git diff shows new files: src/auth/mod.rs, src/middleware/auth.rs
7. Mark subtask "Setup JWT" complete (press 'x')
8. Add to memory: "Using JWT with 24h expiry, bcrypt for hashing"
9. Final view: all panes showing synchronized state

**On-Screen Split View:**
```
┌─────────────────────────────────────────────────────────────┐
│  WORKFLOW DEMO: User Authentication Feature                 │
├──────────────┬──────────────┬──────────────┬────────────────┤
│ 📁 YAZI      │ 🤖 AGENT     │ 📋 TASKS     │ 📅 WIDGET      │
│              │              │              │                │
│ src/         │ > Create JWT │ ☐ Auth       │ 14:32          │
│ ├─ auth/     │   middleware │ ☑ JWT        │ [calendar]     │
│ ├─ middleware│ [thinking...]│ ☐ Tests      │                │
│ ├─ db/       │              │              │                │
│ └─ main.rs   │ Here's the   │              │                │
│              │ JWT setup... │              │                │
├──────────────┴──────────────┴──────────────┴────────────────┤
│ Status: 1 of 3 subtasks complete | Memory: JWT pattern saved │
└─────────────────────────────────────────────────────────────┘
```

**Actions:**
- Step-by-step workflow demonstration
- Task creation animation
- File navigation in Yazi
- Agent interaction with typing
- Real-time task status updates
- Memory append animation
- Git status integration
- Final synchronized state

**Technical Notes:**
- Time-lapse effect: 45 seconds = ~15 minutes real work
- Smooth transitions between steps
- Highlight active pane for each step
- Progress indicator at bottom

---

## ACT VI: Conclusion (4:30-4:50)

### Scene 15: Benefits Summary (4:30-4:40)
**Duration:** 10 seconds

**Visual:**
- Checklist of benefits appears
- Each item checks off with animation
- Background shows AOC layout faintly

**Narration:** *"AOC brings it all together: persistent project context that your AI agents can actually use, integrated task management, multi-agent support, and a terminal-native workflow that stays out of your way. No more context switching. No more lost conversations. Just you, your code, and AI that understands your project."*

**On-Screen Checklist:**
```
✅ Persistent AI context across sessions
✅ Integrated task & subtask management
✅ Multi-agent workflow support
✅ Automatic project memory
✅ Terminal-native & keyboard-driven
✅ Customizable layouts
✅ Large codebase analysis tools
```

**Actions:**
- Each checkmark appears with a "ding" sound effect (visual only)
- Staggered appearance: 0.8s between each
- Background: faint animated AOC interface

**Technical Notes:**
- Checkmark animation: stroke draws from left to right
- Sound indicator: speaker icon pulses (visual representation)
- Faint background: 10% opacity AOC layout animation

---

### Scene 16: Call to Action (4:40-4:50)
**Duration:** 10 seconds

**Visual:**
- Terminal appears with install command
- GitHub link and documentation links
- AOC logo returns with tagline
- Fade to black

**Narration:** *"Ready to transform your AI-assisted development? Install AOC today and experience the difference that true context-awareness makes. Visit our GitHub repository for documentation, examples, and community support."*

**On-Screen Text:**
```
Get Started:

$ ./install.sh && aoc-init && aoc

GitHub: github.com/your-org/agent-ops-cockpit
Docs:   Full guides at docs/

┌──────────────────────────┐
│     ___    ____   ____   │
│    /   \  /    \ /   /   │
│   /  A  \/  O   /  C  /   │
│  /_______/_____/_____/    │
│                           │
│  Agent Ops Cockpit        │
│  Context-Aware Development│
└──────────────────────────┘
```

**Actions:**
- Install command types in (loop 2x)
- GitHub link appears with icon
- Logo animates in with tagline
- Fade to black

**Technical Notes:**
- Command typing loop: show twice with cursor blink
- GitHub icon: Octocat logo
- Final logo: larger scale, centered
- Fade: 1-second fade to black

---

## Technical Implementation Notes

### Font Recommendations
- **Terminal:** JetBrains Mono, Fira Code, or Cascadia Code
- **UI Elements:** Inter or SF Pro
- **ASCII Art:** Monospace with Nerd Font icons

### Color Palette
- **Background:** #1E1E1E (dark terminal)
- **Primary:** #3B82F6 (blue - AOC brand)
- **Success:** #10B981 (green)
- **Warning:** #F59E0B (orange)
- **Error:** #EF4444 (red)
- **Text:** #E5E7EB (light gray)
- **Accent:** #8B5CF6 (purple)

### Animation Timing Standards
- **Typing speed:** 50ms per character (user), 30ms (AI)
- **Pane transitions:** 300ms with ease-in-out
- **Fade durations:** 500ms
- **Stagger delays:** 100-200ms between items
- **Highlight pulses:** 500ms cycle

### Remotion Composition Structure
```typescript
// Suggested file structure
src/
├── scenes/
│   ├── Scene01_Title.tsx
│   ├── Scene02_Problem.tsx
│   ├── Scene03_Solution.tsx
│   ├── Scene04_Install.tsx
│   ├── Scene05_Layout.tsx
│   ├── Scene06_Yazi.tsx
│   ├── Scene07_Agent.tsx
│   ├── Scene08_Tasks.tsx
│   ├── Scene09_MultiAgent.tsx
│   ├── Scene10_Architecture.tsx
│   ├── Scene11_RLM.tsx
│   ├── Scene12_Layouts.tsx
│   ├── Scene13_Widget.tsx
│   ├── Scene14_Workflow.tsx
│   ├── Scene15_Benefits.tsx
│   └── Scene16_CTA.tsx
├── components/
│   ├── Terminal.tsx
│   ├── Pane.tsx
│   ├── CodeBlock.tsx
│   ├── Diagram.tsx
│   └── ASCIIArt.tsx
└── Root.tsx
```

### Voiceover Notes
- **Pacing:** Moderate, professional but enthusiastic
- **Tone:** Educational, empowering
- **Target Audience:** Developers familiar with terminal/AI tools
- **Duration:** ~4 minutes total narration

---

## Assets Needed

### Graphics
- [ ] AOC ASCII art logo (high resolution)
- [ ] Pane layout diagrams (SVG)
- [ ] Architecture diagram (SVG)
- [ ] Screenshot/mockup of full AOC interface
- [ ] File manager icons (folder, file types)
- [ ] Checkmark, star, and UI icons

### Fonts
- [ ] JetBrains Mono (terminal text)
- [ ] Inter (UI elements)
- [ ] Nerd Font patched version (for icons)

### Audio (Optional)
- [ ] Background music track (ambient, non-intrusive)
- [ ] UI sound effects (optional - can be visual only)

---

## Version History

- **v1.0** (2026-01-31) - Initial walkthrough script

---

**End of Walkthrough Script**
