---
name: browser-qa
description: Authenticated browser QA on the Voyager dev server — mint real AuthKit sessions for seeded test personas via /api/dev/login, then drive the UI with Playwright. Use whenever QA, visual review, or E2E verification needs a logged-in user.
---

# browser-qa

Run authenticated browser QA against the local dev server. Auth is self-serve: `/api/dev/login` mints a **genuine AuthKit session** (real access token — middleware, actor resolution, and Convex auth behave exactly as in production) for seeded WorkOS test personas. Never ask the user for cookies, and never use the legacy `/dev/qa-login` route (it fabricates sessions without access tokens, so Convex queries run unauthenticated).

## Prerequisites

1. **Dev server running** on `http://localhost:3000` (Next.js app in `apps/web`). Check before starting one — the user often has a dev server running already; never kill an existing one. If none is up: `bun run dev` from `apps/web`.
2. **Env gates** (all live in repo-root `.env.local`; `apps/web/.env.local` is a symlink to it):
   - `WORKOS_API_KEY` starting with `sk_test_`
   - `VOYAGER_ENABLE_DEV_LOGIN=1`
   - `VOYAGER_DEV_LOGIN_SECRET` and `VOYAGER_DEV_LOGIN_PASSWORD` set

Read the secret from the env file when building the login URL; never hardcode or print it:

```bash
grep '^VOYAGER_DEV_LOGIN_SECRET=' .env.local | cut -d= -f2
```

## Logging in

One GET per persona, in a fresh browser context:

```
http://localhost:3000/api/dev/login?as=<persona>&secret=<VOYAGER_DEV_LOGIN_SECRET>&returnTo=<path>
```

`returnTo` is optional (must be a same-origin path starting with `/`; defaults to `/dashboard`). The response sets the session cookie and redirects.

## Personas

All are seeded, password-auth WorkOS test users (`dev-<persona>@voyager.test`) with matching Convex user rows:

| `as=` | Role | Owns / member of | Default landing |
|---|---|---|---|
| `superuser` | globalRole superuser + platform grant | — | `/superuser` |
| `scout` | plain user | — | `/scout/atlas` |
| `agency-admin` | agency owner | **Dev QA Agency** (`/agency/dev-qa-agency`, verified); also admin of Dev QA Business | `/dashboard/select-entity` |
| `business-admin` | BUSINESS_ADMIN | **Dev QA Business** (`/business/dev-qa-business`, verified) | business dashboard |
| `community-admin` | community owner | **Dev QA Community** (`dev-qa-community`, approved) | community dashboard |

**Fixtures are durable — do NOT delete or "clean up"** the `dev-*@voyager.test` users or the Dev QA Agency/Business/Community entities. They were created through the real onboarding + verification flows and are shared QA infrastructure. If a persona's permissions look wrong, re-run entity verification as `superuser` to re-sync FGA grants rather than recreating anything.

## QA workflow

- Use one fresh browser context (or profile) per persona; switch personas by hitting the login URL again in a new context, not by mixing cookies.
- Prefer accessibility-tree snapshots for state assertions (cheap); take screenshots only for visual judgment.
- Watch the browser console and network log for errors while exercising flows — a rendered page with failing Convex queries is a failure, not a pass.
- Test the golden path *and* the unhappy paths relevant to the change under review.

## Failure modes

| Symptom | Cause |
|---|---|
| `404 Not found` from the login route | An env gate is unsatisfied (non-`sk_test_` key, `VOYAGER_ENABLE_DEV_LOGIN` unset, or missing secret/password vars) |
| `403 Invalid secret` | `secret` param doesn't match `VOYAGER_DEV_LOGIN_SECRET` |
| `400 Unknown persona` | `as` value not in the persona table |
| `500 Authentication failed` | WorkOS password auth failed — check `VOYAGER_DEV_LOGIN_PASSWORD` and that the test users still exist |
| Env edits not picked up | Next.js doesn't watch the symlinked env file — `touch apps/web/.env.local` to trigger a reload |
| Redirect bounces to hosted AuthKit sign-in | You navigated to a non-`localhost` origin (e.g. `0.0.0.0:3000`) — cookies are origin-scoped; always use `http://localhost:3000` |

The route implementation is `apps/web/src/app/api/dev/login/route.ts`; it is structurally production-safe (requires an `sk_test_` key plus explicit opt-in).
