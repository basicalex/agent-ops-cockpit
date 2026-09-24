# Verification contract

Use this reference to keep findings factual and checks bounded. It applies to both audit and apply modes.

## Separate evidence from impact

A finding has two distinct fields.

**Evidence basis**

- `source`: code, content, configuration, or design contract states the fact.
- `rendered`: a permitted visual or interactive inspection shows the fact.
- `measured`: a named method returns a value, such as contrast, line measure, target size, timing, or request count.
- `user-observed`: supplied research or a recorded user session shows behavior.

**Impact status**

- `observed`: the matching user or system outcome was captured.
- `inference`: the effect is plausible but not observed.
- `unknown`: required data is unavailable.

A source fact and its likely effect belong in separate sentences. Source evidence can prove that a route drops state; it cannot by itself prove abandonment or lost conversion.

## Finding template

```text
F#: [task or surface]
Evidence: [basis] [path:line, state, method, or supplied observation]
Symptom: [fact only]
Action: [smallest contextual change]
Do not use when: [exception or protecting condition]
Impact: [observed | inference | unknown] [statement]
Verify: [one acceptance check]
Owner: /impeccable [command] [target] | stated fallback
```

Return at most three to five findings. Do not add a score unless its scale, inputs, and interpretation are defined.

## Minimal acceptance form

Write acceptance checks as observable fixtures:

```text
Given [real role, locale, content, state, input method, viewport]
When [one action or state change]
Then [observable behavior or measured value]
And [protected behavior remains unchanged]
Evidence method: [source | rendered | measured | user-observed]
```

Use the fixtures that match the finding, not every fixture below.

| Topic | Minimal fixtures |
|---|---|
| Alignment | LTR and relevant RTL locale; long prose; comparable positive, negative, decimal, unit-bearing, and mixed-script values. |
| Repeated peers | shortest, longest, missing, localized, and status-heavy content; intrinsic and intentionally cropped media. |
| Creation form | initial, partial, invalid, corrected, preview, saved or published state; narrow keyboard order. |
| Font change | source record, included notice, shipped file identity, used weights, fallback and loading state. |
| Text measure | real face and fallback, shortest and longest locale, narrow and intermediate width, zoom or text scaling. |
| Variable media | light, dark, noisy, focal crop, loading, error, theme, and text/control states. |
| Task cost | baseline and proposed path; remembered facts, context switches, repeated entry, waits, errors, protective steps. |
| Direct controls | frequent and uncommon use, stable and changed set, keyboard, touch, narrow width, localization. |
| Input fit | mobile keyboard, paste, autofill, password manager, backspace, correction, server rejection, retry, focus. |
| Protective friction | destructive and harmless action, cancel or undo, double activation, focus return, consent record where required. |

## Bounded QA

For an audit, run only in-scope read-only checks and report findings. Do not implement corrections or exercise real writes to verify a hypothesis.

For an authorized apply task:
1. Run the smallest initial check that can disprove the finding or change.
2. If it fails, make one correction batch within the authorized scope.
3. Run one confirmation pass and stop. Report anything unresolved.

Do not auto-run a full build, detector, formal critique, or browser pass for a narrow source audit. Use targeted diagnostics or checks first.

Browser QA needs scope and permission. Any script must close its browser in `try/finally` and run under a hard timeout. Test reduced motion, keyboard, touch, and narrow layout when relevant. Do not submit real forms, upload files, buy anything, or trigger destructive actions without explicit authorization. Clean up only browser processes started by the check.

## Claims boundary

- Cite WCAG requirements as guidance when relevant. Claim a failure or conformance only for a verified applicable criterion; name its version, criterion, method, and threshold where relevant. Non-numeric criteria may require source inspection or manual testing, not a fabricated measurement. A passing criterion does not establish whole-page conformance.
- Book ranges are diagnostics, not WCAG rules.
- Do not claim conversion, retention, satisfaction, task success, legal clearance, or real-world improvement without matching evidence.
- If rendered, analytics, or user data is unavailable, label the result `unknown` or `inference`.
