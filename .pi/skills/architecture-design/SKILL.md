---
name: architecture-design
description: Umbrella skill for product and system architecture decisions. Use when a request involves product architecture, route/system structure, conversion architecture, gamification architecture, onboarding/retention loops, information architecture, cross-cutting UX systems, or choosing which architecture/design specialist skill to load.
compatibility: Designed for Pi Coding Agent and other Agent Skills-compatible coding agents.
metadata:
  version: "0.1.0"
  author: "AOC"
allowed-tools: Read Bash
---

# Architecture Design Umbrella Skill

Use this as a router for cross-cutting product/system architecture work. It should stay concise and route to specialist skills instead of duplicating their full bodies.

## Prime directive

1. Read root `DESIGN.md` before product-facing architecture changes.
2. Preserve existing stack, route conventions, data flows, privacy posture, and accessibility expectations.
3. Load the narrowest specialist skill only when its body is needed.
4. For larger implementation work, align spec/task/subtasks before editing.

## Skill router

| Need | Skill |
|---|---|
| Ethical gamification, progress systems, streaks, quests, rewards, leaderboards, retention mechanics | `safe-gamification` |
| Conversion architecture, landing pages, CTAs, qualification, lead capture, pricing, onboarding, funnel analytics | `funnel-design` |
| Frontend/product-facing design mode selection | `frontend-design` |
| Design system, critique, redesign, polish, handoff, tokens, motion | `design-*` and `motion-director` skills as relevant |
| HyperFrames/media/campaign architecture | `aoc-hyperframes` |
| Task/spec architecture | `spec-rpg-authoring` or Taskmaster flows |

## Architecture review checklist

- What user goal does the system support?
- What product/business goal does it support?
- What routes, data flows, state, events, and handoffs are affected?
- What safety/privacy/ethics constraints apply?
- What is the smallest coherent architecture?
- What should be reusable versus one-off?
- What metrics prove user value rather than only engagement?

## Output expectation

Report:

- specialist skill loaded, if any
- architecture decision
- files/surfaces implicated
- safety/privacy constraints
- validation or next implementation steps
