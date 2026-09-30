---
name: frontend-design
description: Umbrella skill for product-facing frontend design work. Use when a request involves UI/UX, landing pages, conversion funnels, design systems, design review, redesigns, visual polish, motion, interaction, design tokens, or choosing the right existing frontend/design guidance. Keeps root DESIGN.md and implementation context in mind.
compatibility: Designed for Pi Coding Agent and other Agent Skills-compatible coding agents.
metadata:
  version: "1.0.0"
  author: "AOC"
allowed-tools: Read Bash
---

# Frontend Design Umbrella Skill

Use this skill when the user asks for frontend/product-facing work but the exact specialist skill is unclear.

## Prime directive

1. Read root `DESIGN.md` before product-facing UI, copy, layout, docs-site, marketing, or media changes.
2. Preserve existing stack, components, tokens, accessibility, and route conventions.
3. Load a specialist skill only when its body is needed. Do not inject unrelated skills by default.
4. For implementation work, inspect code narrowly before editing and run targeted checks.

## Skill router

Load these skills as needed:

| Need | Skill |
|---|---|
| End-to-end conversion architecture, landing pages, CTAs, lead capture, pricing, onboarding, funnel analytics | `funnel-design` |
| UI critique, redesign, polish, tokens, handoff, or motion | Use root `DESIGN.md` and the existing project conventions |
| Browser visual QA, screenshots, site interaction | `agent-browser` if available |

## Default triage

Ask or infer:

- Is this audit, design/spec, implementation, or review?
- What surface: landing page, app screen, docs site, checkout, onboarding, dashboard, media?
- What outcome: clarity, conversion, activation, trust, polish, accessibility, performance?
- What constraints: existing design system, brand, framework, deadline, compliance/privacy?

## Lightweight operating modes

### UI patch

1. Read `DESIGN.md`.
2. Inspect target route/component.
3. Apply smallest coherent change.
4. Run targeted lint/type/build/test.

### Review/audit

1. Read `DESIGN.md`.
2. Map route/component evidence.
3. Score issues by severity and impact.
4. Recommend fixes with file paths.

### Funnel work

Load `funnel-design`.

### Visual redesign, polish, or motion work

Use root `DESIGN.md`, the existing components and tokens, and the target route's implementation.

## Output expectation

Be concise. Report:

- specialist skill loaded, if any
- design contract/source used
- key files inspected/changed
- validation performed
- next recommended action
