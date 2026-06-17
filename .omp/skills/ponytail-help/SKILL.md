---
name: ponytail-help
description: Quick-reference card for OMP ponytail modes, skills, and slash commands. Use for /ponytail-help, "ponytail help", "what ponytail commands", or "how do I use ponytail".
---

# Ponytail Help

Ported from DietrichGebert/ponytail, MIT licensed: https://github.com/DietrichGebert/ponytail

Display this reference card when invoked. One-shot: do not change mode, write flag files, or persist anything.

## Levels

| Level | Trigger | Behavior |
| --- | --- | --- |
| lite | `/ponytail lite` | Build what's asked, name the lazier alternative in one line. |
| full | `/ponytail` or `/ponytail full` | Enforce the ladder: YAGNI → stdlib → native → installed dependency → one line → minimum. Default. |
| ultra | `/ponytail ultra` | YAGNI extremist. Deletion before addition. Challenge requirements before building. |
| off | `/ponytail off` | Stop applying ponytail guidance. |
| status | `/ponytail status` | Report the intended mode in the current conversation context. |
| default | `/ponytail default` | Return to full-mode guidance unless the user says otherwise. |

OMP support is instruction-only: slash commands can send prompts, but this port does not mutate project files or rely on a persistent system-prompt lifecycle hook.

## Skills and commands

| Skill | Slash command | What it does |
| --- | --- | --- |
| ponytail | `/ponytail [mode]` | Lazy mode itself: simplest solution that works. |
| ponytail-review | `/ponytail-review` | Diff review for over-engineering: `L42: yagni: factory, one product. Inline.` |
| ponytail-audit | `/ponytail-audit` | Repo-wide over-engineering audit. Ranked delete/simplify list, no edits. |
| ponytail-debt | `/ponytail-debt` | Ledger of `ponytail:` shortcut comments and missing revisit triggers. |
| ponytail-help | `/ponytail-help` | This card. |

## Deactivate

Say `stop ponytail`, `normal mode`, or run `/ponytail off`. Resume with `/ponytail`.

## More

Full upstream docs and examples: https://github.com/DietrichGebert/ponytail
