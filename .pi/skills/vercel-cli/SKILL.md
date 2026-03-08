---
name: vercel-cli
description: Vercel CLI operations for deployments, environment variables, projects, domains, logs, aliases, teams, builds, and production-safe workflows. Use when the user asks to deploy, inspect Vercel state, manage preview/prod, pull env vars, inspect logs, manage domains, link projects, or otherwise operate a Vercel-hosted app.
allowed-tools: Bash(vercel:*), Bash(npx vercel:*), Bash(env), Bash(printenv), Bash(pnpm:*), Bash(npm:*)
---

# Vercel CLI Operations

Use this skill when a repo or developer needs Vercel-aware operational help. Prefer explicit, auditable CLI steps. Treat production changes as high-risk.

## Core Principles

1. **Prefer inspect before mutate**
   - Start with read-only commands when possible.
   - Confirm project, team, target environment, and current branch before deploying.

2. **Prefer preview before production**
   - Use preview deployments for validation unless the user explicitly asks for prod.
   - Call out when an action impacts production.

3. **Respect auth and scope**
   - Vercel auth can come from interactive login or `VERCEL_TOKEN`.
   - Team/project scope can affect results; check link state and scope first.

4. **Keep secrets out of chat output**
   - Do not echo secret env values unless the user explicitly requests that.
   - Prefer `vercel env pull` into local files over printing values inline.

5. **Be explicit about cwd**
   - Most Vercel commands are repo-sensitive. Run them from the intended project directory.

## Quick Readiness Checklist

Before doing real work, verify:

```bash
vercel --version
vercel whoami
vercel link --yes      # only when you intentionally want to link the current repo
```

Useful local signals:

- `.vercel/project.json` present => repo linked
- `VERCEL_TOKEN` set => non-interactive auth likely available
- `vercel whoami` succeeds => CLI auth is working

## High-Value Command Families

### Authentication

```bash
vercel login
vercel whoami
vercel logout
```

Use `VERCEL_TOKEN` for automation/non-interactive usage:

```bash
VERCEL_TOKEN=*** vercel whoami
```

If a team/scope matters, include it explicitly where supported:

```bash
vercel teams ls
vercel switch
```

## Repo Linking and Project Selection

```bash
vercel link
vercel link --yes
vercel unlink
vercel project ls
vercel project inspect <name>
vercel project add
```

Typical flow:

1. Confirm current working directory.
2. Check for `.vercel/project.json`.
3. If not linked, use `vercel link`.
4. Verify linked project before deploy/env/log operations.

## Deployments

### Preview deploy

```bash
vercel
vercel deploy
```

### Production deploy

```bash
vercel --prod
vercel deploy --prod
```

### Deploy from prebuilt output

```bash
vercel build
vercel deploy --prebuilt
vercel deploy --prebuilt --prod
```

### Useful deploy flags

```bash
vercel deploy --yes
vercel deploy --archive=tgz
vercel deploy --meta key=value
vercel deploy --env KEY=value
vercel deploy --build-env KEY=value
```

## Build / Dev

```bash
vercel dev
vercel build
vercel pull
```

Use cases:

- `vercel dev` => emulate Vercel local behavior
- `vercel build` => validate what Vercel would build
- `vercel pull` => fetch project/environment settings for local use

## Environment Variables

### List env vars

```bash
vercel env ls
vercel env ls production
vercel env ls preview
vercel env ls development
```

### Add env vars

```bash
vercel env add
vercel env add MY_KEY production
vercel env add MY_KEY preview
vercel env add MY_KEY development
```

### Remove env vars

```bash
vercel env rm MY_KEY
vercel env rm MY_KEY production
```

### Pull env vars locally

```bash
vercel env pull
vercel env pull .env.local
vercel env pull .env.preview.local --environment=preview
vercel env pull .env.production.local --environment=production
```

### Good practice

- Prefer `vercel env pull` over manually copying secret values.
- Call out target environment clearly.
- Avoid printing pulled secrets back into chat.

## Inspect Deployments, Logs, and Runtime State

```bash
vercel list
vercel ls
vercel inspect <deployment-url-or-id>
vercel logs <deployment-url-or-id>
vercel logs <deployment-url-or-id> --since 1h
```

Recommended troubleshooting flow:

1. `vercel list` or identify current deployment URL.
2. `vercel inspect <deployment>` for summary/build/runtime details.
3. `vercel logs <deployment>` for runtime behavior.
4. If env/config issue suspected, inspect project link + env configuration.

## Domains and Aliases

```bash
vercel domains ls
vercel domains inspect <domain>
vercel domains add <domain>
vercel domains rm <domain>

vercel alias ls
vercel alias set <deployment-url> <domain>
vercel alias rm <domain>
```

Be careful with aliases/custom domains because they may redirect live traffic.

## Teams / Scope / Collaboration

```bash
vercel teams ls
vercel teams invite
vercel switch
```

If results look wrong, the CLI may be operating under the wrong scope/team. Check that before diagnosing deeper issues.

## Certificates / DNS / Edge Cases

Depending on CLI version and account capabilities, Vercel may expose commands around certs, dns, integration state, or advanced project configuration. When exploring available capabilities on a given machine, check:

```bash
vercel help
vercel --help
vercel <subcommand> --help
```

## Safe Workflows

### 1. Inspect current project and auth

```bash
vercel --version
vercel whoami
test -f .vercel/project.json && echo linked || echo unlinked
```

### 2. Pull preview env locally

```bash
vercel env pull .env.preview.local --environment=preview
```

### 3. Preview deploy

```bash
vercel deploy --yes
```

### 4. Inspect preview

```bash
vercel inspect <deployment-url>
vercel logs <deployment-url>
```

### 5. Promote explicitly to prod

```bash
vercel deploy --prod --yes
```

Only do step 5 when the user clearly wants production impact.

## Production Guardrails

When the user asks for a prod action:

- confirm this is intended production scope
- identify the target project/team
- check linked repo state
- mention that domains/aliases/env/prod deploys can affect live traffic
- prefer a preview deploy first unless the user explicitly wants direct prod

## Common Diagnostic Playbooks

### "Why is prod broken?"

```bash
vercel whoami
vercel inspect <deployment>
vercel logs <deployment>
vercel env ls production
```

Check:
- wrong scope/team
- missing prod env vars
- failing runtime logs
- stale alias/domain target

### "Why does preview differ from local?"

```bash
vercel pull
vercel env pull .env.preview.local --environment=preview
vercel build
```

Check:
- missing env parity
- build-time vs runtime vars
- framework config differences

### "Can we connect this repo to Vercel?"

```bash
test -f .vercel/project.json && echo linked || echo unlinked
vercel project ls
vercel link
```

## Decision Rules for Agents

- If the user says **deploy**, default to preview unless they say prod.
- If the user says **prod**, restate that this is a production-impacting action.
- If the user says **check Vercel**, start with `vercel --version`, `vercel whoami`, link state, and project scope.
- If the user says **sync env**, prefer `vercel env pull`.
- If the user says **debug deployment**, prefer `inspect` + `logs`.
- If the user says **set domain/alias**, call out live traffic impact.

## Handy Reference

```bash
# Basics
vercel --version
vercel whoami
vercel help

# Linking / project
vercel link
vercel unlink
vercel project ls
vercel project inspect <name>

# Deploy
vercel deploy
vercel deploy --prod
vercel build
vercel deploy --prebuilt

# Env
vercel env ls
vercel env add
vercel env rm
vercel env pull .env.local

# Inspect
vercel list
vercel inspect <deployment>
vercel logs <deployment>

# Domains / alias
vercel domains ls
vercel domains inspect <domain>
vercel alias ls
vercel alias set <deployment> <domain>
```

## Notes

- Exact subcommands/options can vary slightly by Vercel CLI version.
- When unsure, consult `vercel --help` or `<subcommand> --help` on the installed machine.
- Prefer explicit commands and small reversible steps over broad automated mutations.
