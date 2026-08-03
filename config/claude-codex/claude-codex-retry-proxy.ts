import { mkdir } from "node:fs/promises";

const LISTEN_HOST = "127.0.0.1";
const LISTEN_PORT = Number(Bun.env.LISTEN_PORT ?? "8318");
const UPSTREAM = Bun.env.UPSTREAM_BASE_URL ?? "http://127.0.0.1:8317";
const TARGET_MODEL = "gpt-5.6-sol";
const FALLBACK_MODEL = "gpt-5.6-terra";
const SOL_BACKOFF_MS = [2_000, 4_000, 8_000];
const PROMPT_RULES_PATH =
  Bun.env.CLAUDE_CODEX_PROMPT_RULES ?? `${Bun.env.HOME}/.config/cli-proxy-api/system-prompt-rules.json`;
const DUMP_SYSTEM_DIR = Bun.env.CLAUDE_CODEX_DUMP_SYSTEM;

type PromptRule = {
  name: string;
  find: string;
  replace: string;
  regex: boolean;
  flags: string;
  dedupe: boolean;
};

type RewriteState = {
  seen: boolean;
  matches: number;
};

const HOP_BY_HOP_HEADERS: Record<string, true> = {
  connection: true,
  "keep-alive": true,
  "proxy-authenticate": true,
  "proxy-authorization": true,
  te: true,
  trailer: true,
  "transfer-encoding": true,
  upgrade: true,
  host: true,
  "content-length": true,
};

const TRANSIENT_STATUSES: Record<number, true> = {
  429: true,
  500: true,
  502: true,
  503: true,
  504: true,
};

type AttemptPlan = {
  model: string;
  solAttempt: number;
  totalAttempt: number;
  isFallback: boolean;
};

type SseInspection =
  | { kind: "commit"; response: Response }
  | { kind: "transient"; className: string };

type LastTransient =
  | { kind: "http"; response: Response }
  | { kind: "sse"; className: string };

function metadataLog(parts: Record<string, string | number | boolean>): void {
  console.error(
    Object.entries(parts)
      .map(([key, value]) => `${key}=${String(value)}`)
      .join(" "),
  );
}

function parsePromptRules(value: unknown): PromptRule[] {
  if (!Array.isArray(value)) throw new TypeError("Prompt rules must be an array");

  const names = new Set<string>();
  return value.map((candidate) => {
    if (
      candidate === null ||
      typeof candidate !== "object" ||
      typeof candidate.name !== "string" ||
      typeof candidate.find !== "string" ||
      typeof candidate.replace !== "string" ||
      typeof candidate.regex !== "boolean" ||
      typeof candidate.flags !== "string" ||
      typeof candidate.dedupe !== "boolean"
    ) {
      throw new TypeError("Invalid prompt rule");
    }
    if (candidate.name.length === 0 || candidate.find.length === 0 || names.has(candidate.name)) {
      throw new TypeError("Prompt rule names and find strings must be non-empty and names must be unique");
    }
    if (candidate.regex) new RegExp(candidate.find, candidate.flags);
    names.add(candidate.name);
    return candidate as PromptRule;
  });
}

let promptRules: PromptRule[] = [];
try {
  promptRules = parsePromptRules(await Bun.file(PROMPT_RULES_PATH).json());
} catch (error) {
  metadataLog({
    prompt_rules: "disabled",
    path: PROMPT_RULES_PATH,
    error: error instanceof Error ? error.name : typeof error,
  });
}

function countLiteralMatches(text: string, find: string): number {
  let count = 0;
  let offset = 0;
  while ((offset = text.indexOf(find, offset)) !== -1) {
    count += 1;
    offset += find.length;
  }
  return count;
}

function rewriteText(text: string, rule: PromptRule, state: RewriteState): string {
  if (rule.dedupe) {
    if (rule.regex) {
      const flags = rule.flags.includes("g") ? rule.flags : `${rule.flags}g`;
      return text.replace(new RegExp(rule.find, flags), (match) => {
        state.matches += 1;
        if (!state.seen) {
          state.seen = true;
          return match;
        }
        return "";
      });
    }

    let offset = 0;
    let output = "";
    while (true) {
      const matchAt = text.indexOf(rule.find, offset);
      if (matchAt === -1) return output + text.slice(offset);
      output += text.slice(offset, matchAt);
      state.matches += 1;
      if (!state.seen) {
        state.seen = true;
        output += rule.find;
      }
      offset = matchAt + rule.find.length;
    }
  }

  if (rule.regex) {
    const expression = new RegExp(rule.find, rule.flags);
    if (rule.flags.includes("g")) {
      state.matches += Array.from(text.matchAll(expression)).length;
    } else if (expression.test(text)) {
      state.matches += 1;
    }
    return text.replace(new RegExp(rule.find, rule.flags), rule.replace);
  }

  const matches = countLiteralMatches(text, rule.find);
  if (rule.flags.includes("g")) {
    state.matches += matches;
    return text.replaceAll(rule.find, rule.replace);
  }
  if (matches > 0) {
    state.matches += 1;
    return text.replace(rule.find, rule.replace);
  }
  return text;
}

function rewriteSystem(system: unknown): {
  system: unknown;
  matches: Record<string, number>;
  changed: boolean;
} {
  const states = new Map(promptRules.map((rule) => [rule.name, { seen: false, matches: 0 }]));
  let changed = false;

  const rewriteBlockText = (text: string): string => {
    let rewritten = text;
    for (const rule of promptRules) {
      rewritten = rewriteText(rewritten, rule, states.get(rule.name)!);
    }
    if (rewritten !== text) changed = true;
    return rewritten;
  };

  let rewrittenSystem = system;
  if (typeof system === "string") {
    rewrittenSystem = rewriteBlockText(system);
  } else if (Array.isArray(system)) {
    rewrittenSystem = system.map((block) => {
      if (block === null || typeof block !== "object" || typeof block.text !== "string") return block;
      const text = rewriteBlockText(block.text);
      return text === block.text ? block : { ...block, text };
    });
  }

  const matches: Record<string, number> = {};
  for (const rule of promptRules) {
    const count = states.get(rule.name)!.matches;
    if (count > 0) matches[rule.name] = count;
  }
  return { system: rewrittenSystem, matches, changed };
}

async function rewriteMessagesBody(request: Request, body: ArrayBuffer): Promise<ArrayBuffer> {
  const url = new URL(request.url);
  if (request.method !== "POST" || url.pathname !== "/v1/messages") return body;

  try {
    const json = JSON.parse(new TextDecoder().decode(body)) as Record<string, unknown>;
    if (json === null || typeof json !== "object" || !Object.hasOwn(json, "system")) return body;

    const before = json.system;
    const rewritten = rewriteSystem(before);
    if (DUMP_SYSTEM_DIR) {
      await mkdir(DUMP_SYSTEM_DIR, { recursive: true });
      await Promise.all([
        Bun.write(`${DUMP_SYSTEM_DIR}/latest-pre.json`, `${JSON.stringify(before, null, 2)}\n`),
        Bun.write(`${DUMP_SYSTEM_DIR}/latest-post.json`, `${JSON.stringify(rewritten.system, null, 2)}\n`),
      ]);
    }

    const matchSummary = Object.entries(rewritten.matches)
      .map(([name, count]) => `${name}:${count}`)
      .join(",");
    if (matchSummary) metadataLog({ prompt_rule_matches: matchSummary });
    if (!rewritten.changed) return body;

    json.system = rewritten.system;
    return new TextEncoder().encode(JSON.stringify(json)).buffer as ArrayBuffer;
  } catch (error) {
    metadataLog({ prompt_rewrite_error: error instanceof Error ? error.name : typeof error });
    return body;
  }
}

function filteredRequestHeaders(headers: Headers): Headers {
  const next = new Headers();
  for (const [name, value] of headers) {
    if (!HOP_BY_HOP_HEADERS[name.toLowerCase()]) next.set(name, value);
  }
  return next;
}

function filteredResponseHeaders(headers: Headers): Headers {
  const next = new Headers();
  for (const [name, value] of headers) {
    if (!HOP_BY_HOP_HEADERS[name.toLowerCase()]) next.set(name, value);
  }
  return next;
}

function responseIsSse(response: Response): boolean {
  const contentType = response.headers.get("content-type") ?? "";
  return contentType.toLowerCase().includes("text/event-stream");
}

function joinChunks(chunks: Uint8Array[]): Uint8Array {
  const total = chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0);
  const joined = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    joined.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return joined;
}

function findSseFrameEnd(buffer: Uint8Array): number {
  for (let index = 0; index < buffer.byteLength - 1; index += 1) {
    if (buffer[index] === 10 && buffer[index + 1] === 10) return index + 2;
    if (
      index < buffer.byteLength - 3 &&
      buffer[index] === 13 &&
      buffer[index + 1] === 10 &&
      buffer[index + 2] === 13 &&
      buffer[index + 3] === 10
    ) {
      return index + 4;
    }
  }
  return -1;
}

function parseSseFrame(frameText: string): { eventName: string | null; dataLines: string[]; hasMeaningfulLine: boolean } {
  let eventName: string | null = null;
  const dataLines: string[] = [];
  let hasMeaningfulLine = false;

  for (const rawLine of frameText.split(/\r?\n/)) {
    if (rawLine === "" || rawLine.startsWith(":")) continue;
    hasMeaningfulLine = true;

    const separator = rawLine.indexOf(":");
    const field = separator === -1 ? rawLine : rawLine.slice(0, separator);
    const rawValue = separator === -1 ? "" : rawLine.slice(separator + 1);
    const value = rawValue.startsWith(" ") ? rawValue.slice(1) : rawValue;

    if (field === "event") eventName = value;
    if (field === "data") dataLines.push(value);
  }

  return { eventName, dataLines, hasMeaningfulLine };
}

function valueSignalsTransient(value: unknown): boolean {
  if (typeof value === "number") return TRANSIENT_STATUSES[value] === true;
  if (typeof value !== "string") return false;

  const normalized = value.toLowerCase();
  if (
    normalized === "rate_limit_error" ||
    normalized === "rate_limit_exceeded" ||
    normalized === "service_unavailable_error" ||
    normalized === "server_error" ||
    normalized === "api_error" ||
    normalized === "server_is_overloaded" ||
    normalized === "429" ||
    normalized === "500" ||
    normalized === "502" ||
    normalized === "503" ||
    normalized === "504" ||
    /(^|\D)(429|500|502|503|504)(\D|$)/.test(normalized)
  ) {
    return true;
  }

  return (
    normalized.includes("currently overloaded") ||
    normalized.includes("server is overloaded") ||
    normalized.includes("selected model is at capacity") ||
    normalized.includes("model is at capacity") ||
    normalized.includes("servers are at capacity")
  );
}

function errorPayloadIsTransient(dataLines: string[]): boolean {
  if (dataLines.length === 0) return false;
  const data = dataLines.join("\n").trim();
  if (data === "" || data === "[DONE]") return false;

  try {
    const parsed = JSON.parse(data) as unknown;
    if (!parsed || typeof parsed !== "object") return false;
    const record = parsed as Record<string, unknown>;
    const nested = record.error && typeof record.error === "object" ? (record.error as Record<string, unknown>) : null;
    const candidates = [
      record.type,
      record.code,
      record.status,
      record.message,
      nested?.type,
      nested?.code,
      nested?.status,
      nested?.message,
    ];

    return candidates.some(valueSignalsTransient);
  } catch {
    return false;
  }
}

function classifySseFrame(frame: Uint8Array): "keepalive" | "transient" | "commit" {
  const parsed = parseSseFrame(new TextDecoder().decode(frame));
  if (!parsed.hasMeaningfulLine) return "keepalive";
  if (errorPayloadIsTransient(parsed.dataLines)) return "transient";

  const data = parsed.dataLines.join("\n").trim();
  let payloadType = "";
  try {
    const payload = JSON.parse(data) as Record<string, unknown>;
    payloadType = typeof payload.type === "string" ? payload.type : "";
  } catch {
    // Non-JSON data is output and commits the stream.
  }
  if (parsed.eventName === "message_start" || parsed.eventName === "ping" || payloadType === "message_start" || payloadType === "ping") {
    return "keepalive";
  }
  return "commit";
}

function responseFromUpstream(upstream: Response, body: BodyInit | null): Response {
  return new Response(body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers: filteredResponseHeaders(upstream.headers),
  });
}

async function inspectSseBeforeCommit(upstream: Response): Promise<SseInspection> {
  if (!upstream.body) return { kind: "commit", response: responseFromUpstream(upstream, null) };

  const reader = upstream.body.getReader();
  const prefixChunks: Uint8Array[] = [];
  let pending = new Uint8Array(0);

  while (true) {
    const { value, done } = await reader.read();
    if (done) {
      if (pending.byteLength > 0) prefixChunks.push(pending);
      const prefix = joinChunks(prefixChunks);
      return { kind: "commit", response: responseFromUpstream(upstream, prefix) };
    }

    if (value.byteLength > 0) {
      pending = pending.byteLength === 0 ? value : joinChunks([pending, value]);
    }

    while (true) {
      const frameEnd = findSseFrameEnd(pending);
      if (frameEnd === -1) break;

      const frame = pending.slice(0, frameEnd);
      const classification = classifySseFrame(frame);
      prefixChunks.push(frame);
      pending = pending.slice(frameEnd);

      if (classification === "keepalive") continue;

      if (classification === "transient") {
        await reader.cancel().catch(() => undefined);
        return { kind: "transient", className: "sse_error" };
      }

      if (pending.byteLength > 0) prefixChunks.push(pending);
      const prefix = joinChunks(prefixChunks);
      const stream = new ReadableStream<Uint8Array>({
        start(controller) {
          if (prefix.byteLength > 0) controller.enqueue(prefix);
        },
        async pull(controller) {
          const next = await reader.read();
          if (next.done) {
            controller.close();
            return;
          }
          if (next.value.byteLength > 0) controller.enqueue(next.value);
        },
        async cancel(reason) {
          await reader.cancel(reason).catch(() => undefined);
        },
      });
      return { kind: "commit", response: responseFromUpstream(upstream, stream) };
    }
  }
}

function withModel(body: ArrayBuffer, model: string): ArrayBuffer {
  const text = new TextDecoder().decode(body);
  const json = JSON.parse(text) as Record<string, unknown>;
  json.model = model;
  return new TextEncoder().encode(JSON.stringify(json)).buffer as ArrayBuffer;
}

async function abortAwareSleep(ms: number, signal: AbortSignal): Promise<void> {
  if (signal.aborted) throw signal.reason ?? new DOMException("Aborted", "AbortError");
  await new Promise<void>((resolve, reject) => {
    const timeout = setTimeout(resolve, ms);
    signal.addEventListener(
      "abort",
      () => {
        clearTimeout(timeout);
        reject(signal.reason ?? new DOMException("Aborted", "AbortError"));
      },
      { once: true },
    );
  });
}

async function proxyOnce(request: Request, body: ArrayBuffer, modelOverride?: string): Promise<Response> {
  const url = new URL(request.url);
  const init: RequestInit = {
    method: request.method,
    headers: filteredRequestHeaders(request.headers),
    body: request.method === "GET" || request.method === "HEAD" ? undefined : modelOverride ? withModel(body, modelOverride) : body,
    signal: request.signal,
    redirect: "manual",
  };

  return fetch(`${UPSTREAM}${url.pathname}${url.search}`, init);
}

async function proxyUnchanged(request: Request, body: ArrayBuffer): Promise<Response> {
  const upstream = await proxyOnce(request, body);
  return responseFromUpstream(upstream, upstream.body);
}

function attemptPlans(): AttemptPlan[] {
  return [
    { model: TARGET_MODEL, solAttempt: 1, totalAttempt: 1, isFallback: false },
    { model: TARGET_MODEL, solAttempt: 2, totalAttempt: 2, isFallback: false },
    { model: TARGET_MODEL, solAttempt: 3, totalAttempt: 3, isFallback: false },
    { model: TARGET_MODEL, solAttempt: 4, totalAttempt: 4, isFallback: false },
    { model: FALLBACK_MODEL, solAttempt: 4, totalAttempt: 5, isFallback: true },
  ];
}

async function proxyTarget(request: Request, body: ArrayBuffer): Promise<Response> {
  let lastTransient: LastTransient | null = null;

  for (const plan of attemptPlans()) {
    if (plan.totalAttempt > 1 && !plan.isFallback) {
      await abortAwareSleep(SOL_BACKOFF_MS[plan.solAttempt - 2], request.signal);
    }

    const upstream = await proxyOnce(request, body, plan.model);
    metadataLog({ attempt: plan.totalAttempt, model: plan.model, status: upstream.status });

    if (TRANSIENT_STATUSES[upstream.status]) {
      await upstream.body?.cancel().catch(() => undefined);
      lastTransient = { kind: "http", response: upstream };
      metadataLog({ attempt: plan.totalAttempt, model: plan.model, transient: `http_${upstream.status}` });
      continue;
    }

    if (!responseIsSse(upstream)) return responseFromUpstream(upstream, upstream.body);

    const inspected = await inspectSseBeforeCommit(upstream);
    if (inspected.kind === "commit") return inspected.response;

    lastTransient = { kind: "sse", className: inspected.className };
    metadataLog({ attempt: plan.totalAttempt, model: plan.model, transient: inspected.className });
  }

  if (lastTransient?.kind === "http") return responseFromUpstream(lastTransient.response, null);

  return new Response(
    JSON.stringify({
      type: "error",
      error: {
        type: "api_error",
        message: "Upstream returned transient pre-output SSE errors after retry and fallback attempts.",
      },
    }),
    {
      status: 502,
      headers: { "content-type": "application/json" },
    },
  );
}

function isTargetMessagesRequest(request: Request, body: ArrayBuffer): boolean {
  const url = new URL(request.url);
  if (request.method !== "POST" || url.pathname !== "/v1/messages") return false;

  try {
    const json = JSON.parse(new TextDecoder().decode(body)) as Record<string, unknown>;
    return json.model === TARGET_MODEL;
  } catch {
    return false;
  }
}

Bun.serve({
  hostname: LISTEN_HOST,
  port: LISTEN_PORT,
  async fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/healthz") {
      return new Response("ok\n", { status: 200, headers: { "content-type": "text/plain; charset=utf-8" } });
    }

    const body = request.method === "GET" || request.method === "HEAD" ? new ArrayBuffer(0) : await request.arrayBuffer();
    const rewrittenBody = await rewriteMessagesBody(request, body);
    if (isTargetMessagesRequest(request, rewrittenBody)) return proxyTarget(request, rewrittenBody);
    return proxyUnchanged(request, rewrittenBody);
  },
});

metadataLog({ listen: `${LISTEN_HOST}:${LISTEN_PORT}`, upstream: new URL(UPSTREAM).host });
