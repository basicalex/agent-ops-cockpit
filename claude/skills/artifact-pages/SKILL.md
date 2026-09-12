---
name: artifact-pages
description: Deliver any artifact, page, report, brief, board, dashboard, or one-off HTML deliverable as a private page on docs.intrface.eu/<project>/<slug>/ from the shared artifact library (~/dev/artifact-library), independent of which Claude account or harness (claude, claude-codex, omp) produced it. Use whenever the user asks for an artifact or a page to look at or share with a client, when republishing or updating an existing artifact, when they want a share link for a client, or when a claude.ai artifact must be captured into the library. Invoke as /artifact-pages [<project>/<slug>] [share|public|private|list|sync].
argument-hint: "[<project>/<slug>] [share [--days N] | public | private | list | sync <claude-artifact-url>]"
---

# artifact-pages

One library, one site, every harness. Artifacts are HTML page bodies in `~/dev/artifact-library/<project>/<slug>.html`, registered in `manifest.json`, built and deployed to Cloudflare Pages as `https://docs.intrface.eu/<project>/<slug>/`. The site is private: the owner signs in once with the password from `~/.config/aoc/artifact-pages.env`; a client gets a signed share link or the page is flipped to public.

The claude.ai Artifact tool is optional on top. It is account-bound (two accounts on this machine, no private cross-account sharing), so the library and the docs site are the canonical copy and the claude.ai page is a convenience mirror when the Artifact tool exists in the current harness.

CLI: `~/.claude/skills/artifact-pages/bin/artifact-pages` (bun). Run `artifact-pages` with no arguments for usage. Everything below is that CLI plus authoring rules.

## Project name

`<project>` = the basename of the git repo you are working in (`polis`, `voyager`, `patchbay`, `intrface-site`, `astyle-marine`, …). Cross-project or company-level pages go under `intrface-site`. Lowercase kebab-case only, for both project and slug.

## Author a new artifact

1. Write the page **body** to `~/dev/artifact-library/<project>/<slug>.html`: no `<!doctype>`, `<html>`, `<head>` or `<body>`. Put `<title>` first, then `<style>`, then content. The build wraps it in the standard skeleton (charset, viewport, 14px system font, off-white ground) and moves `<title>` into the head. This is the same body format the Artifact tool takes as `file_path`, so one file serves both targets.
2. Design rules are the artifact rules: load the `artifact-design` skill before writing (and `artifact-diagramming` / `dataviz` when relevant); phone-width safe with a 16px side gutter; theme tokens on `:root` with the dark-mode blocks; external scripts only from cdnjs / jsdelivr npm / tailwind play / jquery; stylesheets only from fonts.googleapis.com; everything else inline or data: URI. `<pre class="mermaid">` blocks render on the docs site too (mermaid is injected when present).
3. Register and publish:
   ```
   artifact-pages add <project>/<slug> --title "Short Name"
   artifact-pages publish -m "<project>: <slug>"
   ```
   `add` is idempotent: rerun it after every edit to bump the date. `publish` builds `site/`, deploys with wrangler, and commits the library repo.
4. Report the URL `https://docs.intrface.eu/<project>/<slug>/` to the user. If the Artifact tool is available in this harness and the user wants a claude.ai copy too, publish the same file with it and record the URL: `artifact-pages add <project>/<slug> --claude-url <url> --account <prodigyceii30|basicalex>` (the account is the signed-in email's local part).

A multi-file page (index.html plus siblings) is a directory entry: put the folder at `<project>/<slug>/` and register with `--dir`.

## Update an existing artifact

Edit the body file in the library, then `artifact-pages add <project>/<slug>` and `artifact-pages publish`. The library file is the source of truth: if a claude.ai copy is newer (someone edited it there), capture it first with **sync** below, then edit.

## Sync a claude.ai artifact into the library

Only a harness with the Artifact tool can read claude.ai artifacts. Use `action: "read"` with the URL; a small page comes back inline, a large one is saved to a file named in the result. Then either write the body directly or strip the frame wrapper from a saved raw file:

```
artifact-pages add <project>/<slug> --title "…" --from <raw-or-body.html> --claude-url <url> --account <name>
```

`--from` strips `<!doctype …><body>` … `</body></html>` automatically when present.

## Share with a client

- Time-limited link for one page (default 30 days): `artifact-pages share <project>/<slug> --days 14`. The link carries a signed token; the first visit sets a cookie scoped to that page so its subresources load. No login, no account. Rotating `DOCS_SECRET` in the env file (then `artifact-pages secrets`) revokes every share link at once.
- Permanent public page: `artifact-pages visibility <project>/<slug> public` then `publish`. Reverse with `private`.
- Never hand out the owner password. Clients get share links or public pages.

Every response that delivers a page for a client states which mode was used and, for share links, the expiry date.

## Site operations

- `artifact-pages list` / `url <p>/<s>` / `open [<p>/<s>]`.
- `artifact-pages secrets` pushes `DOCS_SECRET` and `DOCS_PASSWORD` from `~/.config/aoc/artifact-pages.env` to the Pages project; run after rotating either.
- Cloudflare auth: `CLOUDFLARE_API_TOKEN` in the env file if present, else wrangler's OAuth token from `wrangler login`, which the CLI refreshes in place when it has expired (no wrangler run needed, so any agent in any project can publish). DNS calls (`artifact-pages domain`) use `CLOUDFLARE_DNS_TOKEN` from the env file, else the keychain item `cloudflare-dns-intrface`. Pages project `intrface-docs`, custom domain `docs.intrface.eu`, both in `manifest.json` under `site`.
- Gate logic lives in `templates/functions/_middleware.ts` inside this skill and is copied into `site/functions/` on every build; change it here, never in the library.
- `INDEX.md` in the library is generated on build; the manifest is the record.

## Boundaries

- Do not commit the library from anywhere but `artifact-pages publish` (or `--no-commit` and commit by hand).
- Do not put secrets, client personal data, or credentials into page bodies; the site is private by default but a public flip is one command away.
- Pages that depend on the claude.ai runtime (`window.claude.*`, comments, live republish) keep working on claude.ai only; on the docs site they are static. Say so when it matters.
