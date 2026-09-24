---
name: design-principles
description: Audit or apply focused interface-design refinements for task-path cost, content-aware alignment, repeated peers, output-shaped creation forms, font provenance, responsive text measure, input fit, and evidence boundaries. Use as a companion to Impeccable, not as a replacement for its frontend workflow.
version: 1.0.1
user-invocable: true
argument-hint: "[audit|apply] [target]"
---

# Design principles companion

Use this skill to make a few contextual decisions that Impeccable 4.3.1 covers only partly or through fixed heuristics. Impeccable still owns frontend design and implementation. The project brief, `DESIGN.md`, product truth, access rules, and harness permissions remain authoritative.

## Route the request

- `audit [target]`: read-only review. This is the default.
- `apply [target]`: change only a surface and finding the user asked to change.
- If the first argument is not a mode, treat it as the audit target.
- If the target is clear from the request or current context, use it. Do not ask again.
- With no arguments, audit the known current target. If no target is known, ask only for the target.

Do not turn a narrow audit into a formal Impeccable critique, build, detector run, or browser session. Never claim that a formal critique ran unless it did.

## Reuse project and Impeccable context

1. Read the project brief and `DESIGN.md` when applicable, available, and in scope. For supplied examples, use the supplied brief; do not inspect an unrelated project. Preserve factual copy, pinned brand, localization, privacy, and publication limits. Existing styling is evidence to assess, not a pinned requirement merely because it exists.
2. If Impeccable context was already loaded this session, reuse it. Do not restart setup.
3. For an audit, load one topic reference per target, a second only when needed. A supplied multi-target evaluation may use more overall; do not widen a normal audit beyond one or two tasks. Use verification guidance when needed and keep the full delta audit out of routine context.
4. For apply, invoke the owning Impeccable command through the harness when available, such as `/impeccable layout <target>` or `/impeccable harden <target>`. Never invent a bare `/layout` or `/harden` command.
5. Only for apply or an explicitly requested Impeccable workflow: if installed but not invocable, read its [entrypoint](../impeccable/SKILL.md), load the appropriate reference, and follow that workflow. If absent, state that and use this skill plus the project rules without claiming Impeccable completion. A narrow companion audit needs no Impeccable setup.

Topic references:

- [Composition](reference/composition.md): scan alignment, peer normalization, artifact-shaped creation forms.
- [Type and media](reference/type-and-media.md): font provenance, real responsive measure, variable-media stress checks.
- [Interaction cost](reference/interaction-cost.md): task cost, direct controls, input fit, protective friction.
- [Verification](reference/verification.md): evidence labels, acceptance checks, bounded QA.
- [Delta audit](reference/delta-audit.md): source reconciliation and known tension with fixed heuristics.

## Audit workflow

1. Name at most one or two user tasks. Inspect only the source, rendered state, or supplied evidence needed for them.
2. For each relevant rule, record: trigger or symptom, action, when not to use it, and verification.
3. Separate observed facts from likely impact. Use one evidence basis: `source`, `rendered`, `measured`, or `user-observed`. Label an uncaptured effect `inference`.
4. Return at most three to five findings. Fewer findings, or none, is valid.
5. State any direct tension with an Impeccable rule. Do not make both rules sound absolute.
6. Give the smallest acceptance check that can confirm each finding.

Do not score the interface unless the evidence scale is defined. Do not treat book ranges, click counts, step counts, or visible-choice counts as accessibility or usability pass/fail limits. A fifth legitimate choice is not a defect by count alone.

## Apply workflow

Apply only after the user asks to change an identified surface. Use the current audit findings when they exist; do not repeat discovery.

1. Pick the Impeccable command that owns the change and follow its setup, reference, and delegation rules.
2. Preserve valid review, consent, confirmation, re-authentication, privacy, security, and recovery steps.
3. Change the smallest surface that resolves the finding. More interactive does not mean more animated.
4. Verify once, make one correction batch if needed, then run one confirmation pass. Report unresolved items instead of polishing indefinitely.

If the user explicitly asks for formal `/impeccable critique`, keep its required report. Where its fixed four-item threshold appears, label it as a heuristic and compare it with task evidence rather than silently replacing the critique workflow.

## Safety and QA boundaries

- No hover-only essential content, scroll hijacking, forced cards, or motion without a task job.
- Never submit a real form, send an upload, activate a purchase, or trigger a destructive action during QA without explicit authorization.
- Use browser QA only when it is in scope and permitted. Browser scripts must close the browser in `try/finally`, run under a hard timeout, and clean up only owned headless processes.
- When browser QA applies, check keyboard, touch, narrow viewports, localization where relevant, and reduced motion.
- Missing rendered, measured, analytics, or user data stays unknown. Report a hypothesis, not an invented result.

## Output

For each finding, give:

- `F#` and the affected task or surface;
- evidence basis and exact evidence location;
- symptom, action, and when not to use it;
- likely impact labeled as inference unless observed;
- one acceptance check;
- the owning Impeccable command or a stated fallback.

End with the bounded next action. Do not claim conversion, retention, task success, WCAG conformance, or real-world improvement without matching evidence.
