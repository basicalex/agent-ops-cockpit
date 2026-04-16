import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import { existsSync, mkdirSync, readFileSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";

type SettingsPackageEntry = string | { source?: string };

type SettingsFile = {
	packages?: SettingsPackageEntry[];
};

type ModelsJsonProvider = {
	api?: string;
	baseUrl?: string;
	apiKey?: string;
	authHeader?: boolean;
	headers?: Record<string, unknown>;
	compat?: Record<string, unknown>;
	modelOverrides?: Record<string, unknown>;
	models?: unknown[];
};

type ModelsJsonFile = {
	providers?: Record<string, ModelsJsonProvider>;
};

type MigrationResult = {
	changed: boolean;
	backupPath?: string;
	message?: string;
};

const STATUS_ID = "aoc-models";
const DEFAULT_OPENROUTER_BASE_URL = "https://openrouter.ai/api/v1";
const DEFAULT_OPENROUTER_REFERER = "https://github.com/ceii/agent-ops-cockpit";
const DEFAULT_OPENROUTER_TITLE = "AOC";
const OPENROUTER_KEY_HELPER = "openrouter-key-from-multi-auth";

function readJsonFile<T>(path: string, fallback: T): T {
	try {
		return JSON.parse(readFileSync(path, "utf8")) as T;
	} catch {
		return fallback;
	}
}

function writeJsonFile(path: string, value: unknown): void {
	mkdirSync(dirname(path), { recursive: true });
	writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function getAgentRuntimeRoot(): string {
	const envRoot = process.env.PI_CODING_AGENT_DIR?.trim();
	return envRoot ? resolve(envRoot) : join(homedir(), ".pi", "agent");
}

function getAgentSettingsPath(): string {
	return join(getAgentRuntimeRoot(), "settings.json");
}

function getProjectSettingsPath(): string {
	return resolve(process.cwd(), ".pi", "settings.json");
}

function getAgentModelsPath(): string {
	return join(getAgentRuntimeRoot(), "models.json");
}

function getLegacyBackupsDir(): string {
	return join(getAgentRuntimeRoot(), "backups");
}

function getLegacyHelperPath(): string {
	return join(getAgentRuntimeRoot(), "bin", OPENROUTER_KEY_HELPER);
}

function getErrorMessage(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

function packageSourceMatches(entry: SettingsPackageEntry | undefined): boolean {
	if (!entry) return false;
	const source = typeof entry === "string" ? entry : entry.source;
	return typeof source === "string" && source.includes("pi-multi-auth");
}

function isMultiAuthInstalled(): boolean {
	const project = readJsonFile<SettingsFile>(getProjectSettingsPath(), {});
	if ((project.packages ?? []).some(packageSourceMatches)) return true;
	const agent = readJsonFile<SettingsFile>(getAgentSettingsPath(), {});
	return (agent.packages ?? []).some(packageSourceMatches);
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function normalizeStringRecord(value: unknown): Record<string, string> | undefined {
	if (!isRecord(value)) return undefined;
	const entries = Object.entries(value)
		.filter((entry): entry is [string, string] => typeof entry[1] === "string")
		.map(([key, item]) => [key.trim(), item.trim()] as const)
		.filter(([key, item]) => key.length > 0 && item.length > 0);
	return entries.length > 0 ? Object.fromEntries(entries) : undefined;
}

function hasOwnKeys(value: unknown): boolean {
	return isRecord(value) && Object.keys(value).length > 0;
}

function resolveDefaultOpenRouterHeaders(): Record<string, string> {
	const referer = process.env.OPENROUTER_HTTP_REFERER?.trim()
		|| process.env.OPENROUTER_REFERER?.trim()
		|| process.env.AOC_OPENROUTER_REFERER?.trim()
		|| DEFAULT_OPENROUTER_REFERER;
	const title = process.env.OPENROUTER_X_TITLE?.trim()
		|| process.env.AOC_OPENROUTER_TITLE?.trim()
		|| DEFAULT_OPENROUTER_TITLE;
	return { "HTTP-Referer": referer, "X-Title": title };
}

function recordsEqual(a: Record<string, string> | undefined, b: Record<string, string> | undefined): boolean {
	if (!a && !b) return true;
	if (!a || !b) return false;
	const aKeys = Object.keys(a).sort();
	const bKeys = Object.keys(b).sort();
	if (aKeys.length !== bKeys.length) return false;
	return aKeys.every((key, index) => key === bKeys[index] && a[key] === b[key]);
}

function isLegacyManagedOpenRouterProvider(entry: ModelsJsonProvider | undefined): boolean {
	if (!entry) return false;
	const apiKey = typeof entry.apiKey === "string" ? entry.apiKey.trim() : "";
	const helperManaged = apiKey.includes(OPENROUTER_KEY_HELPER);
	const headers = normalizeStringRecord(entry.headers);
	const headersMatchDefault = recordsEqual(headers, resolveDefaultOpenRouterHeaders());
	const modelsCount = Array.isArray(entry.models) ? entry.models.length : 0;
	const looksLikeManagedCatalog = modelsCount >= 100;
	const hasExplicitCustomizations = hasOwnKeys(entry.compat) || hasOwnKeys(entry.modelOverrides);
	return helperManaged || (!hasExplicitCustomizations && entry.authHeader === true && headersMatchDefault && looksLikeManagedCatalog);
}

function sanitizeLegacyOpenRouterProvider(entry: ModelsJsonProvider): ModelsJsonProvider | undefined {
	const next: ModelsJsonProvider = {};
	const baseUrl = typeof entry.baseUrl === "string" ? entry.baseUrl.trim() : "";
	if (baseUrl && baseUrl !== DEFAULT_OPENROUTER_BASE_URL) {
		next.baseUrl = baseUrl;
	}

	const headers = normalizeStringRecord(entry.headers);
	if (headers && !recordsEqual(headers, resolveDefaultOpenRouterHeaders())) {
		next.headers = headers;
	}

	if (hasOwnKeys(entry.compat)) next.compat = entry.compat;
	if (hasOwnKeys(entry.modelOverrides)) next.modelOverrides = entry.modelOverrides;

	const models = Array.isArray(entry.models) ? entry.models : [];
	if (models.length > 0 && models.length < 25 && !isLegacyManagedOpenRouterProvider({ ...entry, models })) {
		next.models = models;
	}

	return Object.keys(next).length > 0 ? next : undefined;
}

function backupLegacyOpenRouterProvider(entry: ModelsJsonProvider): string {
	const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
	const backupPath = join(getLegacyBackupsDir(), `openrouter-models.${timestamp}.json`);
	writeJsonFile(backupPath, entry);
	return backupPath;
}

function cleanupLegacyHelperIfUnused(file: ModelsJsonFile): void {
	const serialized = JSON.stringify(file);
	if (serialized.includes(OPENROUTER_KEY_HELPER)) return;
	const helperPath = getLegacyHelperPath();
	if (!existsSync(helperPath)) return;
	try {
		unlinkSync(helperPath);
	} catch {
		return;
	}
	const helperDir = dirname(helperPath);
	try {
		if (existsSync(helperDir) && readFileSync(join(helperDir, ".keep"), "utf8") === "") {
			// no-op; avoid removing managed dirs with sentinels
		}
	} catch {
		// ignore
	}
	try {
		rmSync(helperDir, { recursive: false });
	} catch {
		// ignore non-empty or missing dir
	}
}

function migrateLegacyOpenRouterModelsEntry(): MigrationResult {
	const modelsPath = getAgentModelsPath();
	const file = readJsonFile<ModelsJsonFile>(modelsPath, { providers: {} });
	const providers = { ...(file.providers ?? {}) };
	const entry = providers.openrouter;
	if (!isLegacyManagedOpenRouterProvider(entry)) {
		return { changed: false };
	}

	const backupPath = backupLegacyOpenRouterProvider(entry!);
	const sanitized = sanitizeLegacyOpenRouterProvider(entry!);
	if (sanitized) {
		providers.openrouter = sanitized;
	} else {
		delete providers.openrouter;
	}
	const nextFile: ModelsJsonFile = { providers };
	writeJsonFile(modelsPath, nextFile);
	cleanupLegacyHelperIfUnused(nextFile);

	const preserved = sanitized ? "preserved custom OpenRouter overrides" : "removed managed OpenRouter catalog override";
	return {
		changed: true,
		backupPath,
		message: `AOC migrated legacy OpenRouter bridge state: ${preserved}. Backup: ${backupPath}`,
	};
}

function updateStatus(ctx: ExtensionContext | undefined): void {
	const multiAuth = isMultiAuthInstalled() ? "multi-auth" : "native-only";
	ctx?.ui?.setStatus?.(STATUS_ID, `models:native • OR:${multiAuth}`);
}

function notify(ctx: ExtensionContext, message: string, level: "info" | "success" | "warning" = "info"): void {
	ctx.ui?.notify?.(message, level);
}

function buildStatusMessage(): string {
	return [
		"AOC Models",
		`  OpenRouter catalog: Pi native`,
		`  Multi-auth package: ${isMultiAuthInstalled() ? "active" : "not detected"}`,
		"  Model scope UI: /model and /scoped-models",
		"  Rotation UI: /multi-auth",
		"  Legacy AOC OpenRouter bridge: deprecated; auto-migrated when detected",
	].join("\n");
}

export default function aocModelsExtension(pi: ExtensionAPI): void {
	pi.on("session_start", async (_event, ctx) => {
		try {
			const migration = migrateLegacyOpenRouterModelsEntry();
			if (migration.changed && migration.message) {
				ctx.modelRegistry.refresh();
				notify(ctx, migration.message, "success");
			}
		} catch (error) {
			notify(ctx, `AOC OpenRouter migration failed: ${getErrorMessage(error)}`, "warning");
		}
		updateStatus(ctx);
	});

	pi.on("session_shutdown", async (_event, ctx) => {
		ctx.ui?.setStatus?.(STATUS_ID, undefined);
	});

	pi.registerCommand("aoc-models", {
		description: "AOC model status + legacy OpenRouter bridge migration info.",
		handler: async (args, ctx) => {
			const tokens = (args ?? "").trim().toLowerCase().split(/\s+/).filter(Boolean);
			const sub = tokens[0] ?? "status";
			if (sub === "status" || sub === "scope" || sub === "") {
				notify(ctx, buildStatusMessage(), "info");
				updateStatus(ctx);
				return;
			}
			if (sub === "discover") {
				notify(ctx, "Deprecated: AOC no longer manages the OpenRouter catalog. Use Pi native /model, /scoped-models, and /multi-auth.", "warning");
				updateStatus(ctx);
				return;
			}
			notify(ctx, "Unknown subcommand. Use /aoc-models status, /model, /scoped-models, or /multi-auth.", "warning");
		},
	});

	pi.registerCommand("aoc-model-mode", {
		description: "Deprecated. OpenRouter scope is now managed by Pi native /scoped-models.",
		handler: async (_args, ctx) => {
			notify(ctx, "Deprecated: use Pi native /scoped-models. AOC no longer manages OpenRouter model catalogs.", "info");
			updateStatus(ctx);
		},
	});

	pi.registerCommand("aoc-model-status", {
		description: "Show AOC model migration status.",
		handler: async (_args, ctx) => {
			notify(ctx, buildStatusMessage(), "info");
			updateStatus(ctx);
		},
	});
}
