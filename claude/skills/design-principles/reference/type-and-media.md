# Type and media refinements

Load this reference for a font change, responsive prose-measure question, or reusable text over variable media. Preserve the project's pinned type and visual world unless the user asked to replace them.

## T1. Record font provenance when assets change

Source basis: supplied textbook, physical pp. 72–76 and 96–97. The rights and provenance procedure is operational synthesis.

| Part | Guidance |
|---|---|
| Trigger or symptom | A task introduces or replaces a font file, hosted font, weight, subset, or variable-font build. |
| Action | Record the asset source, file identity, available license text, intended delivery, and any stated web or commercial-use terms. Confirm the shipped files match the reviewed source. If terms are unclear, stop and ask the owner or qualified reviewer. |
| Do not use | Do not re-audit a pinned existing font on every typography task. Do not claim legal certainty from a filename, download page, or missing notice. Do not choose serif, script, or sans by industry stereotype. |
| Verify | Compare source and shipped filenames or hashes, inspect included notices and project records, confirm only used files load, and record unresolved rights questions without guessing. |

This check documents evidence. It does not provide legal advice or grant rights.

## T2. Measure real narrow-viewport text

Source basis: supplied textbook, physical pp. 76–80. Its 45–75 desktop and 30–40 mobile character ranges are optional heuristics, not standards.

| Part | Guidance |
|---|---|
| Trigger or symptom | Prose wraps poorly at a supported narrow width, or a review relies only on a CSS `ch` value. |
| Action | Measure representative real copy with the shipped face, weight, size, locale, container, and viewport. Record actual line samples or characters per rendered line and inspect line height, word breaks, and fallback behavior. |
| Do not use | Do not treat CSS `ch` as an actual character count. Do not force copy into a target range, shrink type to hit a number, or make a heuristic a pass/fail rule. |
| Verify | Test shortest and longest supported locales, narrow and intermediate widths, zoom or text scaling, font load and fallback, and any dense or reading-specific container. A good result may sit outside the book ranges. |

State whether a number is a rendered measurement or a CSS proxy. Never blur the two.

## T3. Stress reusable text-on-media components

Source basis: supplied textbook, physical pp. 21–22. Testing varied crops and luminance is an operational example of Impeccable's existing contrast requirement, not a newly found accessibility rule.

| Part | Guidance |
|---|---|
| Trigger or symptom | One component places text or controls over changing editorial, user-supplied, or responsive media. |
| Action | Test representative light, dark, noisy, cropped, and focal-point-shifted media. Prefer structural text separation or a deterministic treatment when source variance defeats local contrast. |
| Do not use | Do not approve from one favorable image, use blur or shadow as the automatic fix, or state WCAG conformance without computed pairs for relevant states. |
| Verify | Measure computed foreground/background contrast for each tested state and inspect hierarchy across responsive crops, loading, errors, themes, and reduced-data fallbacks where applicable. Record untested media as unknown. |

## Existing prerequisites

Impeccable already covers type hierarchy, family count and fit, delivery, plain-language hierarchy, paragraph rhythm, content chunking, color semantics, and variable text-on-media contrast. Use those rules as prerequisites. Do not present them as additions from this companion.
