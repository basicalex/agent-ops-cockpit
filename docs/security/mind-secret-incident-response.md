# Mind Secret Incident Response

## When to use this
Use this playbook if you suspect any Mind runtime, export, checkpoint, or repository-visible artifact may have captured credentials or other secrets.

## Immediate containment
1. Stop any active Mind-related runtimes that may still be ingesting or exporting unsafe content.
2. Treat exposed credentials as compromised.
3. Rotate provider tokens first, especially:
   - `ANTHROPIC_API_KEY`
   - `OPENAI_API_KEY`
   - cloud access tokens
   - session or refresh tokens
   - any `Authorization: Bearer ...` material
4. Invalidate related sessions, cookies, and derived credentials.

## Local cleanup
1. Remove local runtime state from the state directory used by Mind.
   - Default root: `${XDG_STATE_HOME:-$HOME/.local/state}/aoc/mind/`
2. Remove repo-local safe exports if they are suspected to contain unsafe material:
   - `.aoc/mind/insight/`
   - `.aoc/mind/t3/`
3. Rebuild exports only after the sanitized pipeline is confirmed healthy.

## Repository verification
Run:

```bash
scripts/verify-mind-runtime-safety.sh
```

This checks for:
- forbidden tracked Mind runtime DB/lock artifacts
- known secret markers in git-visible files

Also inspect git history and the current tree manually if exposure is suspected.

## Git remediation
If unsafe material was committed:
1. Rotate secrets before rewriting history.
2. Remove the material from the current tree.
3. Rewrite history if the secret entered commits, tags, or release branches.
4. Force-push only after coordinating with collaborators.
5. Ask downstream consumers to re-clone or hard-reset if history changed.

## Validation after cleanup
1. Run relevant cargo tests.
2. Run `scripts/verify-mind-runtime-safety.sh`.
3. Re-run the Mind export path and confirm exports are regenerated cleanly.
4. Confirm no runtime DBs or locks exist under tracked repo paths.

## Operational notes
- Live Mind runtime state now belongs under the AOC state directory, not under the repo.
- Repo-visible Mind files should be treated as validated exports only.
- Storage and export writes fail closed on known secret-bearing content, but rotation is still required after any suspected leak.
