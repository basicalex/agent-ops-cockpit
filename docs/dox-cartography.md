# AOC DOX cartography

AOC DOX maps sparse, evidence-backed `AGENTS.md` local contracts. It is for operational context that changes agent behavior in a subtree, not for architecture summaries, onboarding docs, or recursive documentation generation.

## Command sequence

```bash
aoc dox map
aoc dox review
aoc dox review --packet --write-packet
aoc dox apply --dry-run
aoc dox apply --yes
aoc dox doctor
aoc dox eval
```

- `aoc dox map` writes only `.aoc/dox/` metadata. It never creates or edits `AGENTS.md`.
- `aoc dox review` groups create/update/reject candidates and reports budget state.
- `aoc dox review --packet` renders full proposed local contracts for editor review.
- `aoc dox review --packet --write-packet` writes `.aoc/dox/review.md`.
- `aoc dox apply --dry-run` renders target paths and byte counts without writing files. `aoc dox apply --dry-run --json --include-content` adds full rendered content for machine review; default dry-run JSON remains paths/bytes only.
- `aoc dox apply --yes` is the only DOX command that may create or update local `AGENTS.md` files.
- `aoc dox doctor` validates metadata, evidence paths, safe verification commands, and instruction-chain budgets.
- `aoc dox eval` writes an eval matrix scaffold only; v1 does not run task evals.

## OMP slash command

Use `/dox` inside OMP:

```text
/dox full [path]
/dox scout [path]
/dox map
/dox review
/dox packet
/dox doctor
/dox dry-run
```

- `/dox full` runs the safe agent workflow: `aoc_dox map` -> `dox-scout` -> `dox-mapper` -> `dox-critic` -> `dox-writer` -> `review-packet` -> `apply-dry-run` -> `doctor`.
- `/dox scout` runs map plus scout-only candidate discovery.
- `/dox packet` renders and writes `.aoc/dox/review.md` for editor review.
- `/dox dry-run` runs `aoc_dox` action `apply-dry-run` and points to `.aoc/dox/review.md` when present.
- No `/dox` mode may run `aoc dox apply --yes`; that remains a human/operator CLI action.

## OMP tool sequence

1. Run `aoc_dox` with action `map`.
2. Launch `dox-scout` in parallel for high-risk or insufficient-coverage paths from `.aoc/dox/map.json`.
3. Launch `dox-mapper` only for scout-approved candidate areas.
4. Launch `dox-critic` on every create/update proposal.
5. Use `dox-writer` only after critic approval; writer may edit `.aoc/dox/candidates.json` and `.aoc/dox/report.md`.
6. Run `aoc_dox` action `review-packet` with `writePacket=true` to write `.aoc/dox/review.md`.
7. Run `aoc_dox` action `apply-dry-run`.
8. Finish with `aoc_dox` action `doctor` or `aoc dox doctor`.
9. A human/operator may later run `aoc dox apply --yes` after reviewing the packet and dry-run.

The `aoc_dox` extension is safe by construction: it exposes `map`, `review`, `review-packet`, `doctor`, `eval`, and `apply-dry-run`; it never exposes `apply --yes`.

## CodeGraph-first behavior

When `.codegraph/codegraph.db` exists and `--no-codegraph` is not set, `aoc dox map` may read:

```text
codegraph status . --json
codegraph files --path . --max-depth 3 --json
```

DOX must not run mutating CodeGraph commands such as `init`, `index`, `sync`, `install`, `unlock`, or `uninit`. If CodeGraph is missing, stale, or errors, DOX records reduced confidence and continues with deterministic filesystem/config scanning.

## Artifacts

`aoc dox map` writes:

- `.aoc/dox/map.json`: deterministic repo facts, directory coverage, AGENTS resolution chains, and CodeGraph status.
- `.aoc/dox/candidates.json`: scored local-contract candidates and decisions.
- `.aoc/dox/routes.json`: path glob to agent profile/context/verification routing metadata.
- `.aoc/dox/budgets.json`: byte budgets and measured root/project-chain sizes.
- `.aoc/dox/report.md`: concise review report generated from the JSON metadata.

`aoc dox review --packet --write-packet` writes:

- `.aoc/dox/review.md`: full editor review packet with proposed routes, rejected routes, rendered local contracts, evidence, verification, and the manual apply command.

`aoc dox eval` writes `.aoc/dox/eval-matrix.json`.

## AGENTS resolution-chain coverage

DOX maps the actual root-to-directory instruction chain for every scanned directory. At each directory, `AGENTS.override.md` wins over `AGENTS.md`; empty files are skipped.

Coverage levels:

- `root_only`: only the root instruction file applies.
- `inherited`: root plus one or more parent instruction files apply, but none at the exact directory.
- `specific`: an instruction file exists in the exact directory.
- `insufficient`: deterministic risk score meets threshold and no exact-directory instruction file exists.

Mapping the chain does not mean DOX will create every file in the chain. Only approved create/update candidates get local `AGENTS.md` files.

## Budgets

- Root target: 8192 bytes.
- Root hard limit: 12288 bytes.
- Child target: 2048 bytes.
- Child hard limit: 4096 bytes.
- Active chain default target: 16384 bytes.
- Active chain default hard limit: 24576 bytes.

Over-target is a warning. Over-hard fails `doctor`.

## Scoring formula

Positive signals:

- `+3` existing local rule differs from parent AGENTS.
- `+3` high-risk invariant: auth, secrets, money, data loss, generated files, migrations, task state, memory/STM, or deployment.
- `+2` non-obvious build/test command local to subtree.
- `+2` public API, plugin API, CLI surface, schema, or wire format.
- `+2` dynamic registry, reflection, runtime loading, or generated dispatch.
- `+1` frequent edits evidence is reserved for a future version.
- `+1` known regression history only with evidence in docs/tests/comments.

Negative signals:

- `-3` rule is obvious from file names/package layout.
- `-3` duplicates parent/root AGENTS instruction.
- `-2` no verification command exists.
- `-2` likely temporary implementation detail.
- `-2` proposed child file would exceed child hard budget.

A create/update candidate must have evidence and verification. In v1, deterministic create/update requires an existing local instruction file whose durable rules can be used as evidence; otherwise high-scoring areas are rejected for OMP mapper extraction.

## Good local AGENTS content

```md
# Repository Guidelines

Scope: `crates/aoc-cli/src`

## Local Contracts
- Keep `clap` subcommand names and flags backward compatible unless the migration updates wrapper scripts and docs.

## Verification
- `cargo test -p aoc-cli dox`

## Do Not
- Do not run mutating CodeGraph commands from DOX.

## Update When
- CLI flags, metadata schemas, or apply safety rules change.
```

Good content is short, scoped, operational, and directly verifiable.

## Bad local AGENTS content

```md
# AOC CLI architecture

The CLI is written in Rust and has several modules. It talks to storage, docs, agents, and runtime services. Developers should understand the overall design before making changes.
```

Bad content summarizes architecture, duplicates obvious context, and does not tell agents what to do differently in the subtree.
