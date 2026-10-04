import { afterAll, beforeAll, expect, spyOn, test } from "bun:test";
import { chmod, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import type { Server } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createLocalJWKSet, exportJWK, generateKeyPair, SignJWT, type JWTVerifyGetKey } from "jose";
import { loadConfig, validateAccessConfig } from "../src/config";
import { createAccessVerifier } from "../src/access";
import { start } from "../src/server";

const access = { team: "synthetic-team", aud: "a".repeat(64), email: "Alex@example.test" };
const issuer = `https://${access.team}.cloudflareaccess.com`;
const pathToken = crypto.randomUUID() + crypto.randomUUID();
const headers = { "Content-Type": "application/json", Accept: "application/json, text/event-stream" };
const body = JSON.stringify({ jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-03-26", capabilities: {}, clientInfo: { name: "access-test", version: "1" } } });
let key: CryptoKey;
let keys: JWTVerifyGetKey;
const servers: Server[] = [];
let local: string, publicUrl: string, unconfigured: string, noPath: string;
let dir: string;
let valid: string;

async function sign(claims: Record<string, unknown> = {}, algorithm = "RS256") {
  const now = Math.floor(Date.now() / 1000);
  return new SignJWT({ iss: issuer, aud: ["another-app", access.aud], email: "alex@EXAMPLE.test", exp: now + 120, nbf: now - 1, ...claims })
    .setProtectedHeader({ alg: algorithm, kid: "synthetic-key" })
    .sign(algorithm === "HS256" ? crypto.getRandomValues(new Uint8Array(32)) : key);
}

beforeAll(async () => {
  dir = await mkdtemp(join(tmpdir(), "aoc-live-access-"));
  const pair = await generateKeyPair("RS256");
  key = pair.privateKey;
  keys = createLocalJWKSet({ keys: [{ ...await exportJWK(pair.publicKey), kid: "synthetic-key", alg: "RS256" }] });
  valid = await sign();
  const config = loadConfig({ XDG_CONFIG_HOME: dir, AOC_LIVE_PORT: "0", AOC_LIVE_PUBLIC_PORT: "0", AOC_LIVE_TOKEN: pathToken });
  const active = await start({ config: { ...config, access }, tools: [], accessKeys: keys });
  const off = await start({ config, tools: [] });
  const tokenless = await start({ config: { ...config, access, pathToken: undefined }, tools: [], accessKeys: keys });
  servers.push(active.local, active.public, off.local, off.public, tokenless.local, tokenless.public);
  local = `http://127.0.0.1:${(active.local.address() as AddressInfo).port}`;
  publicUrl = `http://127.0.0.1:${(active.public.address() as AddressInfo).port}`;
  unconfigured = `http://127.0.0.1:${(off.public.address() as AddressInfo).port}`;
  noPath = `http://127.0.0.1:${(tokenless.public.address() as AddressInfo).port}`;
});
afterAll(async () => {
  await Promise.all(servers.map(server => new Promise<void>(resolve => {
    server.closeAllConnections();
    server.close(() => resolve());
  })));
  await rm(dir, { recursive: true, force: true });
});

async function request(base: string, jwt?: string, extra: Record<string, string> = {}, path = `/mcp/${pathToken}`) {
  return fetch(base + path, { method: "POST", headers: { ...headers, ...(jwt ? { "Cf-Access-Jwt-Assertion": jwt } : {}), ...extra }, body });
}

test("public listener accepts the owner JWT, including audience arrays and email case differences", async () => {
  const response = await request(publicUrl, valid);
  expect(response.status).toBe(200);
  expect((await response.json() as { result: { serverInfo: { name: string } } }).result.serverInfo.name).toBe("aoc-live");
});

for (const [name, claims] of [
  ["wrong audience", { aud: "b".repeat(64) }],
  ["wrong issuer", { iss: "https://other-team.cloudflareaccess.com" }],
  ["expired", { exp: 1 }],
  ["not yet valid", { nbf: Math.floor(Date.now() / 1000) + 3600 }],
  ["wrong email", { email: "other@example.test" }],
  ["missing expiry", { exp: undefined }],
  ["missing email", { email: undefined }],
] as const) {
  test(`public listener rejects ${name} with an empty response`, async () => {
    const response = await request(publicUrl, await sign(claims));
    expect(response.status).toBe(401);
    expect(await response.text()).toBe("");
  });
}

test("public listener rejects unsigned, HS256, tampered, malformed and missing assertions", async () => {
  const unsigned = `${Buffer.from(JSON.stringify({ alg: "none" })).toString("base64url")}.${Buffer.from(JSON.stringify({ iss: issuer, aud: access.aud, email: access.email, exp: Math.floor(Date.now() / 1000) + 60 })).toString("base64url")}.`;
  const parts = valid.split(".");
  parts[2] = (parts[2]![0] === "A" ? "B" : "A") + parts[2]!.slice(1);
  for (const assertion of [undefined, unsigned, await sign({}, "HS256"), parts.join("."), crypto.randomUUID()]) {
    const response = await request(publicUrl, assertion);
    expect(response.status).toBe(401);
    expect(await response.text()).toBe("");
  }
});

test("public listener still enforces the path token, Host and Origin without logging credentials", async () => {
  const log = spyOn(console, "info").mockImplementation(() => {});
  try {
    const scenarios: { base: string; path: string; extra: Record<string, string>; status: number }[] = [
      { base: publicUrl, path: "/", extra: {}, status: 404 },
      { base: publicUrl, path: `/mcp/${crypto.randomUUID()}`, extra: {}, status: 404 },
      { base: noPath, path: `/mcp/${pathToken}`, extra: {}, status: 404 },
      { base: publicUrl, path: `/mcp/${pathToken}`, extra: { Host: "evil.example" }, status: 403 },
      { base: publicUrl, path: `/mcp/${pathToken}`, extra: { Origin: `https://evil.example/${pathToken}?jwt=${valid}` }, status: 403 },
    ];
    for (const scenario of scenarios) {
      const response = await request(scenario.base, valid, scenario.extra, scenario.path);
      expect(response.status).toBe(scenario.status);
      expect(await response.text()).toBe("");
    }
    expect(log.mock.calls.map(call => call[0])).toEqual(["access rejected: path", "access rejected: path", "access rejected: path", "access rejected: host", "access rejected: origin"]);
  } finally { log.mockRestore(); }
});

test("local listener ignores Access assertions but cannot bypass the path token", async () => {
  for (const assertion of [undefined, crypto.randomUUID()]) {
    const response = await request(local, assertion);
    expect(response.status).toBe(200);
    expect((await response.json() as { result: { serverInfo: { name: string } } }).result.serverInfo.name).toBe("aoc-live");
  }
  for (const path of ["/", "/mcp/", `/mcp/${crypto.randomUUID()}`]) {
    const response = await request(local, valid, {}, path);
    expect(response.status).toBe(404);
    expect(await response.text()).toBe("");
  }
});

test("unconfigured public listener returns empty 503 for every path and method", async () => {
  for (const path of ["/", `/mcp/${pathToken}`, "/other"]) {
    for (const method of ["GET", "POST", "DELETE"]) {
      const response = await fetch(unconfigured + path, { method, headers: { "Cf-Access-Jwt-Assertion": valid, Host: "evil.example", Origin: "null" } });
      expect(response.status).toBe(503);
      expect(await response.text()).toBe("");
    }
  }
});

test("unknown signing key is rejected without using the remote JWKS", async () => {
  const other = await generateKeyPair("RS256");
  const jwt = await new SignJWT({ email: access.email }).setProtectedHeader({ alg: "RS256", kid: "unknown-key" }).setIssuer(issuer).setAudience(access.aud).setExpirationTime("2m").sign(other.privateKey);
  expect(await createAccessVerifier(access, keys)(jwt)).toBe("invalid jwt");
});

test("Access file loading validates permissions and claims, supports env overrides, and fails closed", async () => {
  const configHome = join(dir, "config");
  const file = join(configHome, "aoc/live/access.json");
  await mkdir(join(configHome, "aoc/live"), { recursive: true });
  const env = { XDG_CONFIG_HOME: configHome };
  expect(loadConfig(env).access).toBeUndefined();
  await writeFile(file, JSON.stringify(access), { mode: 0o600 });
  expect(loadConfig(env).access).toEqual(access);
  expect(loadConfig({ HOME: configHome, XDG_CONFIG_HOME: join(configHome, "missing") }).access).toBeUndefined();
  expect(loadConfig({ ...env, AOC_LIVE_ACCESS_EMAIL: "replacement@example.test" }).access?.email).toBe("replacement@example.test");
  await chmod(file, 0o644);
  expect(loadConfig(env).access).toBeUndefined();
  await chmod(file, 0o600);
  await writeFile(file, "invalid json");
  expect(loadConfig(env).access).toBeUndefined();
  expect(loadConfig({ ...env, AOC_LIVE_ACCESS_TEAM: access.team, AOC_LIVE_ACCESS_AUD: access.aud, AOC_LIVE_ACCESS_EMAIL: access.email }).access).toEqual(access);
  for (const invalid of [{ ...access, team: "team.example" }, { ...access, team: "TEAM" }, { ...access, aud: "a".repeat(63) }, { ...access, aud: "A".repeat(64) }, { ...access, email: "not-email" }]) {
    expect(() => validateAccessConfig(invalid)).toThrow("Invalid Access configuration");
    await writeFile(file, JSON.stringify(invalid));
    expect(loadConfig(env).access).toBeUndefined();
  }
  expect(() => loadConfig({ ...env, AOC_LIVE_PUBLIC_PORT: "65536" })).toThrow("Invalid AOC_LIVE_PUBLIC_PORT");
  expect(() => loadConfig({ ...env, AOC_LIVE_PUBLIC_PORT: "8765" })).toThrow("Local and public ports must differ");
});
