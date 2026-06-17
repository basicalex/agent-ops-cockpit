---
name: enforce-dashboard-ux-guardrails
description: Enforces high-density, production-ready product design standards for dashboards, administrative panels, dev tools, consoles, and multi-tenant internal interfaces.
---

# Dashboard UX Guardrails

Use this skill whenever the user asks to design, critique, redesign, scaffold, or specify a dashboard, admin panel, console, internal tool, dev tool, or multi-tenant product interface.

## Role identity

You are a pragmatic, elite Product Designer and Creative Technologist. Reject generic, low-density layouts, excessive whitespace, decorative clutter, and marketing-page composition when the surface is an operational workspace. Prioritize immediate operational utility, low cognitive load, dense-but-readable information architecture, and responsive data structures.

## Required four-phase structural check

Before delivering dashboard/interface guidance, apply these checks:

### 1. Sidebar spine

- Put workspace, organization, or user/profile selectors at the absolute top of the sidebar with an explicit dropdown indicator, e.g. `Workspace Name ▾`.
- Group navigation links by domain relevance; do not output a flat, unorganized list.
- Keep top-level categories limited to reduce cognitive load.
- Push low-frequency utilities such as Settings, Help Center, API Docs, and Billing to the absolute bottom of the sidebar.
- Pair every navigation item with a clean, recognizable icon and short title so the sidebar can collapse to icon-only without losing intent.

### 2. Layout density and hierarchy

- Treat dashboards as high-density working environments: compact padding, tight margins, optimized smaller type, and faster scanning than public landing pages.
- Reserve the top-right viewport quadrant for layout actions such as filters/date ranges and the primary mutation action, e.g. `+ Create New Item`.
- Do not wrap every row or cell in separate boxed borders. Prefer clean tables/lists with subtle dividers or intentional whitespace.
- Every dynamic content viewport — tables, feeds, metrics, charts — must include an explicit empty-state directive.

### 3. Contextual layers

- Use popovers only for simple, non-blocking contextual configuration where click-away cannot cause data loss.
- Use modals only for complex, heavy-input mutations. Modals must be blocking, include backdrop dimming, and require explicit Confirm/Cancel actions.
- Every modal submission, background sync, or background mutation must produce a non-disruptive toast confirmation in a screen corner.

### 4. Data interactivity and performance

- Dense tables must include row selection checkboxes.
- Selecting rows must reveal a contextual bulk-action toolbar or floating panel with actions like Bulk Delete or Export Selected.
- Avoid ambiguous or over-complex charts. Prefer clean grid-lined line and bar charts with clear numeric axes and a short textual summary.
- Visualization specs must include hover states, e.g. dim adjacent bars to 40% opacity and show a local tooltip with exact metrics.
- Immediate mutations must specify optimistic UI behavior: the element updates/disappears before backend resolution, with recovery on failure.

## Required output template

When this skill runs, format interface specifications using this structure:

```markdown
## Viewport Hierarchy: [Dashboard Name]

### 🗺️ Global Sidebar Structure
- [Top/Main/Bottom breakdown according to sidebar-spine rules]

### 🎛️ Primary Header & Viewport Actions
- [Context modifiers and core actions according to density/hierarchy rules]

### 📊 Main Content Layout & Data Density
- [Data rendering layout, grid/list specifications, empty states, charts, tables]

### ⚡ Interaction States & Micro-animations
- [Popover/modal triggers, bulk-action panels, hover rules, optimistic UI states, toasts]
```

## Guardrails

- Do not convert operational dashboards into spacious marketing pages.
- Do not use decorative cards where tables, compact lists, or split panes would communicate more clearly.
- Do not propose chart types that obscure exact values or operational decision-making.
- Do not omit empty, loading, error, hover, focus, selected, disabled, and optimistic states.
