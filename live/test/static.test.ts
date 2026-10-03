import { expect, test } from "bun:test";
import { chmod, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
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
    const env = { ...process.env, HOME: join(dir, "home"), AOC_LIVE_DIR: packageDir, AOC_LIVE_STATE_DIR: join(dir, "state"), AOC_LIVE_TOKEN_CMD: tokenCommand, AOC_LIVE_PORT: "65534" };
    for (const command of ["start", "status", "help"]) {
      const child = Bun.spawn(["bash", script, command], { env, stdout: "pipe", stderr: "pipe" });
      const [code, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
      expect(code).toBe(0); expect(stderr).toBe("");
      if (command === "start") expect(stdout).toContain("no token stored; staying off");
      if (command === "status") { expect(stdout).toContain("not running"); expect(stdout).toContain("/mcp/<hidden>"); }
      if (command === "help") { expect(stdout).toContain("start | stop | restart"); expect(stdout).not.toContain("up | down"); }
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
    const env = { ...process.env, CODEX_HOME: home, AOC_LIVE_DIR: packageDir, AOC_LIVE_STATE_DIR: join(dir, "state"), AOC_LIVE_TOKEN_CMD: command, AOC_LIVE_PORT: "8765" };
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
    expect(await run("connect")).toBe("connected; restart the ChatGPT desktop app\n");
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
      env: { ...process.env, PATH: `${dir}:${process.env.PATH}`, CODEX_HOME: home, AOC_LIVE_DIR: packageDir, AOC_LIVE_STATE_DIR: join(dir, "state"), AOC_LIVE_TOKEN_CMD: tokenCommand, FAKE_TOKEN_FILE: tokenFile, AOC_LIVE_PORT: "8765" },
      stdout: "pipe", stderr: "pipe",
    });
    const [code, out, err] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
    expect(code).toBe(0); expect(err).toBe("");
    const rotated = (await Bun.file(tokenFile).text()).trim();
    expect(rotated).toMatch(/^[a-f0-9]{64}$/);
    expect(out).toBe("token stored\nconnected; restart the ChatGPT desktop app\n");
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
else
  exit 1
fi
`, { mode: 0o700 });
    const token = "synthetic-mobile-path-token-not-secret-123456789";
    const tokenCommand = join(dir, "token");
    await writeFile(tokenCommand, `#!/usr/bin/env bash\nprintf '%s' '${token}'\n`, { mode: 0o700 });
    const configHome = join(dir, "config");
    const env = { ...process.env, PATH: `${dir}:${process.env.PATH}`, HOME: home, XDG_CONFIG_HOME: configHome, AOC_LIVE_DIR: packageDir, AOC_LIVE_STATE_DIR: join(dir, "state"), AOC_LIVE_TOKEN_CMD: tokenCommand, AOC_LIVE_PORT: "8765", FAKE_CALLS: calls, FAKE_CREATED: marker };
    const run = async (...args: string[]) => {
      const child = Bun.spawn(["bash", script, ...args], { env, stdout: "pipe", stderr: "pipe" });
      const [code, out, err] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
      expect(code).toBe(0); expect(err).toBe("");
      return out;
    };
    expect(await run("url")).toBe("http://127.0.0.1:8765/mcp/<hidden>\n");
    expect(await run("tunnel", "setup", "live.example.test")).toBe("tunnel configured: live.example.test\n");
    const cfg = join(configHome, "aoc/live/cloudflared.yml");
    const credentials = join(configHome, "aoc/live/aoc-live.json");
    expect(await Bun.file(cfg).text()).toBe(`tunnel: ${id}\ncredentials-file: "${credentials}"\ningress:\n  - hostname: "live.example.test"\n    service: http://127.0.0.1:8765\n    originRequest:\n      httpHostHeader: 127.0.0.1:8765\n  - service: http_status:404\n`);
    expect((await Bun.file(cfg).stat()).mode & 0o777).toBe(0o600);
    expect(await Bun.file(existing).text()).toBe("existing intrface and voyager-dev configuration\n");
    await run("tunnel", "setup", "live.example.test");
    expect((await Bun.file(calls).text()).match(/tunnel create /g)).toHaveLength(1);
    expect(await run("tunnel", "status")).toBe("hostname: live.example.test; cloudflared: not running\n");
    const hidden = await run("url");
    expect(hidden).toBe("http://127.0.0.1:8765/mcp/<hidden>\nhttps://live.example.test/mcp/<hidden>\n");
    expect(hidden).not.toContain(token);
    expect(await run("url", "--show")).toBe(`http://127.0.0.1:8765/mcp/${token}\nhttps://live.example.test/mcp/${token}\n`);
  } finally { await rm(dir, { recursive: true, force: true }); }
});
