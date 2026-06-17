---
name: ponytail-review
description: Review a diff only for over-engineering: what to delete, replace with stdlib/native features, or shrink. Use for /ponytail-review, "review for over-engineering", "what can we delete", or "is this over-engineered".
---

# Ponytail Review

Ported from DietrichGebert/ponytail, MIT licensed: https://github.com/DietrichGebert/ponytail

Review diffs for unnecessary complexity. One line per finding: location, what to cut, what replaces it. The diff's best outcome is getting shorter.

## Format

`L<line>: <tag> <what>. <replacement>.`

Use `<file>:L<line>: ...` for multi-file diffs.

Tags:

- `delete:` dead code, unused flexibility, speculative feature. Replacement: nothing.
- `stdlib:` hand-rolled thing the standard library ships. Name the function.
- `native:` dependency or code doing what the platform already does. Name the feature.
- `yagni:` abstraction with one implementation, config nobody sets, layer with one caller.
- `shrink:` same logic, fewer lines. Show the shorter form.

## Examples

Bad: "This EmailValidator class might be more complex than necessary, have you considered whether all these validation rules are needed?"

Good: `L12-38: stdlib: 27-line validator class. "@" in email, 1 line; real validation is the confirmation mail.`

Good: `L4: native: moment.js imported for one format call. Intl.DateTimeFormat, 0 deps.`

Good: `repo.py:L88: yagni: AbstractRepository with one implementation. Inline it until a second exists.`

Good: `L52-71: delete: retry wrapper around an idempotent local call. Nothing replaces it.`

Good: `L30-44: shrink: manual loop builds dict. dict(zip(keys, values)), 1 line.`

## Scoring

End with the only metric that matters: `net: -<N> lines possible.`

If there is nothing to cut, say `Lean already. Ship.` and stop.

## Boundaries

- Complexity only. Correctness, security, and performance belong in a normal review pass.
- Do not apply fixes; list them.
- Do not flag one small smoke test or assert-based self-check as bloat. That is the ponytail minimum for non-trivial logic.
