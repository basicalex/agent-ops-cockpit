// Review comments for the artifact library on Cloudflare Pages.
//
// Served at /_comments (the middleware lets this path through; access is
// enforced here). Storage is D1 `intrface-docs-comments`, bound as COMMENTS.
//
//   GET    /_comments?page=<project>/<slug>   threads + viewer identity
//   GET    /_comments?id=<id>                 one thread (used by the CLI)
//   GET    /_comments?summary=1               per-page state counts
//   POST   /_comments                         {page, body, anchor?, parent?, author?}
//   PATCH  /_comments                         {id, state}
//   DELETE /_comments?id=<id>                 owner only, soft delete
//
// Identity: owner cookie or Authorization: Bearer <ts>.<hmac> (same value as
// the owner cookie, minted by the CLI from DOCS_SECRET). A bearer request with
// X-Docs-Role: agent posts as an agent named by X-Docs-Author. A share cookie
// makes the viewer a client on the pages that token covers. Everyone else may
// read public pages only.

// Minimal ambient types: Cloudflare's generated types are not available here.
interface D1Result<T = Record<string, unknown>> {
  results: T[];
  success: boolean;
  meta: Record<string, unknown>;
}
interface D1PreparedStatement {
  bind(...values: unknown[]): D1PreparedStatement;
  first<T = Record<string, unknown>>(colName?: string): Promise<T | null>;
  run<T = Record<string, unknown>>(): Promise<D1Result<T>>;
  all<T = Record<string, unknown>>(): Promise<D1Result<T>>;
}
interface D1Database {
  prepare(query: string): D1PreparedStatement;
  batch<T = Record<string, unknown>>(statements: D1PreparedStatement[]): Promise<D1Result<T>[]>;
}

interface Env {
  ASSETS: { fetch: (req: Request | string) => Promise<Response> };
  COMMENTS: D1Database;
  DOCS_SECRET: string;
}

type PagesContext = { request: Request; env: Env; next: () => Promise<Response> };
type AccessMap = Record<string, "public" | "private">;
type Role = "owner" | "agent" | "client" | "anon";
type State = "open" | "addressed" | "accepted";

const OWNER_COOKIE = "docs_owner";
const SHARE_COOKIE = "docs_share";
const OWNER_DAYS = 30;
const OWNER_NAME = "Alex";
const MAX_BODY = 4000;
const MAX_EXACT = 300;
const MAX_CONTEXT = 32;
const MAX_HEADING = 200;
const MAX_AUTHOR = 40;
const PAGE_RE = /^[a-z0-9][a-z0-9-]*\/[a-z0-9][a-z0-9-]*$/;
const ID_RE = /^[0-9a-f]{12}$/;
const STATES: State[] = ["open", "addressed", "accepted"];

const enc = new TextEncoder();

// ---------- crypto / cookies (same scheme as _middleware.ts) ----------

async function hmac(secret: string, msg: string): Promise<string> {
  const key = await crypto.subtle.importKey("raw", enc.encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const sig = await crypto.subtle.sign("HMAC", key, enc.encode(msg));
  return [...new Uint8Array(sig)].map((b) => b.toString(16).padStart(2, "0")).join("").slice(0, 40);
}

function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let out = 0;
  for (let i = 0; i < a.length; i++) out |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return out === 0;
}

function cookies(req: Request): Array<[string, string]> {
  const raw = req.headers.get("cookie") || "";
  return raw.split(/;\s*/).filter(Boolean).map((c) => {
    const i = c.indexOf("=");
    return i < 0 ? [c, ""] : [c.slice(0, i), decodeURIComponent(c.slice(i + 1))];
  });
}

async function validOwnerValue(value: string, secret: string): Promise<boolean> {
  const [ts, sig] = value.split(".");
  const t = Number(ts);
  if (!t || !sig) return false;
  if (Date.now() - t > OWNER_DAYS * 864e5) return false;
  return timingSafeEqual(sig, await hmac(secret, `owner|${t}`));
}

function decodeToken(tok: string): { prefix: string; exp: number; sig: string } | null {
  try {
    const s = atob(tok.replace(/-/g, "+").replace(/_/g, "/"));
    const [prefix, exp, sig] = s.split("|");
    if (!prefix || !exp || !sig) return null;
    return { prefix, exp: Number(exp), sig };
  } catch {
    return null;
  }
}

async function tokenValidFor(tok: string, prefix: string, secret: string): Promise<boolean> {
  const t = decodeToken(tok);
  if (!t || t.prefix !== prefix) return false;
  if (t.exp && Date.now() > t.exp) return false;
  return timingSafeEqual(t.sig, await hmac(secret, `${t.prefix}|${t.exp}`));
}

async function loadAccess(env: Env, origin: string): Promise<AccessMap> {
  try {
    const r = await env.ASSETS.fetch(`${origin}/_access.json`);
    if (!r.ok) return {};
    return (await r.json()) as AccessMap;
  } catch {
    return {};
  }
}

// ---------- viewer ----------

interface Viewer {
  role: Role;
  name: string | null;
  shares: string[];
}

function trimmed(v: unknown, max: number): string {
  return typeof v === "string" ? v.trim().slice(0, max) : "";
}

async function resolveViewer(req: Request, url: URL, secret: string): Promise<Viewer> {
  const shares: string[] = [];
  let owner = false;
  for (const [name, value] of cookies(req)) {
    if (name === OWNER_COOKIE && (await validOwnerValue(value, secret))) owner = true;
    // The page cookie is path-scoped; a comments-scoped copy may also be present.
    if (name === SHARE_COOKIE || name === `${SHARE_COOKIE}_c`) shares.push(value);
  }
  const auth = req.headers.get("authorization") || "";
  const bearer = auth.startsWith("Bearer ") ? auth.slice(7).trim() : "";
  if (bearer && (await validOwnerValue(bearer, secret))) {
    if ((req.headers.get("x-docs-role") || "").toLowerCase() === "agent") {
      return { role: "agent", name: trimmed(req.headers.get("x-docs-author"), MAX_AUTHOR) || "agent", shares };
    }
    return { role: "owner", name: OWNER_NAME, shares };
  }
  // A share token may also arrive out of band (header or query) when the
  // path-scoped cookie cannot reach /_comments.
  const headerShare = req.headers.get("x-docs-share");
  if (headerShare) shares.push(headerShare);
  const queryShare = url.searchParams.get("share");
  if (queryShare) shares.push(queryShare);
  if (owner) return { role: "owner", name: OWNER_NAME, shares };
  if (shares.length) return { role: "client", name: null, shares };
  return { role: "anon", name: null, shares };
}

async function clientCovers(viewer: Viewer, page: string, secret: string): Promise<boolean> {
  const prefix = `/${page}/`;
  for (const tok of viewer.shares) if (await tokenValidFor(tok, prefix, secret)) return true;
  return false;
}

async function canRead(viewer: Viewer, page: string, access: AccessMap, secret: string): Promise<boolean> {
  if (viewer.role === "owner" || viewer.role === "agent") return true;
  if (access[`/${page}/`] === "public") return true;
  return clientCovers(viewer, page, secret);
}

async function canWrite(viewer: Viewer, page: string, access: AccessMap, secret: string): Promise<boolean> {
  if (viewer.role === "owner" || viewer.role === "agent") return true;
  if (viewer.role === "anon") return false;
  void access;
  return clientCovers(viewer, page, secret);
}

// ---------- helpers ----------

function json(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store" },
  });
}

function fail(message: string, status: number): Response {
  return json({ error: message }, status);
}

function denied(viewer: Viewer, what: string): Response {
  return viewer.role === "anon" ? fail(`sign in to ${what}`, 401) : fail(`not allowed to ${what}`, 403);
}

function newId(): string {
  const b = new Uint8Array(6);
  crypto.getRandomValues(b);
  return [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
}

type Row = {
  id: string;
  page: string;
  parent: string | null;
  anchor: string | null;
  author: string;
  role: string;
  body: string;
  state: string | null;
  created: number;
  updated: number;
};

function parseAnchor(raw: string | null): unknown {
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

// A single row as the API returns it: anchor parsed, storage flags dropped.
function shapeRow(r: Row): Record<string, unknown> {
  return {
    id: r.id,
    page: r.page,
    parent: r.parent,
    anchor: parseAnchor(r.anchor),
    author: r.author,
    role: r.role,
    body: r.body,
    state: r.state,
    created: r.created,
    updated: r.updated,
  };
}

function shapeRoot(r: Row, replies: Row[]): Record<string, unknown> {
  return {
    id: r.id,
    page: r.page,
    anchor: parseAnchor(r.anchor),
    author: r.author,
    role: r.role,
    body: r.body,
    state: r.state || "open",
    created: r.created,
    updated: r.updated,
    replies: replies.map((x) => ({ id: x.id, author: x.author, role: x.role, body: x.body, created: x.created })),
  };
}

function buildThreads(rows: Row[]): Record<string, unknown>[] {
  const roots = rows.filter((r) => !r.parent).sort((a, b) => a.created - b.created);
  return roots.map((root) =>
    shapeRoot(root, rows.filter((r) => r.parent === root.id).sort((a, b) => a.created - b.created)),
  );
}

// Normalized anchor JSON, or `false` when the shape is wrong.
function anchorJson(raw: unknown): string | null | false {
  if (raw === undefined || raw === null) return null;
  if (typeof raw !== "object") return false;
  const a = raw as Record<string, unknown>;
  if (typeof a.exact !== "string") return false;
  const exact = a.exact.trim();
  if (!exact || exact.length > MAX_EXACT) return false;
  return JSON.stringify({
    exact,
    prefix: trimmed(a.prefix, MAX_CONTEXT),
    suffix: trimmed(a.suffix, MAX_CONTEXT),
    heading: trimmed(a.heading, MAX_HEADING),
  });
}

async function readRow(env: Env, id: string): Promise<Row | null> {
  return (await env.COMMENTS.prepare("SELECT * FROM comments WHERE id = ? AND deleted = 0").bind(id).first<Row>()) || null;
}

async function readThread(env: Env, rootId: string): Promise<{ root: Row; replies: Row[] } | null> {
  const root = await readRow(env, rootId);
  if (!root || root.parent) return null;
  const rs = await env.COMMENTS.prepare("SELECT * FROM comments WHERE parent = ? AND deleted = 0 ORDER BY created ASC")
    .bind(rootId)
    .all<Row>();
  return { root, replies: rs.results || [] };
}

// ---------- handlers ----------

async function handleGet(req: Request, url: URL, env: Env, viewer: Viewer, access: AccessMap): Promise<Response> {
  const secret = env.DOCS_SECRET;

  if (url.searchParams.get("summary")) {
    const rs = await env.COMMENTS
      .prepare("SELECT page, state, COUNT(*) AS n FROM comments WHERE deleted = 0 AND parent IS NULL GROUP BY page, state")
      .all<{ page: string; state: string | null; n: number }>();
    const out: Record<string, Record<State, number>> = {};
    for (const row of rs.results || []) {
      if (!PAGE_RE.test(row.page)) continue;
      if (!(row.page in out)) {
        if (!(await canRead(viewer, row.page, access, secret))) continue;
        out[row.page] = { open: 0, addressed: 0, accepted: 0 };
      }
      const st = (STATES as string[]).includes(row.state || "") ? (row.state as State) : "open";
      out[row.page][st] += Number(row.n) || 0;
    }
    return json(out);
  }

  const id = url.searchParams.get("id");
  if (id) {
    if (!ID_RE.test(id)) return fail("bad comment id", 400);
    const t = await readThread(env, id);
    if (!t) return fail("no such thread", 404);
    if (!(await canRead(viewer, t.root.page, access, secret))) return denied(viewer, "read this page");
    return json({
      viewer: { role: viewer.role, name: viewer.name, canWrite: await canWrite(viewer, t.root.page, access, secret) },
      thread: shapeRoot(t.root, t.replies),
    });
  }

  const page = url.searchParams.get("page") || "";
  if (!PAGE_RE.test(page)) return fail("page must be <project>/<slug>", 400);
  if (!(await canRead(viewer, page, access, secret))) return denied(viewer, "read this page");
  const rs = await env.COMMENTS.prepare("SELECT * FROM comments WHERE page = ? AND deleted = 0 ORDER BY created ASC")
    .bind(page)
    .all<Row>();
  const name = viewer.name || trimmed(url.searchParams.get("author"), MAX_AUTHOR) || null;
  return json({
    viewer: { role: viewer.role, name, canWrite: await canWrite(viewer, page, access, secret) },
    threads: buildThreads(rs.results || []),
  });
}

async function handlePost(req: Request, env: Env, viewer: Viewer, access: AccessMap): Promise<Response> {
  let payload: Record<string, unknown>;
  try {
    payload = (await req.json()) as Record<string, unknown>;
  } catch {
    return fail("invalid JSON body", 400);
  }
  if (!payload || typeof payload !== "object") return fail("invalid JSON body", 400);

  const text = typeof payload.body === "string" ? payload.body.trim() : "";
  if (!text) return fail("body is required", 400);
  if (text.length > MAX_BODY) return fail(`body is longer than ${MAX_BODY} characters`, 400);

  const parentId = payload.parent === undefined || payload.parent === null ? "" : String(payload.parent);
  let parent: Row | null = null;
  if (parentId) {
    if (!ID_RE.test(parentId)) return fail("bad parent id", 400);
    parent = await readRow(env, parentId);
    if (!parent || parent.parent) return fail("no such thread", 404);
  }

  // The page comes from the parent when replying, so the CLI can reply by id.
  const page = parent ? parent.page : typeof payload.page === "string" ? payload.page : "";
  if (!PAGE_RE.test(page)) return fail("page must be <project>/<slug>", 400);
  if (parent && typeof payload.page === "string" && payload.page && payload.page !== page) {
    return fail("page does not match the parent thread", 400);
  }
  if (!(await canWrite(viewer, page, access, env.DOCS_SECRET))) return denied(viewer, "comment on this page");

  const anchor = parent ? null : anchorJson(payload.anchor);
  if (anchor === false) return fail(`anchor needs an "exact" string of 1-${MAX_EXACT} characters`, 400);

  const given = trimmed(payload.author, MAX_AUTHOR);
  let author: string;
  if (viewer.role === "owner") author = given || OWNER_NAME;
  else if (viewer.role === "agent") author = viewer.name || "agent";
  else {
    if (!given) return fail("author is required", 400);
    author = given;
  }

  const now = Date.now();
  const id = newId();
  const state = parent ? null : "open";
  await env.COMMENTS
    .prepare(
      "INSERT INTO comments (id, page, parent, anchor, author, role, body, state, created, updated, deleted) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)",
    )
    .bind(id, page, parent ? parent.id : null, anchor, author, viewer.role, text, state, now, now)
    .run();

  // A rebuttal from the reviewer reopens a thread the agent marked addressed.
  if (parent && (viewer.role === "owner" || viewer.role === "client") && parent.state === "addressed") {
    await env.COMMENTS.prepare("UPDATE comments SET state = 'open', updated = ? WHERE id = ?").bind(now, parent.id).run();
  }

  const row = await readRow(env, id);
  return json(row ? shapeRow(row) : null, 201);
}

async function handlePatch(req: Request, env: Env, viewer: Viewer, access: AccessMap): Promise<Response> {
  let payload: Record<string, unknown>;
  try {
    payload = (await req.json()) as Record<string, unknown>;
  } catch {
    return fail("invalid JSON body", 400);
  }
  const id = typeof payload?.id === "string" ? payload.id : "";
  const state = typeof payload?.state === "string" ? payload.state : "";
  if (!ID_RE.test(id)) return fail("bad comment id", 400);
  if (!(STATES as string[]).includes(state)) return fail(`state must be one of ${STATES.join(", ")}`, 400);

  const row = await readRow(env, id);
  if (!row || row.parent) return fail("no such thread", 404);
  if (!(await canWrite(viewer, row.page, access, env.DOCS_SECRET))) return denied(viewer, "change this thread");
  if (viewer.role === "agent" && state !== "addressed") return fail("an agent may only mark a thread addressed", 403);
  if (viewer.role === "client" && state === "addressed") return fail("only an agent or the owner marks a thread addressed", 403);

  const now = Date.now();
  await env.COMMENTS.prepare("UPDATE comments SET state = ?, updated = ? WHERE id = ?").bind(state, now, id).run();
  const updated = await readRow(env, id);
  return json(updated ? shapeRow(updated) : null);
}

async function handleDelete(url: URL, env: Env, viewer: Viewer): Promise<Response> {
  if (viewer.role !== "owner") return denied(viewer, "delete comments");
  const id = url.searchParams.get("id") || "";
  if (!ID_RE.test(id)) return fail("bad comment id", 400);
  const row = await readRow(env, id);
  if (!row) return fail("no such comment", 404);
  const now = Date.now();
  await env.COMMENTS.prepare("UPDATE comments SET deleted = 1, updated = ? WHERE id = ? OR parent = ?").bind(now, id, id).run();
  return json({ ok: true });
}

export const onRequest = async (ctx: PagesContext): Promise<Response> => {
  const { request, env } = ctx;
  if (!env.DOCS_SECRET) return fail("site not configured: DOCS_SECRET is missing", 503);
  if (!env.COMMENTS) return fail("comments database is not bound", 503);
  const url = new URL(request.url);
  // A bearer that does not verify is a misconfigured client, not an anonymous
  // reader: say so instead of silently answering with public data.
  const auth = request.headers.get("authorization") || "";
  if (auth.startsWith("Bearer ") && !(await validOwnerValue(auth.slice(7).trim(), env.DOCS_SECRET))) {
    return fail("bearer token is not valid; check DOCS_SECRET", 401);
  }
  const access = await loadAccess(env, url.origin);
  const viewer = await resolveViewer(request, url, env.DOCS_SECRET);
  try {
    switch (request.method) {
      case "GET":
      case "HEAD":
        return await handleGet(request, url, env, viewer, access);
      case "POST":
        return await handlePost(request, env, viewer, access);
      case "PATCH":
        return await handlePatch(request, env, viewer, access);
      case "DELETE":
        return await handleDelete(url, env, viewer);
      case "OPTIONS":
        return new Response(null, { status: 204, headers: { allow: "GET, POST, PATCH, DELETE", "cache-control": "no-store" } });
      default:
        return fail("method not allowed", 405);
    }
  } catch (e) {
    return fail(`comments failed: ${(e as Error)?.message || String(e)}`, 500);
  }
};
