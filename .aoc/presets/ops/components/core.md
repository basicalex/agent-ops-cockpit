# Ops preset core

Use this preset for health checks, deploys, runtime diagnostics, repo mapping, task hygiene, and production-safe changes.

Rules:
- Use smallest viable command first.
- Avoid broad scans unless targeted inspection fails.
- Do not perform destructive actions without explicit operator intent.
- Prefer structured summaries over raw logs.
- Keep plans and implementation aligned when work crosses planning boundaries.
