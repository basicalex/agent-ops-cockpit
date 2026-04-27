# DESIGN.md

This is the project-wide visual and product design contract for Agent Ops Cockpit (AOC). Agents must read it before changing AOC control surfaces, docs presentation, product-facing UI, HyperFrames/media flows, themes, or visual assets.

## Product experience

- Product/project: Agent Ops Cockpit — a Pi-first agent operations cockpit for context, tasks, memory, skills, layouts, and production modes.
- Primary audience: developers and AI-assisted builders operating complex projects from a terminal-first workflow.
- Primary promise: make agent work observable, controllable, recoverable, and production-ready without burying the operator in noise.
- Desired emotional impression: calm command center, sharp engineering instrument, trustworthy automation.
- Trust/energy level: high-trust, low-friction, focused, quietly powerful.

## Brand personality

- Voice: concise, operational, confident, helpful.
- Mood: cockpit, mission control, workshop, precise craft.
- Keywords: signal, provenance, control, continuity, production, focus.
- Avoid: toy-like UI, vague magic, gratuitous animation, excessive color, noisy dashboards.

## Visual principles

1. **Signal over spectacle** — show actionable state, progress, and next steps before decoration.
2. **Operator control** — every long-running or risky flow should expose status, logs, cancel/retry paths, and safe defaults.
3. **Project continuity** — AOC surfaces should reinforce durable artifacts: tasks, PRDs, DESIGN.md, STM, memory, commits, and provenance.

## Layout system

- Density: compact terminal-first density with clear grouping and whitespace between conceptual sections.
- Grid/container rules: prefer two-pane or three-pane command-center layouts; left/selectors, right/details, bottom/status/logs.
- Spacing scale: small consistent gaps; avoid sprawling forms.
- Responsive behavior: prioritize readable text and visible current action over decorative panes.
- Empty/loading/error-state layout rules: always show what is missing, why it matters, and the smallest next command.

## Color system

| Token | Value | Usage | Notes |
| --- | --- | --- | --- |
| `--color-bg` | dark neutral | Primary terminal/cockpit background | Keep low glare |
| `--color-surface` | slightly lifted neutral | Panels/cards/details | Subtle contrast |
| `--color-primary` | cyan/blue family | Active controls, selected state, command focus | Avoid overuse |
| `--color-accent` | violet/amber selectively | Important production modes, warnings, highlights | Use sparingly |
| `--color-text` | high-contrast neutral | Main text | Accessibility first |
| `--color-muted` | muted neutral | Secondary detail/help | Must remain readable |
| `--color-success` | green | Successful checks/completions | Do not rely on color alone |
| `--color-warning` | amber | Warnings/non-blocking issues | Include text label |
| `--color-danger` | red | Destructive/error states | Require explicit operator intent |

## Typography

- Primary font: terminal/default monospace for TUI and command output.
- Secondary/fallback font: system sans for generated docs/sites where applicable.
- Heading style: short noun phrases; avoid marketing fluff in control surfaces.
- Body style: concise instructions and status summaries.
- Numeric/metric style: aligned when possible; include units.
- Line-height/measure notes: optimize for scanability in terminal panes.

## Component rules

- Buttons/actions: label by outcome, not implementation detail, unless command transparency is useful.
- Cards/panels: each panel should have a clear state or decision purpose.
- Forms/inputs: preserve defaults; make destructive changes explicit.
- Navigation: keep Alt+C control hierarchy shallow and predictable.
- Tables/lists: sort stable, show status and path where possible.
- Modals/dialogs: reserve for confirmation, help, and focused detail.
- Notifications/toasts/status lines: include command result and relevant log/path.
- Icons: optional; text labels must carry meaning.

## Motion and interaction

- Motion personality: minimal, functional, progress-oriented.
- Duration range: short and subtle where graphical UI exists.
- Easing: calm, non-bouncy.
- What should animate: progress, loading, transitions that clarify state.
- What should not animate: critical logs, warnings, destructive confirmations.
- Reduced-motion expectations: all functionality must work without motion.

## Imagery and media

- Image style: crisp product screenshots, terminal captures, architectural diagrams.
- Illustration style: diagrammatic rather than decorative.
- Iconography style: simple, symbolic, terminal-compatible.
- Screenshot/product-frame treatment: show real AOC surfaces when possible; annotate sparingly.
- Video/animation treatment: demonstrate operator flow, visible logs, and before/after state.

## Content design

- Tone: concise, clear, operator-centered.
- CTA style: command-oriented: “Initialize”, “Sync”, “Run doctor”, “Open log”.
- Terminology: prefer AOC, Pi, Taskmaster, Mind, STM, PRD, HyperFrames consistently.
- Error message style: name failing command/action, give log path or next repair step.
- Things to avoid: vague success, silent failures, unbounded “magic”, unsupported runtime claims.

## Accessibility requirements

- Contrast: readable in dark terminal themes and generated docs.
- Keyboard/focus behavior: every TUI action must be keyboard-first and discoverable.
- Reduced motion: no essential information should depend on animation.
- Captions/alt text: include for media assets and docs images.
- Minimum readable sizes: avoid tiny text in screenshots and rendered demos.

## Design do / don't

### Do

- Show current status, next action, and relevant path/log.
- Preserve user-authored project artifacts.
- Make background work observable and cancellable.
- Keep design decisions traceable through PRDs, tasks, commits, and memory.

### Don't

- Invent a new visual style for a subsystem without updating this file.
- Hide logs for install/init/doctor flows.
- Overwrite project design docs or assets without explicit confirmation.
- Add decorative complexity that reduces operator confidence.

## Subsystem design extensions

Subsystem-specific design files may extend this document, but should not contradict it.

- HyperFrames/media: `hyperframes/docs/DESIGN.md`
- Presets/layouts: `.aoc/presets/**` and `.aoc/layouts/**`
- Docs/marketing-specific extensions: document locally when introduced

## Agent instructions

When changing UI, visual assets, product copy, documentation presentation, marketing pages, or media:

1. Read this file first.
2. Reuse existing components, tokens, and patterns before inventing new ones.
3. Preserve visual consistency unless the user explicitly requests a design-system change.
4. Update this file when making intentional design-system changes.
5. If a subsystem has its own `DESIGN.md`, treat this root file as the upstream contract and the subsystem file as a specialization.
6. Mention design-impacting changes in task notes, PRs, commits, or handoffs.
