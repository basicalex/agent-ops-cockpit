// Access gate for the artifact library on Cloudflare Pages.
//
// Every path is private unless the manifest marks it public. Private paths open
// for (a) the owner cookie, set by /_login with DOCS_PASSWORD, or (b) a signed
// share link (?share=<token>) minted by `artifact-pages share`, which sets a
// path-scoped cookie so the page's own subresources load. Tokens are
// HMAC-SHA256 over "<prefix>|<expiry>" with DOCS_SECRET.

interface Env {
  ASSETS: { fetch: (req: Request | string) => Promise<Response> };
  DOCS_SECRET: string;
  DOCS_PASSWORD: string;
}

type AccessMap = Record<string, "public" | "private">;

const OWNER_COOKIE = "docs_owner";
const SHARE_COOKIE = "docs_share";
const OWNER_DAYS = 30;

const enc = new TextEncoder();

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

async function ownerCookieValue(secret: string, ts: number): Promise<string> {
  return `${ts}.${await hmac(secret, `owner|${ts}`)}`;
}

async function isOwner(req: Request, secret: string): Promise<boolean> {
  for (const [name, value] of cookies(req)) {
    if (name !== OWNER_COOKIE) continue;
    const [ts, sig] = value.split(".");
    const t = Number(ts);
    if (!t || !sig) continue;
    if (Date.now() - t > OWNER_DAYS * 864e5) continue;
    if (timingSafeEqual(sig, await hmac(secret, `owner|${t}`))) return true;
  }
  return false;
}

// token = base64url("<prefix>|<expiryMs>|<sig>")
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

// Longest registered prefix ("/project/slug/") that contains the path.
function matchPrefix(path: string, access: AccessMap): string | null {
  let best: string | null = null;
  for (const p of Object.keys(access)) {
    if (path === p || path.startsWith(p)) {
      if (!best || p.length > best.length) best = p;
    }
  }
  return best;
}

function html(body: string, status = 200, headers: Record<string, string> = {}): Response {
  return new Response(body, { status, headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store", ...headers } });
}

function loginPage(next: string, failed = false): string {
  const safeNext = next.replace(/[^\w\-./]/g, "");
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Sign in</title>
<style>body{margin:0;min-height:100vh;display:grid;place-items:center;font:15px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;background:#f5f1eb;color:#0f1729}
form{background:#fbf9f4;border:1px solid rgba(15,23,41,.14);padding:28px 32px;width:min(360px,calc(100vw - 32px));display:grid;gap:12px}
h1{font-size:17px;font-weight:500;margin:0}p{margin:0;color:#47536b;font-size:13px}
input{font:inherit;padding:10px 12px;border:1px solid rgba(15,23,41,.25);background:#fff}
button{font:inherit;padding:10px 12px;border:0;background:#0f1729;color:#fff;cursor:pointer}
.err{color:#a33a30}</style></head><body>
<form method="post" action="/_login"><h1>Intrface documents</h1><p>This page is private. Enter the owner password to continue.</p>
${failed ? '<p class="err">Wrong password.</p>' : ""}
<input type="hidden" name="next" value="${safeNext}"><input type="password" name="password" autofocus autocomplete="current-password" placeholder="Password" required>
<button type="submit">Open</button></form></body></html>`;
}

async function handleLogin(req: Request, env: Env, url: URL): Promise<Response> {
  if (req.method === "GET") return html(loginPage(url.searchParams.get("next") || "/"));
  if (req.method !== "POST") return new Response("Method not allowed", { status: 405 });
  const form = await req.formData();
  const password = String(form.get("password") || "");
  const next = String(form.get("next") || "/").replace(/[^\w\-./]/g, "") || "/";
  if (!env.DOCS_PASSWORD || !timingSafeEqual(password, env.DOCS_PASSWORD)) return html(loginPage(next, true), 401);
  const value = await ownerCookieValue(env.DOCS_SECRET, Date.now());
  return new Response(null, {
    status: 303,
    headers: {
      location: next,
      "set-cookie": `${OWNER_COOKIE}=${value}; Path=/; Max-Age=${OWNER_DAYS * 86400}; HttpOnly; Secure; SameSite=Lax`,
    },
  });
}

export const onRequest = async (ctx: { request: Request; env: Env; next: () => Promise<Response> }): Promise<Response> => {
  const { request, env } = ctx;
  const url = new URL(request.url);
  const path = url.pathname;

  if (!env.DOCS_SECRET || !env.DOCS_PASSWORD) {
    return new Response("Site not configured: DOCS_SECRET and DOCS_PASSWORD are missing.", { status: 503 });
  }

  if (path === "/_login") return handleLogin(request, env, url);
  if (path === "/_logout") {
    return new Response(null, { status: 303, headers: { location: "/_login", "set-cookie": `${OWNER_COOKIE}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax` } });
  }

  const owner = await isOwner(request, env.DOCS_SECRET);
  if (path === "/_access.json") return owner ? ctx.next() : new Response("Not found", { status: 404 });

  const access = await loadAccess(env, url.origin);
  const prefix = matchPrefix(path, access);
  if (prefix && access[prefix] === "public") return ctx.next();
  if (owner) return ctx.next();

  // Share link: validate, set a path-scoped cookie, redirect to the clean URL.
  const shareParam = url.searchParams.get("share");
  if (prefix && shareParam && (await tokenValidFor(shareParam, prefix, env.DOCS_SECRET))) {
    url.searchParams.delete("share");
    const t = decodeToken(shareParam)!;
    const maxAge = t.exp ? Math.max(60, Math.floor((t.exp - Date.now()) / 1000)) : 30 * 86400;
    return new Response(null, {
      status: 303,
      headers: {
        location: url.pathname + (url.search || ""),
        "set-cookie": `${SHARE_COOKIE}=${encodeURIComponent(shareParam)}; Path=${prefix}; Max-Age=${maxAge}; HttpOnly; Secure; SameSite=Lax`,
      },
    });
  }
  if (prefix) {
    for (const [name, value] of cookies(request)) {
      if (name === SHARE_COOKIE && (await tokenValidFor(value, prefix, env.DOCS_SECRET))) return ctx.next();
    }
  }

  const wantsHtml = (request.headers.get("accept") || "").includes("text/html");
  if (wantsHtml && request.method === "GET") {
    return new Response(null, { status: 302, headers: { location: `/_login?next=${encodeURIComponent(path)}`, "cache-control": "no-store" } });
  }
  return new Response("Unauthorized", { status: 401, headers: { "cache-control": "no-store" } });
};
