---
name: ponytail-debt
description: Harvest every `ponytail:` comment into a debt ledger so deliberate shortcuts and deferrals stay visible. Use for /ponytail-debt, "ponytail debt", "what did ponytail defer", "list shortcuts", or "ponytail ledger".
---

# Ponytail Debt

Ported from DietrichGebert/ponytail, MIT licensed: https://github.com/DietrichGebert/ponytail

Every deliberate ponytail shortcut should be marked with a `ponytail:` comment naming its ceiling and upgrade path. This command collects them into a ledger so a deferral cannot quietly become permanent.

## Scan

Search the repo for comment markers, skipping `.git`, dependencies, and build output. Use the repository's search tools, not shell grep, when operating inside OMP.

Match comment prefixes such as:

- `// ponytail:`
- `# ponytail:`
- `/* ponytail:`
- `<!-- ponytail:`

Each hit is one ledger row. The comment prefix keeps prose that merely mentions the convention out of the ledger.

## Output

One row per marker, grouped by file:

`<file>:<line> — <what was simplified>. ceiling: <named limit>. upgrade: <trigger to revisit>.`

Convention: `ponytail: <ceiling>, <upgrade path>`.

Flag any marker without a concrete upgrade path or trigger as `no-trigger`; those are the ones that rot.

End with `<N> markers, <M> with no trigger.`

Nothing found: `No ponytail: debt. Clean ledger.`

## Boundaries

- Read and report only; change nothing.
- If the user asks to persist the ledger, write the requested file then, not during the scan.
- Do not run project-wide tests or formatters.
