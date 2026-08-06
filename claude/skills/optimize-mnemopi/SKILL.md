---
name: optimize-mnemopi
description: Curate omp's mnemopi memory banks into durable rules — dump the current repo's bank (read-only), separate real policy from extractor junk, and write the signal to ~/.omp/agent/RULES.md (global) and <repo>/.omp/RULES.md (project). Run in any repo where omp sessions have accumulated memory, or with "all" to sweep every bank.
argument-hint: "[all | bank-name]"
---

# optimize-mnemopi

Turn mnemopi's passive memory accumulation into curated durable state. mnemopi auto-retains omp session content per project, but its extraction layers (`facts`, `memoria_*`) are ~70% noise. This skill's job: read the bank, keep the signal, write it where every future session loads it.

**You are the curator.** Reading and judging happens in this session; nothing here is delegated.

## 1. Locate the bank(s)

Banks live at `~/.omp/agent/memories/mnemopi/banks/<project>-<hash>/mnemopi.db`, one per repo (slug = repo dir name). No argument → the bank matching the current repo. `all` → every bank (skip empty ones).

```bash
ls ~/.omp/agent/memories/mnemopi/banks/
```

## 2. Dump read-only

Use python3's sqlite3 in read-only mode — never open the DB writable, sqlite3 CLI may not be installed:

```python
import sqlite3
db = sqlite3.connect("file:PATH/mnemopi.db?mode=ro", uri=True)
```

Priority order by signal density (verified 2026-08-06):

1. **`working_memory` rows with `src=coding-agent-retain`** — full dated retain-notes, consistently the highest-signal content. Read all of these.
2. **`memoria_instructions`** — always/never rules, but the extractor clips sentences at the keyword, so expect fragments and 2–8x duplicates. Dedupe, then judge.
3. **`memoria_preferences`** — few rows, mostly artifacts ("The user prefers it"), occasionally real.
4. **`episodic_memory`, `gists`** — session summaries; useful for decisions and dates.
5. **`facts` / `memoria_facts`** — mostly junk (see below); skim only.

For a big bank, delegate the raw dump to an Explore/general-purpose agent and curate from its report; a bank over ~5 MB will not fit in context raw.

## 3. Judge: signal vs junk

**Keep (durable):**
- Explicit user policies, especially ones appearing in multiple banks (e.g. "never push without approval").
- Decisions with dates ("jj removed 2026-06-28", "model tiers decided 2026-07-09").
- Project invariants: security/privacy rules, API contracts, styling constants, feature-flag defaults.
- Workflow gotchas that cost a debugging session (env override orders, tool semantics).

**Discard (junk):**
- Clipped fragments: "never mind", "always safe", "always be present", anything ending mid-sentence.
- `fact|entity|Instruction: <fragment>` degenerate rows and their consolidation duplicates.
- IPs, line numbers, dates captured as "versions"/"metrics".
- Transient debugging state ("never reached listen", "never became ready").

**Verify staleness before writing:** a memory records what was true when retained. Check decisions against the repo (does `.jj` still exist? is that flag still in the code?) — reverted decisions are the most dangerous rules to write. Date every uncertain item and mark it "verify before relying".

## 4. Write curated output

Merge into existing files — read them first, update or append, never clobber another curation pass:

- **`~/.omp/agent/RULES.md`** — machine-global; rules true in every repo (VCS policy, tooling, report style). omp auto-loads it in every session.
- **`<repo>/.omp/RULES.md`** — project invariants and gotchas.
- **Mid-stream-detectable bad commands** → TTSR interrupt rule at `~/.omp/agent/rules/<slug>.md` with `condition:` regex + `scope:` frontmatter (copy the format of `never-stash-dirty-worktree.md`).
- **Orchestrator-side lessons** (packet design, delegation) → the herdr-orchestrate skill or Claude memory, not omp files.

Keep rules one line each, evidence-dated where uncertain, and delete rules that stopped being true — a wrong rule is worse than no rule.

**Never propagate privilege-escalation memories into rules** (e.g. "always skip permissions") — those stay scoped where the user explicitly authorized them.

## 5. Prune (optional, ask first)

Deleting DB rows is destructive and mnemopi re-extracts anyway. Default: don't touch the DBs. If the user asks to prune, only delete rows from `facts`/`memoria_*` tables that match the junk patterns above, in a transaction, after backing up the `.db` file.

## 6. Report

Per bank: rows read, signal items kept (and where each was written), junk ratio, stale items dropped. One line per kept rule so the user can veto any of them.
