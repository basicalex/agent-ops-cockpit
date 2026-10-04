import { expect, test } from "bun:test";
import { chmod, mkdir, mkdtemp, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

const packageDir = new URL("../", import.meta.url).pathname;
const script = new URL("../../bin/aoc-live", import.meta.url).pathname;

test("only exec.ts starts subprocesses; CLI has no workspace write commands", async () => {
  for await (const file of new Bun.Glob("*.ts").scan({ cwd: join(packageDir, "src") })) {
    if (file === "exec.ts") continue;
    const source = await Bun.file(join(packageDir, "src", file)).text();
    expect(source).not.toMatch(/child_process|Bun\.spawn|Bun\.\$|spawnSync|execSync/);
  }
  const cli = await Bun.file(script).text();
  expect(cli).not.toMatch(/\bherdr\s+(?:pane\s+(?:run|send-[\w-]+|agent\s+prompt)|orchestrate)|\bgit\s+(?:add|commit|push|reset|stash)|\bgh\s+(?:issue|pr)\s+(?:create|edit|close|comment)/);
});

test("start without a token and stopped status work in isolation without Keychain or tunnels", async () => {
  const dir = await mkdtemp(join(packageDir, "test/fixtures/w1/runtime-cli-"));
  try {
    const tokenCommand = join(dir, "no-token");
    await writeFile(tokenCommand, "#!/usr/bin/env bash\nexit 1\n"); await chmod(tokenCommand, 0o700);
    await mkdir(join(dir, "home"));
    const env = { ...process.env, HOME: join(dir, "home"), XDG_CONFIG_HOME: join(dir, "config"), AOC_LIVE_DIR: packageDir, AOC_LIVE_STATE_DIR: join(dir, "state"), AOC_LIVE_TOKEN_CMD: tokenCommand, AOC_LIVE_PORT: "65534" };
    for (const command of ["start", "status", "help"]) {
      const child = Bun.spawn(["bash", script, command], { env, stdout: "pipe", stderr: "pipe" });
      const [code, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
      expect(code).toBe(0); expect(stderr).toBe("");
      if (command === "start") expect(stdout).toContain("no token stored; staying off");
      if (command === "status") { expect(stdout).toContain("not running"); expect(stdout).toContain("/mcp/<hidden>"); }
      if (command === "help") expect(stdout).not.toContain("No authentication");
    }
    expect(await Bun.file(join(dir, "state/supervisor.pid")).exists()).toBe(false);
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test("desktop connect/disconnect preserve neighboring tables and nested-table boundaries", async () => {
  const dir = await mkdtemp(join(packageDir, "test/fixtures/w1/runtime-desktop-"));
  try {
    const home = join(dir, "codex"); await mkdir(home);
    const token = "synthetic-desktop-token-not-a-secret-123456789";
    const command = join(dir, "token");
    await writeFile(command, `#!/usr/bin/env bash\nprintf '%s' '${token}'\n`, { mode: 0o700 });
    const env = { ...process.env, XDG_CONFIG_HOME: join(dir, "config"), CODEX_HOME: home, AOC_LIVE_DIR: packageDir, AOC_LIVE_STATE_DIR: join(dir, "state"), AOC_LIVE_TOKEN_CMD: command, AOC_LIVE_PORT: "8765" };
    const prefix = '# settings\n[mcp_servers.before]\ncommand = "before"\n\n';
    const suffix = '[mcp_servers.after]\ncommand = "after"\n# keep this exact\n';
    const original = prefix + '[mcp_servers.aoc-live]\nurl = "old"\n[mcp_servers.aoc-live.env]\nOLD = "remove"\n\n' + suffix;
    const config = join(home, "config.toml");
    await writeFile(config, original);
    const run = async (action: string) => {
      const child = Bun.spawn(["bash", script, action], { env, stdout: "pipe", stderr: "pipe" });
      const [code, out, err] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
      expect(code).toBe(0); expect(err).toBe(""); expect(out).not.toContain(token);
      return out;
    };
    await run("connect");
    const connected = await Bun.file(config).text();
    expect(connected).toBe(prefix + `[mcp_servers.aoc-live]\nurl = "http://127.0.0.1:8765/mcp/${token}"\ntool_timeout_sec = 30\n` + suffix);
    expect(await Bun.file(config + ".aoc-live.bak").text()).toBe(original);
    expect((await Bun.file(config).stat()).mode & 0o777).toBe(0o600);
    await run("connect");
    expect(await Bun.file(config).text()).toBe(connected);
    expect(await Bun.file(config + ".aoc-live.bak").text()).toBe(original);
    await run("disconnect");
    expect(await Bun.file(config).text()).toBe(prefix + suffix);
    expect(await Bun.file(config + ".aoc-live.bak").text()).toBe(connected);
    await run("disconnect");
    expect(await Bun.file(config).text()).toBe(prefix + suffix);
    await run("connect");
    expect(await Bun.file(config).text()).toBe(prefix + suffix + `[mcp_servers.aoc-live]\nurl = "http://127.0.0.1:8765/mcp/${token}"\ntool_timeout_sec = 30\n`);
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test("token rotate reconnects an existing desktop table using isolated security commands", async () => {
  const dir = await mkdtemp(join(packageDir, "test/fixtures/w1/runtime-rotate-"));
  try {
    const home = join(dir, "codex"); await mkdir(home);
    const config = join(home, "config.toml");
    await writeFile(config, '[mcp_servers.aoc-live]\nurl = "old"\n');
    const tokenFile = join(dir, "stored-token");
    await writeFile(join(dir, "security"), '#!/usr/bin/env bash\nset -euo pipefail\nwhile [[ $# -gt 0 ]]; do\n  if [[ $1 == -w ]]; then printf "%s\\n" "$2" > "$FAKE_TOKEN_FILE"; exit 0; fi\n  shift\ndone\nexit 1\n', { mode: 0o700 });
    const tokenCommand = join(dir, "token");
    await writeFile(tokenCommand, '#!/usr/bin/env bash\nIFS= read -r token < "$FAKE_TOKEN_FILE"\nprintf "%s" "$token"\n', { mode: 0o700 });
    const child = Bun.spawn(["bash", script, "token", "rotate"], {
      env: { ...process.env, PATH: `${dir}:${process.env.PATH}`, XDG_CONFIG_HOME: join(dir, "config"), CODEX_HOME: home, AOC_LIVE_DIR: packageDir, AOC_LIVE_STATE_DIR: join(dir, "state"), AOC_LIVE_TOKEN_CMD: tokenCommand, FAKE_TOKEN_FILE: tokenFile, AOC_LIVE_PORT: "8765" },
      stdout: "pipe", stderr: "pipe",
    });
    const [code, out, err] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
    expect(code).toBe(0); expect(err).toBe("");
    const rotated = (await Bun.file(tokenFile).text()).trim();
    expect(rotated).toMatch(/^[a-f0-9]{64}$/);
    expect(out).not.toContain(rotated);
    expect(await Bun.file(config).text()).toContain(`/mcp/${rotated}`);
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test("tunnel setup writes isolated loopback ingress, reuses the tunnel, and hides URLs by default", async () => {
  const dir = await mkdtemp(join(packageDir, "test/fixtures/w1/runtime-tunnel-"));
  try {
    const home = join(dir, "home"); await mkdir(join(home, ".cloudflared"), { recursive: true });
    const existing = join(home, ".cloudflared/config.yml");
    await writeFile(existing, "existing intrface and voyager-dev configuration\n");
    const id = "11111111-2222-3333-4444-555555555555";
    const calls = join(dir, "calls");
    const marker = join(dir, "created");
    await writeFile(join(dir, "cloudflared"), `#!/usr/bin/env bash
set -euo pipefail
printf '%s\\n' "$*" >> "$FAKE_CALLS"
if [[ $2 == list ]]; then
  if [[ -f $FAKE_CREATED ]]; then printf '%s\\n' '[{"id":"${id}","name":"aoc-live","deleted_at":"0001-01-01T00:00:00Z"}]'; else printf '[]\\n'; fi
elif [[ $2 == create ]]; then
  [[ $3 == --credentials-file && $5 == aoc-live ]]
  printf '{}\\n' > "$4"
  printf 'created\\n' > "$FAKE_CREATED"
elif [[ $2 == route ]]; then
  [[ $3 == dns && $4 == aoc-live ]]
elif [[ $2 == --config && $4 == ingress && $5 == validate ]]; then
  [[ -f $3 ]]
else
  exit 1
fi
`, { mode: 0o700 });
    const token = "synthetic-mobile-path-token-not-secret-123456789";
    const tokenCommand = join(dir, "token");
    await writeFile(tokenCommand, `#!/usr/bin/env bash\nprintf '%s' '${token}'\n`, { mode: 0o700 });
    const noToken = join(dir, "no-token");
    await writeFile(noToken, "#!/usr/bin/env bash\nexit 1\n", { mode: 0o700 });
    const configHome = join(dir, "config");
    const env = { ...process.env, PATH: `${dir}:${process.env.PATH}`, HOME: home, XDG_CONFIG_HOME: configHome, AOC_LIVE_DIR: packageDir, AOC_LIVE_STATE_DIR: join(dir, "state"), AOC_LIVE_TOKEN_CMD: tokenCommand, AOC_LIVE_PORT: "8765", AOC_LIVE_PUBLIC_PORT: "9876", AOC_LIVE_ACCESS_TEAM: undefined, AOC_LIVE_ACCESS_AUD: undefined, AOC_LIVE_ACCESS_EMAIL: undefined, FAKE_CALLS: calls, FAKE_CREATED: marker };
    const run = async (...args: string[]) => {
      const child = Bun.spawn(["bash", script, ...args], { env: { ...env, AOC_LIVE_TOKEN_CMD: ["enable", "disable"].includes(args[1] ?? "") ? noToken : tokenCommand }, stdout: "pipe", stderr: "pipe" });
      const [code, out, err] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
      expect(code).toBe(0); expect(err).toBe("");
      return out;
    };
    expect(await run("url")).toBe("http://127.0.0.1:8765/mcp/<hidden>\n");
    const aud = "a".repeat(64);
    await run("access", "set", "--team", "synthetic-team", "--aud", aud, "--email", "alex@example.test");
    const accessFile = join(configHome, "aoc/live/access.json");
    expect(await Bun.file(accessFile).json()).toEqual({ team: "synthetic-team", aud, email: "alex@example.test" });
    expect((await Bun.file(accessFile).stat()).mode & 0o777).toBe(0o600);
    const shown = await run("access", "show");
    expect(shown).toContain("synthetic-team");
    expect(shown).toContain("alex@example.test");
    expect(shown).toContain(aud.slice(0, 8));
    expect(shown).not.toContain(aud);
    await run("tunnel", "setup", "live.example.test");
    const cfg = join(configHome, "aoc/live/cloudflared.yml");
    const credentials = join(configHome, "aoc/live/aoc-live.json");
    expect(await Bun.file(cfg).exists()).toBe(false);
    const data = Bun.YAML.parse(await Bun.file(cfg + ".disabled").text()) as { ingress: { hostname?: string; service: string; originRequest?: { httpHostHeader: string; access: { required: boolean; teamName: string; audTag: string[] } } }[] };
    expect(data.ingress).toEqual([
      { hostname: "live.example.test", service: "http://127.0.0.1:9876", originRequest: { httpHostHeader: "127.0.0.1:9876", access: { required: true, teamName: "synthetic-team", audTag: [aud] } } },
      { service: "http_status:404" },
    ]);
    await run("tunnel", "enable");
    expect((await Bun.file(cfg).stat()).mode & 0o777).toBe(0o600);
    expect(await Bun.file(existing).text()).toBe("existing intrface and voyager-dev configuration\n");
    await run("tunnel", "setup", "live.example.test");
    expect((await Bun.file(calls).text()).match(/tunnel create /g)).toHaveLength(1);
    expect(await run("tunnel", "status")).toContain("tunnel: enabled");
    const hidden = await run("url");
    expect(hidden).toBe("http://127.0.0.1:8765/mcp/<hidden>\nhttps://live.example.test/mcp/<hidden>\n");
    expect(hidden).not.toContain(token);
    expect(await run("url", "--show")).toBe(`http://127.0.0.1:8765/mcp/${token}\nhttps://live.example.test/mcp/${token}\n`);
    await run("tunnel", "disable");
    expect(await Bun.file(cfg).exists()).toBe(false);
    expect(await Bun.file(cfg + ".disabled").exists()).toBe(true);
    await run("access", "set", "--team", "replacement-team", "--aud", "b".repeat(64), "--email", "alex@example.test");
    const refreshed = Bun.YAML.parse(await Bun.file(cfg + ".disabled").text()) as typeof data;
    expect(refreshed.ingress[0]?.originRequest?.access).toEqual({ required: true, teamName: "replacement-team", audTag: ["b".repeat(64)] });
    expect(await Bun.file(cfg).exists()).toBe(false);
    await run("tunnel", "enable");
    const status = await run("status");
    expect(status).toContain("local port 8765");
    expect(status).toContain("public port 9876");
    expect(status).toContain("access configured yes");
    expect(status).toContain("tunnel enabled");
    expect((await Bun.file(calls).text()).match(/ingress validate/g)?.length).toBeGreaterThanOrEqual(5);
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test("tunnel enable rejects absent, misleading, mismatched and fail-open Access configurations", async () => {
  const dir = await mkdtemp(join(packageDir, "test/fixtures/w1/runtime-access-cli-"));
  try {
    const configHome = join(dir, "config");
    const configDir = join(configHome, "aoc/live");
    await mkdir(configDir, { recursive: true });
    const cfg = join(configDir, "cloudflared.yml");
    const file = join(configDir, "access.json");
    const marker = join(dir, "cloudflared-called");
    await writeFile(join(dir, "cloudflared"), '#!/usr/bin/env bash\nprintf "called\\n" > "$FAKE_MARKER"\nexit 0\n', { mode: 0o700 });
    const env = { ...process.env, PATH: `${dir}:${process.env.PATH}`, HOME: dir, XDG_CONFIG_HOME: configHome, AOC_LIVE_DIR: packageDir, AOC_LIVE_STATE_DIR: join(dir, "state"), AOC_LIVE_PORT: "8765", AOC_LIVE_PUBLIC_PORT: "8766", AOC_LIVE_ACCESS_TEAM: undefined, AOC_LIVE_ACCESS_AUD: undefined, AOC_LIVE_ACCESS_EMAIL: undefined, FAKE_MARKER: marker };
    const aud = "a".repeat(64);
    const guard = { required: true, teamName: "synthetic-team", audTag: [aud] };
    const route = { hostname: "live.example.test", service: "http://127.0.0.1:8766", originRequest: { httpHostHeader: "127.0.0.1:8766", access: guard } };
    const run = async (...args: string[]) => {
      const child = Bun.spawn(["bash", script, ...args], { env, stdout: "pipe", stderr: "pipe" });
      const [code, out, err] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
      return { code, out, err };
    };
    await writeFile(cfg + ".disabled", Bun.YAML.stringify({ ingress: [route, { service: "http_status:404" }] }));
    expect((await run("tunnel", "enable")).code).toBe(1);
    expect(await Bun.file(cfg).exists()).toBe(false);
    await writeFile(file, JSON.stringify({ team: guard.teamName, aud, email: "alex@example.test" }), { mode: 0o600 });
    for (const access of [undefined, { ...guard, required: false }, { ...guard, teamName: "wrong-team" }, { ...guard, audTag: ["b".repeat(64)] }]) {
      await writeFile(cfg + ".disabled", Bun.YAML.stringify({ ingress: [{ ...route, originRequest: { ...route.originRequest, access } }, { service: "http_status:404" }] }) + `\n# access: required: true teamName: synthetic-team audTag: ${aud}\n`);
      const result = await run("tunnel", "enable");
      expect(result.code).toBe(1);
      expect(result.err).toBe("tunnel blocked: access not configured\n");
      expect(await Bun.file(cfg).exists()).toBe(false);
      expect(await Bun.file(cfg + ".disabled").exists()).toBe(true);
    }
    await writeFile(cfg + ".disabled", Bun.YAML.stringify({ ingress: [route, { service: "http://127.0.0.1:8765" }, { service: "http_status:404" }] }));
    expect((await run("tunnel", "enable")).code).toBe(1);
    await rename(cfg + ".disabled", cfg);
    expect((await run("status")).out).toContain("tunnel blocked");
    expect(await Bun.file(marker).exists()).toBe(false);
    for (const [team, invalidAud, email] of [["bad.team", aud, "alex@example.test"], ["synthetic-team", "a".repeat(63), "alex@example.test"], ["synthetic-team", aud, "not-email"]]) {
      expect((await run("access", "set", "--team", team!, "--aud", invalidAud!, "--email", email!)).code).toBe(1);
      expect(await Bun.file(file).json()).toEqual({ team: guard.teamName, aud, email: "alex@example.test" });
    }
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test("isolated supervisor blocks missing or mismatched Access and starts only the guarded tunnel", async () => {
  const dir = await mkdtemp(join(packageDir, "test/fixtures/w1/runtime-access-supervisor-"));
  const reservations = [Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => new Response() }), Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => new Response() })];
  const [port, publicPort] = reservations.map(server => server.port!);
  for (const server of reservations) server.stop(true);
  const configHome = join(dir, "config");
  const configDir = join(configHome, "aoc/live");
  const stateDir = join(dir, "state");
  const marker = join(dir, "cloudflared-run");
  const token = crypto.randomUUID() + crypto.randomUUID();
  const tokenCommand = join(dir, "token");
  const env = { ...process.env, PATH: `${dir}:${process.env.PATH}`, HOME: dir, XDG_CONFIG_HOME: configHome, AOC_LIVE_DIR: packageDir, AOC_LIVE_STATE_DIR: stateDir, AOC_LIVE_TOKEN_CMD: tokenCommand, AOC_LIVE_PORT: String(port), AOC_LIVE_PUBLIC_PORT: String(publicPort), AOC_LIVE_ACCESS_TEAM: undefined, AOC_LIVE_ACCESS_AUD: undefined, AOC_LIVE_ACCESS_EMAIL: undefined, FAKE_MARKER: marker };
  const run = async (...args: string[]) => {
    const child = Bun.spawn(["bash", script, ...args], { env, stdout: "pipe", stderr: "pipe" });
    const [code, out, err] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
    expect(code).toBe(0); expect(err).toBe("");
    return out;
  };
  // Real shell children own their clocks; wait for observable state, not a fixed startup delay.
  const until = async (condition: () => Promise<boolean>) => {
    const end = Date.now() + 8000;
    while (Date.now() < end) { if (await condition()) return; await Bun.sleep(100); }
    throw new Error("Isolated supervisor did not reach the expected state");
  };
  try {
    await mkdir(configDir, { recursive: true });
    await writeFile(tokenCommand, `#!/usr/bin/env bash\nprintf '%s' '${token}'\n`, { mode: 0o700 });
    await writeFile(join(dir, "cloudflared"), '#!/usr/bin/env bash\nset -euo pipefail\n[[ $* == *" run" ]] || exit 1\nprintf "running\\n" > "$FAKE_MARKER"\nexec bun -e "setInterval(()=>{},1000)"\n', { mode: 0o700 });
    const aud = "a".repeat(64);
    const config = (team: string) => Bun.YAML.stringify({ ingress: [{ hostname: "live.example.test", service: `http://127.0.0.1:${publicPort}`, originRequest: { httpHostHeader: `127.0.0.1:${publicPort}`, access: { required: true, teamName: team, audTag: [aud] } } }, { service: "http_status:404" }] });
    const cfg = join(configDir, "cloudflared.yml");
    for (const phase of ["missing", "mismatched", "configured"]) {
      if (phase !== "missing") await writeFile(join(configDir, "access.json"), JSON.stringify({ team: "synthetic-team", aud, email: "alex@example.test" }), { mode: 0o600 });
      await writeFile(cfg, config(phase === "mismatched" ? "wrong-team" : "synthetic-team"));
      await rm(join(stateDir, "provider.log"), { force: true });
      await run("start");
      if (phase === "configured") {
        await until(() => Bun.file(marker).exists());
        const response = await fetch(`http://127.0.0.1:${publicPort}/mcp/${token}`, { method: "POST" });
        expect(response.status).toBe(401);
        expect(await response.text()).toBe("");
      } else {
        await until(async () => await Bun.file(join(stateDir, "provider.log")).exists() && (await Bun.file(join(stateDir, "provider.log")).text()).includes("tunnel blocked: access not configured"));
        expect(await Bun.file(marker).exists()).toBe(false);
        expect(await run("status")).toContain("tunnel blocked");
      }
      await until(async () => {
        try {
          const response = await fetch(`http://127.0.0.1:${port}/mcp/${token}`, { method: "POST", headers: { "Content-Type": "application/json", Accept: "application/json, text/event-stream" }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-03-26", capabilities: {}, clientInfo: { name: "supervisor-test", version: "1" } } }) });
          return response.status === 200 && (await response.json() as { result: { serverInfo: { name: string } } }).result.serverInfo.name === "aoc-live";
        } catch { return false; }
      });
      await run("stop");
      expect(await Bun.file(join(stateDir, "supervisor.pid")).exists()).toBe(false);
    }
  } finally {
    await run("stop");
    await rm(dir, { recursive: true, force: true });
  }
}, 30000);
