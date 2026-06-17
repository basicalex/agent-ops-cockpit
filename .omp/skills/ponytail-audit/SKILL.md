---
name: ponytail-audit
description: Whole-repo over-engineering audit: ranked list of what to delete, simplify, or replace with stdlib/native equivalents. Use for /ponytail-audit, "audit for over-engineering", "what can I delete", or "find bloat".
---

# Ponytail Audit

Ported from DietrichGebert/ponytail, MIT licensed: https://github.com/DietrichGebert/ponytail

Run a repo-wide ponytail review. Scan the tree, rank findings biggest cut first, and apply nothing.

## Tags

- `delete:` dead code, unused flexibility, speculative feature. Replacement: nothing.
- `stdlib:` hand-rolled thing the standard library ships. Name the function.
- `native:` dependency or code doing what the platform already does. Name the feature.
- `yagni:` abstraction with one implementation, config nobody sets, layer with one caller.
- `shrink:` same logic, fewer lines. Show the shorter form.

## Hunt

Look for:

- Dependencies the stdlib or platform already covers.
- Single-implementation interfaces and adapters.
- Factories with one product.
- Wrappers that only delegate.
- Files exporting one trivial thing.
- Dead flags, unused config, and speculative options.
- Hand-rolled stdlib.

## Output

One line per finding, ranked:

`<tag> <what to cut>. <replacement>. [path]`

End with `net: -<N> lines, -<M> deps possible.`

Nothing to cut: `Lean already. Ship.`

## Boundaries

- Complexity only. Correctness, security, and performance belong in a normal review pass.
- Read and report only; do not edit files.
- Keep scanning targeted enough to support findings. Do not run broad project tests or formatters for this audit.
