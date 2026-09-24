# Composition refinements

Load this reference when the target involves alignment, repeated peers, or a form that creates a user-visible artifact. These rules extend the existing Impeccable layout method; they do not replace hierarchy, proximity, spacing, or responsive-order checks.

## C1. Align by scan behavior

Source basis: supplied textbook, physical pp. 17–20 and 94. The numeric rule below is synthesis, not a direct book rule.

| Part | Guidance |
|---|---|
| Trigger or symptom | Flowing prose, short focal copy, and comparable values share one alignment rule, or a numeric table is hard to compare. |
| Action | Put prose at logical start. Center only short focal copy whose role benefits from symmetry. Put comparable numbers at logical end or a decimal anchor; use tabular figures when stable digit widths help. |
| Do not use | Do not center long prose, force every heading to match body alignment, or mirror numeric columns blindly in RTL. Do not assume Latin digits, separators, signs, units, or bidi order. |
| Verify | Test the real locale and writing direction. Check decimal separators, signs, units, mixed scripts, bidi ordering, zoom, wrapping, and horizontal scroll before choosing a numeric anchor. |

Prefer CSS logical properties for text flow. Treat `ch` and character counts as measurement aids, not alignment proof.

## C2. Normalize semantic slots, not facts

Source basis: supplied textbook, physical pp. 36–49, refined against its equal-height and equal-copy advice on physical pp. 42–44.

| Part | Guidance |
|---|---|
| Trigger or symptom | Repeated peers are hard to scan because media, titles, metadata, status, or actions move without a content reason. |
| Action | Keep the same semantic role order and align comparison anchors or action baselines when comparison matters. Use a coherent media treatment and icon grammar. |
| Do not use | Do not rewrite factual copy to equal lengths, crop intrinsic media without a product reason, force equal heights when natural flow reads better, or suppress a valid missing state. |
| Verify | Stress the set with shortest, longest, missing, localized, and status-heavy content. Confirm each item stays truthful and the intended comparison can be made without searching. |

Equal geometry is useful only when it supports a real comparison. Stable role placement matters more than identical boxes.

## C3. Shape creation forms like their output

Source basis: supplied textbook, physical pp. 34–35. Impeccable already covers product context, states, onboarding, and first success; this is a narrower operational refinement.

| Part | Guidance |
|---|---|
| Trigger or symptom | A form creates a user-visible object, but field order does not match the object's structure or users cannot predict the result. |
| Action | Group and order inputs by the artifact's final structure. Add an honest preview only when it reduces uncertainty about the produced object. |
| Do not use | Do not apply this pattern to ordinary enquiry, login, account, checkout, or settings forms when the submission is not the user's artifact. Do not let a preview hide labels, validation, privacy terms, or review. |
| Verify | Complete the flow with representative, long, empty, invalid, and localized values. Confirm input-to-output mapping, preview fidelity, correction, keyboard order, and narrow reflow. |

A preview is not proof that the saved or published result matches it. Verify the final state separately when that state is in scope.

## Composition boundary

Broad hierarchy, proximity, whitespace, simplicity, content-led order, and contrast are already covered by Impeccable. Do not report them as textbook discoveries. Reject blanket advice to add cards, shadows, blur, equal copy lengths, or fixed-height rows for visual neatness.
