---
name: design-redesign
description: Audit an existing product or website, identify generic or weak UI patterns, and upgrade it with focused, reviewable design improvements.
---

Use this sequence:
1. Scan the current UI and implementation patterns.
2. Diagnose the most generic, weak, or inconsistent decisions.
3. Upgrade them without rewriting the project from scratch.

Audit these areas:
- hierarchy
- spacing rhythm
- alignment and grouping
- typography and density
- color consistency and contrast
- component/state completeness
- responsiveness
- motion noise versus motion purpose
- implementation quality risks

Use these sections:
- Existing patterns found
- Generic or weak patterns
- Priority upgrades
- Safe implementation path

Upgrade priorities:
1. fix hierarchy and spacing before adding decoration
2. remove the most generic layout patterns first
3. improve buttons, forms, cards, nav, and empty/loading/error states
4. tighten type scale, max-width, and spacing consistency
5. add motion only where it improves feedback or continuity

Rules:
- Work with the existing stack.
- Do not force a full redesign when targeted improvements will do.
- Do not break functionality for aesthetic changes.
- Preserve brand elements that are deliberate and working.
- Call out when a pattern should stay even if it is simple.
- Keep recommendations specific and reviewable.
