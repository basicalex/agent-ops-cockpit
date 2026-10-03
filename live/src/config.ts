import { homedir } from "node:os";
import { join } from "node:path";

export type Config = {
  host: "127.0.0.1";
  port: number;
  pathToken?: string;
  claudeDir: string;
  ompDir: string;
};

export function loadConfig(env: NodeJS.ProcessEnv = process.env): Config {
  const port = Number(env.AOC_LIVE_PORT ?? 8765);
  if (!Number.isInteger(port) || port < 0 || port > 65535) throw new Error("Invalid AOC_LIVE_PORT");
  const token = env.AOC_LIVE_TOKEN?.trim();
  return {
    host: "127.0.0.1", port,
    pathToken: token && token.length >= 32 ? token : undefined,
    claudeDir: env.AOC_LIVE_CLAUDE_DIR ?? join(homedir(), ".claude/projects"),
    ompDir: env.AOC_LIVE_OMP_DIR ?? join(homedir(), ".omp/agent/sessions"),
  };
}
