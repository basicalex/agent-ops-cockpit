---
name: ponytail-workflows
description: Ponytail review, audit, and debt workflows for over-engineering review and shortcut ledger scans. Use for /ponytail review, /ponytail audit, /ponytail debt, or when asked what to delete, simplify, or track as ponytail debt.
---

# Ponytail Workflows

Ported from DietrichGebert/ponytail, MIT licensed: https://github.com/DietrichGebert/ponytail

## Review

Review diffs for unnecessary complexity. One line per finding: location, what to cut, what replaces it. The diff's best outcome is getting shorter.

### Format

`L<line>: <tag> <what>. <replacement>.`

Use `<file>:L<line>: ...` for multi-file diffs.

Tags:

- `delete:` dead code, unused flexibility, speculative feature. Replacement: nothing.
- `stdlib:` hand-rolled thing the standard library ships. Name the function.
- `native:` dependency or code doing what the platform already does. Name the feature.
- `yagni:` abstraction with one implementation, config nobody sets, layer with one caller.
- `shrink:` same logic, fewer lines. Show the shorter form.

### Examples

Bad: "This EmailValidator class might be more complex than necessary, have you considered whether all these validation rules are needed?"

Good: `L12-38: stdlib: 27-line validator class. "@" in email, 1 line; real validation is the confirmation mail.`

Good: `L4: native: moment.js imported for one format call. Intl.DateTimeFormat, 0 deps.`

Good: `repo.py:L88: yagni: AbstractRepository with one implementation. Inline it until a second exists.`

Good: `L52-71: delete: retry wrapper around an idempotent local call. Nothing replaces it.`

Good: `L30-44: shrink: manual loop builds dict. dict(zip(keys, values)), 1 line.`

### Scoring

End with the only metric that matters: `net: -<N> lines possible.`

If there is nothing to cut, say `Lean already. Ship.` and stop.

### Boundaries

- Complexity only. Correctness, security, and performance belong in a normal review pass.
- Do not apply fixes; list them.
- Do not flag one small smoke test or assert-based self-check as bloat. That is the ponytail minimum for non-trivial logic.

## Audit

Run a repo-wide ponytail review. Scan the tree, rank findings biggest cut first, and apply nothing.

### Tags

- `delete:` dead code, unused flexibility, speculative feature. Replacement: nothing.
- `stdlib:` hand-rolled thing the standard library ships. Name the function.
- `native:` dependency or code doing what the platform already does. Name the feature.
- `yagni:` abstraction with one implementation, config nobody sets, layer with one caller.
- `shrink:` same logic, fewer lines. Show the shorter form.

### Hunt

Look for:

- Dependencies the stdlib or platform already covers.
- Single-implementation interfaces and adapters.
- Factories with one product.
- Wrappers that only delegate.
- Files exporting one trivial thing.
- Dead flags, unused config, and speculative options.
- Hand-rolled stdlib.

### Output

One line per finding, ranked:

`<tag> <what to cut>. <replacement>. [path]`

End with `net: -<N> lines, -<M> deps possible.`

Nothing to cut: `Lean already. Ship.`

### Boundaries

- Complexity only. Correctness, security, and performance belong in a normal review pass.
- Read and report only; do not edit files.
- Keep scanning targeted enough to support findings. Do not run broad project tests or formatters for this audit.

## Debt

Every deliberate ponytail shortcut should be marked with a `ponytail:` comment naming its ceiling and upgrade path. This command collects them into a ledger so a deferral cannot quietly become permanent.

### Scan

Search the repo for comment markers, skipping `.git`, dependencies, and build output. Use the repository's search tools, not shell grep, when operating inside OMP.

Match comment prefixes such as:

- `// ponytail:`
- `# ponytail:`
- `/* ponytail:`
- `<!-- ponytail:`

Each hit is one ledger row. The comment prefix keeps prose that merely mentions the convention out of the ledger.

### Output

One row per marker, grouped by file:

`<file>:<line> — <what was simplified>. ceiling: <named limit>. upgrade: <trigger to revisit>.`

Convention: `ponytail: <ceiling>, <upgrade path>`.

Flag any marker without a concrete upgrade path or trigger as `no-trigger`; those are the ones that rot.

End with `<N> markers, <M> with no trigger.`

Nothing found: `No ponytail: debt. Clean ledger.`

### Boundaries

- Read and report only; change nothing.
- If the user asks to persist the ledger, write the requested file then, not during the scan.
- Do not run project-wide tests or formatters.
