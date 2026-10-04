import { readFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

export type AccessConfig = { team: string; aud: string; email: string };

export function validateAccessConfig(value: unknown): AccessConfig {
  const config = value as Partial<AccessConfig> | null;
  if (!config || typeof config.team !== "string" || !/^[a-z0-9-]+$/.test(config.team) ||
      typeof config.aud !== "string" || !/^[a-f0-9]{64}$/.test(config.aud) ||
      typeof config.email !== "string" || !/^[^\s@]+@[^\s@]+$/.test(config.email)) {
    throw new Error("Invalid Access configuration");
  }
  return { team: config.team, aud: config.aud, email: config.email };
}

export function loadAccessConfig(env: NodeJS.ProcessEnv = process.env): AccessConfig | undefined {
  const file = join(env.XDG_CONFIG_HOME || join(env.HOME || homedir(), ".config"), "aoc/live/access.json");
  let stored: unknown;
  try {
    if ((statSync(file).mode & 0o777) !== 0o600) return undefined;
    stored = JSON.parse(readFileSync(file, "utf8"));
  } catch { /* Missing or invalid configuration leaves public access off. */ }
  const config = stored as Partial<AccessConfig> | undefined;
  try {
    return validateAccessConfig({
      team: env.AOC_LIVE_ACCESS_TEAM ?? config?.team,
      aud: env.AOC_LIVE_ACCESS_AUD ?? config?.aud,
      email: env.AOC_LIVE_ACCESS_EMAIL ?? config?.email,
    });
  } catch { return undefined; }
}

export type Config = {
  host: "127.0.0.1";
  port: number;
  publicPort: number;
  access?: AccessConfig;
  pathToken?: string;
  claudeDir: string;
  ompDir: string;
};

export function loadConfig(env: NodeJS.ProcessEnv = process.env): Config {
  const port = Number(env.AOC_LIVE_PORT ?? 8765);
  if (!Number.isInteger(port) || port < 0 || port > 65535) throw new Error("Invalid AOC_LIVE_PORT");
  const publicPort = Number(env.AOC_LIVE_PUBLIC_PORT ?? 8766);
  if (!Number.isInteger(publicPort) || publicPort < 0 || publicPort > 65535) throw new Error("Invalid AOC_LIVE_PUBLIC_PORT");
  if (port !== 0 && port === publicPort) throw new Error("Local and public ports must differ");
  const token = env.AOC_LIVE_TOKEN?.trim();
  return {
    host: "127.0.0.1", port, publicPort, access: loadAccessConfig(env),
    pathToken: token && token.length >= 32 ? token : undefined,
    claudeDir: env.AOC_LIVE_CLAUDE_DIR ?? join(homedir(), ".claude/projects"),
    ompDir: env.AOC_LIVE_OMP_DIR ?? join(homedir(), ".omp/agent/sessions"),
  };
}
