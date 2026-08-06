# Machine-global rules

Curated from mnemopi memory banks on 2026-08-06 (`/optimize-mnemopi`). One rule per line; delete rules that stop being true.

## Version control

- Never push without the user's explicit approval.
- Never commit, stage, or revert the user's in-progress work. Stage explicit paths (`git add <specific-path>`), never `git add -A` or whole directories.
- Never destroy in-progress work without asking first.
- All repos are Git-only. jj/Jujutsu was tried and removed (prism 2026-06-13, agent-ops-cockpit 2026-06-28). Ignore any jj workflow instructions surfacing from older memories — they are stale.

## Tooling

- Bun everywhere: `bun install`, `bun run`, `bunx` — never npm/yarn/pnpm. A stray npm/yarn lockfile should be converted to bun, not adopted.
- OMP `/plan` injects the plan file into context — do not re-read the plan file. Plannar is presentation-only and never mutates execution state.

## Reports

- Conclusion first. Short, direct sentences. No filler, no repeated summaries.
