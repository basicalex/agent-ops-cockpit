import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------


type ProviderModelDefinition = {
	id: string;
	name?: string;
	reasoning?: boolean;
	input?: ("text" | "image")[];
	cost?: { input: number; output: number; cacheRead: number; cacheWrite: number };
	contextWindow?: number;
	maxTokens?: number;
	compat?: Record<string, unknown>;
	api?: string;
	baseUrl?: string;
	headers?: Record<string, string>;
};

type ProviderModelsFileEntry = {
	api: string;
	baseUrl: string;
	apiKey?: string;
	authHeader?: boolean;
	headers?: Record<string, string>;
	models: ProviderModelDefinition[];
};

type MultiAuthModelsFile = {
	providers?: Record<string, ProviderModelsFileEntry>;
};

type MultiAuthStateFile = {
	providers?: Record<string, {
		credentialIds?: string[];
		activeIndex?: number;
		manualActiveCredentialId?: string;
	}>;
};

type DiscoveredModel = {
	id: string;
	name?: string;
	reasoning?: boolean;
	input?: ("text" | "image")[];
	contextWindow?: number;
	maxTokens?: number;
	cost?: { input: number; output: number; cacheRead: number; cacheWrite: number };
	api?: string;
	baseUrl?: string;
};

type DiscoveryMode = "remote" | "registry";

type ProviderDiscoveryResult = {
	provider: string;
	mode: DiscoveryMode;
	api: string;
	baseUrl: string;
	models: ProviderModelDefinition[];
	note?: string;
};

type AuthFile = Record<string, unknown>;

type SettingsPackageEntry = string | { source?: string };

type SettingsFile = {
	packages?: SettingsPackageEntry[];
};


// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STATUS_ID = "aoc-models";
const DEFAULT_OPENROUTER_BASE_URL = "https://openrouter.ai/api/v1";
const DEFAULT_OPENROUTER_REFERER = "https://github.com/ceii/agent-ops-cockpit";
const DEFAULT_OPENROUTER_TITLE = "AOC";
const OPENROUTER_KEY_HELPER = "openrouter-key-from-multi-auth";
const DEFAULT_MODEL_COST = { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 };
const DEFAULT_CONTEXT_WINDOW = 128_000;
const DEFAULT_MAX_TOKENS = 16_384;
const DISCOVERY_TIMEOUT_MS = 20_000;
const REGISTRY_ONLY_DISCOVERY_PROVIDERS = new Set([
	"openai-codex",
]);

const AOC_PROVIDER_IDS = new Set(["openai-codex", "opencode", "openrouter"]);
const MODELS_FILE_MANAGED_PROVIDER_IDS = new Set(["openrouter"]);

// ---------------------------------------------------------------------------
// File I/O helpers
// ---------------------------------------------------------------------------

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

function getAgentModelsPath(): string {
	return join(getAgentRuntimeRoot(), "models.json");
}

function getAgentAuthPath(): string {
	return join(getAgentRuntimeRoot(), "auth.json");
}

function getAgentMultiAuthPath(): string {
	return join(getAgentRuntimeRoot(), "multi-auth.json");
}

function getErrorMessage(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

function packageSourceMatches(entry: SettingsPackageEntry | undefined): boolean {
	if (!entry) return false;
	const source = typeof entry === "string" ? entry : entry.source;
	if (typeof source !== "string") return false;
	return source.includes("pi-multi-auth");
}

function isMultiAuthInstalled(): boolean {
	const agent = readJsonFile<SettingsFile>(getAgentSettingsPath(), {});
	return (agent.packages ?? []).some(packageSourceMatches);
}

// ---------------------------------------------------------------------------
// OpenRouter + models.json management
// ---------------------------------------------------------------------------

function resolveOpenRouterBaseUrl(): string {
	return process.env.OPENROUTER_BASE_URL?.trim() ||
		process.env.AOC_OPENROUTER_BASE_URL?.trim() ||
		DEFAULT_OPENROUTER_BASE_URL;
}

function resolveOpenRouterHeaders(): Record<string, string> {
	const referer = process.env.OPENROUTER_HTTP_REFERER?.trim()
		|| process.env.OPENROUTER_REFERER?.trim()
		|| process.env.AOC_OPENROUTER_REFERER?.trim()
		|| DEFAULT_OPENROUTER_REFERER;
	const title = process.env.OPENROUTER_X_TITLE?.trim()
		|| process.env.AOC_OPENROUTER_TITLE?.trim()
		|| DEFAULT_OPENROUTER_TITLE;
	return { "HTTP-Referer": referer, "X-Title": title };
}

function resolveOpenRouterApiKeySource(): string {
	const helperPath = join(getAgentRuntimeRoot(), "bin", OPENROUTER_KEY_HELPER);
	return `!${helperPath}`;
}

function nextOpenRouterCredentialId(auth: AuthFile): string {
	if (!("openrouter" in auth)) return "openrouter";
	let index = 1;
	while (`openrouter-${index}` in auth) index += 1;
	return `openrouter-${index}`;
}

function ensureOpenRouterCredentialInAuth(): void {
	const apiKey = process.env.OPENROUTER_API_KEY?.trim();
	if (!apiKey) return;
	const authPath = getAgentAuthPath();
	const auth = readJsonFile<AuthFile>(authPath, {});
	const existing = Object.entries(auth)
		.filter(([id]) => id === "openrouter" || /^openrouter-\d+$/.test(id))
		.map(([, v]) => v)
		.filter((v): v is { type: string; key?: string } =>
			typeof v === "object" && v !== null && "type" in v,
		);
	if (existing.some((e) => e.type === "api_key" && e.key?.trim() === apiKey)) return;
	auth[nextOpenRouterCredentialId(auth)] = { type: "api_key", key: apiKey };
	writeJsonFile(authPath, auth);
}

function syncOpenRouterProviderConfig(): boolean {
	if (!isMultiAuthInstalled()) return false;
	const modelsPath = getAgentModelsPath();
	const file = readJsonFile<MultiAuthModelsFile>(modelsPath, { providers: {} });
	const providers = file.providers ?? {};
	const existing = providers.openrouter;
	if (!existing?.models?.length) return false;
	providers.openrouter = {
		api: existing.api ?? "openai-completions",
		baseUrl: resolveOpenRouterBaseUrl(),
		apiKey: resolveOpenRouterApiKeySource(),
		authHeader: true,
		headers: resolveOpenRouterHeaders(),
		models: [...existing.models],
	};
	writeJsonFile(modelsPath, { providers });
	ensureOpenRouterCredentialInAuth();
	return true;
}

function pruneDeprecatedProvidersFromModelsFile(): void {
	const modelsPath = getAgentModelsPath();
	const file = readJsonFile<MultiAuthModelsFile>(modelsPath, { providers: {} });
	const providers = { ...(file.providers ?? {}) };
	let changed = false;
	for (const provider of Object.keys(providers)) {
		if (!MODELS_FILE_MANAGED_PROVIDER_IDS.has(provider)) {
			delete providers[provider];
			changed = true;
		}
	}
	if (changed) {
		writeJsonFile(modelsPath, { providers });
	}
}

function applyManagedSettings(): boolean {
	pruneDeprecatedProvidersFromModelsFile();
	return syncOpenRouterProviderConfig();
}

// ---------------------------------------------------------------------------
// Discovery + models.json sync
// ---------------------------------------------------------------------------

function joinUrl(baseUrl: string, path: string): string {
	return `${baseUrl.replace(/\/+$/, "")}/${path.replace(/^\/+/, "")}`;
}

function normalizeInputModes(input: unknown): ("text" | "image")[] {
	if (!Array.isArray(input)) return ["text"];
	const values = input.filter((value): value is "text" | "image" => value === "text" || value === "image");
	return values.length > 0 ? values : ["text"];
}

function humanizeModelId(id: string): string {
	const tail = id.split("/").slice(-1)[0] || id;
	return tail
		.split(/[-_]/g)
		.map(part => part ? part[0].toUpperCase() + part.slice(1) : part)
		.join(" ") || id;
}

function normalizeModelDefinition(model: any, defaultApi?: string, defaultBaseUrl?: string): ProviderModelDefinition {
	const normalized: ProviderModelDefinition = {
		id: String(model.id),
		name: typeof model.name === "string" && model.name.trim() ? model.name : humanizeModelId(String(model.id)),
		reasoning: Boolean(model.reasoning),
		input: normalizeInputModes(model.input),
		cost: typeof model.cost === "object" && model.cost !== null ? model.cost : DEFAULT_MODEL_COST,
		contextWindow: typeof model.contextWindow === "number" ? model.contextWindow : DEFAULT_CONTEXT_WINDOW,
		maxTokens: typeof model.maxTokens === "number" ? model.maxTokens : DEFAULT_MAX_TOKENS,
	};
	if (model.compat && typeof model.compat === "object") normalized.compat = model.compat;
	if (typeof model.api === "string" && model.api && model.api !== defaultApi) normalized.api = model.api;
	if (typeof model.baseUrl === "string" && model.baseUrl && model.baseUrl !== defaultBaseUrl) normalized.baseUrl = model.baseUrl;
	if (model.headers && typeof model.headers === "object") normalized.headers = model.headers;
	return normalized;
}

function buildGenericModelDefinition(model: DiscoveredModel, defaultApi: string, defaultBaseUrl: string): ProviderModelDefinition {
	const normalized: ProviderModelDefinition = {
		id: model.id,
		name: model.name ?? humanizeModelId(model.id),
		reasoning: model.reasoning ?? false,
		input: model.input ?? ["text"],
		cost: model.cost ?? DEFAULT_MODEL_COST,
		contextWindow: model.contextWindow ?? DEFAULT_CONTEXT_WINDOW,
		maxTokens: model.maxTokens ?? DEFAULT_MAX_TOKENS,
	};
	if (model.baseUrl && model.baseUrl !== defaultBaseUrl) normalized.baseUrl = model.baseUrl;
	if ((model as any).api && (model as any).api !== defaultApi) normalized.api = (model as any).api;
	return normalized;
}

function getRegistryModelsForProvider(ctx: ExtensionContext, provider: string): any[] {
	const allModels = (ctx.modelRegistry as any).getAll?.();
	return Array.isArray(allModels)
		? allModels.filter((model: any) => model?.provider === provider)
		: [];
}

function inferProviderDefaults(
	provider: string,
	registryModels: any[],
	existing: ProviderModelsFileEntry | undefined,
): { api: string; baseUrl: string } | null {
	const sample = registryModels[0];
	const api = existing?.api || sample?.api || (provider === "anthropic" || provider === "opencode" ? "anthropic-messages" : "openai-completions");
	const baseUrl = existing?.baseUrl || sample?.baseUrl;
	if (typeof api !== "string" || !api || typeof baseUrl !== "string" || !baseUrl) return null;
	return { api, baseUrl };
}

async function resolveProviderAuth(ctx: ExtensionContext, provider: string, registryModels: any[]): Promise<{ apiKey?: string; headers: Record<string, string> }> {
	const model = registryModels[0];
	const resolver = (ctx.modelRegistry as any).getApiKeyAndHeaders;
	if (!model || typeof resolver !== "function") return { headers: {} };
	const resolved = await resolver.call(ctx.modelRegistry, model);
	const headers = resolved?.headers && typeof resolved.headers === "object"
		? resolved.headers as Record<string, string>
		: {};
	const apiKey = typeof resolved?.apiKey === "string" && resolved.apiKey.trim()
		? resolved.apiKey.trim()
		: undefined;

	if (provider === "openrouter" && !headers.Authorization && apiKey) {
		return {
			apiKey,
			headers: { ...headers, Authorization: `Bearer ${apiKey}` },
		};
	}
	return { apiKey, headers };
}

async function fetchJson(url: string, headers: Record<string, string>): Promise<any> {
	const timeout = typeof AbortSignal !== "undefined" && typeof (AbortSignal as any).timeout === "function"
		? (AbortSignal as any).timeout(DISCOVERY_TIMEOUT_MS)
		: undefined;
	const response = await fetch(url, {
		method: "GET",
		headers,
		signal: timeout,
	});
	if (!response.ok) {
		const error = new Error(`${response.status} ${response.statusText}`) as Error & { status?: number; url?: string };
		error.status = response.status;
		error.url = url;
		throw error;
	}
	return await response.json();
}

function looksLikeOpenRouterBaseUrl(baseUrl: string): boolean {
	return /openrouter\.ai/i.test(baseUrl);
}

function formatFallbackReason(provider: string, error?: unknown): string {
	if (REGISTRY_ONLY_DISCOVERY_PROVIDERS.has(provider)) {
		return provider === "openai-codex"
			? "uses built-in Codex catalog"
			: "registry-only provider";
	}
	const status = typeof error === "object" && error !== null && "status" in error
		? Number((error as any).status)
		: undefined;
	if (provider === "opencode" && status === 404) return "no /models endpoint";
	if (provider === "openai-codex" && status === 403) return "model listing forbidden";
	if (status === 404) return "no /models endpoint";
	if (status === 403) return "model listing forbidden";
	if (error) return `remote discovery unavailable: ${getErrorMessage(error)}`;
	return "used registry metadata fallback";
}

function formatProviderSummary(items: Array<{ provider: string; count?: number; note?: string }>): string {
	return items
		.map((item) => `${item.provider}${typeof item.count === "number" ? `:${item.count}` : ""}${item.note ? `(${item.note})` : ""}`)
		.join(", ");
}

async function getAvailableProviderIds(ctx: ExtensionContext): Promise<Set<string>> {
	try {
		const refs = await ctx.modelRegistry.getAvailable();
		const providers = new Set<string>();
		for (const ref of Array.isArray(refs) ? refs : []) {
			if (typeof ref === "string") {
				const idx = ref.indexOf("/");
				if (idx > 0) providers.add(ref.slice(0, idx));
			} else if (ref && typeof ref === "object" && "provider" in ref) {
				providers.add(String((ref as any).provider));
			}
		}
		return providers;
	} catch {
		return new Set<string>();
	}
}

function parseOpenAiLikeModels(payload: any): DiscoveredModel[] {
	const data = Array.isArray(payload?.data) ? payload.data : [];
	return data
		.filter((entry): entry is Record<string, unknown> => typeof entry === "object" && entry !== null && typeof entry.id === "string")
		.map((entry) => ({
			id: String(entry.id),
			name: typeof entry.name === "string" ? entry.name : undefined,
		}));
}

function parseOpenRouterModels(payload: any): DiscoveredModel[] {
	const data = Array.isArray(payload?.data) ? payload.data : [];
	return data
		.filter((entry): entry is Record<string, any> => typeof entry === "object" && entry !== null && typeof entry.id === "string")
		.map((entry) => {
			const modalities = Array.isArray(entry.architecture?.input_modalities)
				? entry.architecture.input_modalities
				: [];
			const hasImage = modalities.includes("image");
			return {
				id: String(entry.id),
				name: typeof entry.name === "string" ? entry.name : undefined,
				reasoning: Boolean(entry.reasoning || entry.architecture?.reasoning),
				input: hasImage ? ["text", "image"] : ["text"],
				contextWindow: typeof entry.context_length === "number" ? entry.context_length : undefined,
				maxTokens:
					typeof entry.top_provider?.max_completion_tokens === "number"
						? entry.top_provider.max_completion_tokens
						: undefined,
			};
		});
}

function parseAnthropicModels(payload: any): DiscoveredModel[] {
	const data = Array.isArray(payload?.data) ? payload.data : [];
	return data
		.filter((entry): entry is Record<string, unknown> => typeof entry === "object" && entry !== null && typeof entry.id === "string")
		.map((entry) => ({
			id: String(entry.id),
			name: typeof entry.display_name === "string"
				? entry.display_name
				: typeof entry.name === "string"
					? entry.name
					: undefined,
			reasoning: true,
		}));
}

function mergeDiscoveredModels(
	discovered: DiscoveredModel[],
	registryModels: any[],
	defaultApi: string,
	defaultBaseUrl: string,
): ProviderModelDefinition[] {
	const registryById = new Map(
		registryModels
			.filter((model) => model?.id)
			.map((model) => [String(model.id), model]),
	);
	const merged = new Map<string, ProviderModelDefinition>();

	for (const model of discovered) {
		const existing = registryById.get(model.id);
		merged.set(
			model.id,
			existing
				? normalizeModelDefinition(existing, defaultApi, defaultBaseUrl)
				: buildGenericModelDefinition(model, defaultApi, defaultBaseUrl),
		);
	}

	for (const model of registryModels) {
		if (!model?.id) continue;
		const id = String(model.id);
		if (!merged.has(id)) {
			merged.set(id, normalizeModelDefinition(model, defaultApi, defaultBaseUrl));
		}
	}

	return Array.from(merged.values()).sort((a, b) => a.id.localeCompare(b.id));
}

async function discoverProviderCatalog(
	ctx: ExtensionContext,
	provider: string,
	existing: ProviderModelsFileEntry | undefined,
): Promise<ProviderDiscoveryResult | null> {
	const registryModels = getRegistryModelsForProvider(ctx, provider);
	const defaults = inferProviderDefaults(provider, registryModels, existing);
	if (!defaults) {
		return null;
	}

	const fallbackModels = registryModels.length > 0
		? registryModels.map((model) => normalizeModelDefinition(model, defaults.api, defaults.baseUrl))
		: Array.isArray(existing?.models)
			? existing.models
			: [];
	const registryFallback = (note: string): ProviderDiscoveryResult | null => {
		if (fallbackModels.length === 0) return null;
		return {
			provider,
			mode: "registry",
			api: defaults.api,
			baseUrl: defaults.baseUrl,
			models: fallbackModels,
			note,
		};
	};

	if (REGISTRY_ONLY_DISCOVERY_PROVIDERS.has(provider)) {
		return registryFallback(formatFallbackReason(provider)) ?? null;
	}
	if (provider === "anthropic" && looksLikeOpenRouterBaseUrl(defaults.baseUrl)) {
		return registryFallback("base URL points to OpenRouter") ?? null;
	}

	const auth = await resolveProviderAuth(ctx, provider, registryModels);

	try {
		let discovered: DiscoveredModel[] | null = null;
		if (provider === "openrouter") {
			const payload = await fetchJson(joinUrl(defaults.baseUrl, "models"), {
				accept: "application/json",
				...auth.headers,
			});
			discovered = parseOpenRouterModels(payload);
		} else if (provider === "anthropic") {
			const headers: Record<string, string> = {
				accept: "application/json",
				"anthropic-version": "2023-06-01",
				...auth.headers,
			};
			if (!headers.Authorization && auth.apiKey) {
				if (auth.apiKey.includes("sk-ant-oat")) {
					headers.Authorization = `Bearer ${auth.apiKey}`;
				} else {
					headers["x-api-key"] = auth.apiKey;
				}
			}
			const payload = await fetchJson(joinUrl(defaults.baseUrl, "models"), headers);
			discovered = parseAnthropicModels(payload);
		} else if (["openai-completions", "openai-responses", "openai-codex-responses", "azure-openai-responses"].includes(defaults.api) || provider === "opencode") {
			const headers: Record<string, string> = {
				accept: "application/json",
				...auth.headers,
			};
			if (!headers.Authorization && auth.apiKey) {
				headers.Authorization = `Bearer ${auth.apiKey}`;
			}
			const payload = await fetchJson(joinUrl(defaults.baseUrl, "models"), headers);
			discovered = parseOpenAiLikeModels(payload);
		}

		if (discovered && discovered.length > 0) {
			return {
				provider,
				mode: "remote",
				api: defaults.api,
				baseUrl: defaults.baseUrl,
				models: mergeDiscoveredModels(discovered, registryModels, defaults.api, defaults.baseUrl),
			};
		}
	} catch (error) {
		const fallback = registryFallback(formatFallbackReason(provider, error));
		if (fallback) return fallback;
		throw error;
	}

	return registryFallback("used registry metadata fallback");
}

function buildManagedProviderEntry(
	provider: string,
	discovery: ProviderDiscoveryResult,
	existing: ProviderModelsFileEntry | undefined,
): ProviderModelsFileEntry {
	const entry: ProviderModelsFileEntry = {
		api: discovery.api,
		baseUrl: discovery.baseUrl,
		models: discovery.models,
	};
	if (existing?.apiKey) entry.apiKey = existing.apiKey;
	if (existing?.authHeader) entry.authHeader = existing.authHeader;
	if (existing?.headers) entry.headers = existing.headers;

	if (provider === "openrouter") {
		entry.apiKey = resolveOpenRouterApiKeySource();
		entry.authHeader = true;
		entry.headers = resolveOpenRouterHeaders();
	}

	return entry;
}

async function discoverAndSyncModels(
	ctx: ExtensionContext,
	mode: "configured" | "all" = "configured",
	explicitProviders: string[] = [],
): Promise<string> {
	const multiAuthState = readJsonFile<MultiAuthStateFile>(getAgentMultiAuthPath(), { providers: {} });
	const multiAuthProviders = Object.keys(multiAuthState.providers ?? {})
		.filter((provider) => AOC_PROVIDER_IDS.has(provider))
		.sort();
	if (multiAuthProviders.length === 0) {
		throw new Error(`No multi-auth providers found in ${getAgentMultiAuthPath()}`);
	}

	const availableProviders = await getAvailableProviderIds(ctx);
	const credentialBackedProviders = new Set(
		Object.entries(multiAuthState.providers ?? {})
			.filter(([, value]) => Array.isArray(value?.credentialIds) && value.credentialIds.length > 0)
			.map(([provider]) => provider),
	);
	const configuredProviders = new Set<string>([...availableProviders, ...credentialBackedProviders]);

	let targetProviders: string[];
	const skipped: Array<{ provider: string; note: string }> = [];
	if (explicitProviders.length > 0) {
		const unknown = explicitProviders.filter((provider) => !multiAuthProviders.includes(provider));
		if (unknown.length > 0) {
			throw new Error(`Unknown multi-auth provider(s): ${unknown.join(", ")}`);
		}
		targetProviders = [...new Set(explicitProviders)];
	} else if (mode === "all") {
		targetProviders = multiAuthProviders;
	} else {
		targetProviders = multiAuthProviders.filter((provider) => configuredProviders.has(provider));
		for (const provider of multiAuthProviders) {
			if (!configuredProviders.has(provider)) {
				skipped.push({ provider, note: "not configured" });
			}
		}
	}

	if (targetProviders.length === 0) {
		const suffix = skipped.length > 0 ? ` Skipped: ${formatProviderSummary(skipped)}.` : "";
		return `No providers selected for discovery.${suffix}`;
	}

	const modelsPath = getAgentModelsPath();
	const modelsFile = readJsonFile<MultiAuthModelsFile>(modelsPath, { providers: {} });
	const providers = { ...(modelsFile.providers ?? {}) };
	for (const provider of Object.keys(providers)) {
		if (!MODELS_FILE_MANAGED_PROVIDER_IDS.has(provider)) {
			delete providers[provider];
		}
	}
	const live: Array<{ provider: string; count: number }> = [];
	const fallback: Array<{ provider: string; count: number; note?: string }> = [];
	const catalogOnly: Array<{ provider: string; count: number; note?: string }> = [];
	let totalModels = 0;

	for (const provider of targetProviders) {
		const result = await discoverProviderCatalog(ctx, provider, providers[provider]);
		if (!result) {
			skipped.push({ provider, note: "no model metadata" });
			continue;
		}
		totalModels += result.models.length;
		if (MODELS_FILE_MANAGED_PROVIDER_IDS.has(provider)) {
			providers[provider] = buildManagedProviderEntry(provider, result, providers[provider]);
			if (result.mode === "remote") {
				live.push({ provider, count: result.models.length });
			} else {
				fallback.push({ provider, count: result.models.length, note: result.note });
			}
		} else {
			catalogOnly.push({ provider, count: result.models.length, note: "built-in provider; not written to models.json" });
		}
	}

	writeJsonFile(modelsPath, { providers });
	ensureOpenRouterCredentialInAuth();
	applyManagedSettings();
	ctx.modelRegistry.refresh();
	updateStatus(ctx);

	const parts = [
		`Discovery sync (${explicitProviders.length > 0 ? "explicit" : mode}) → ${modelsPath}.`,
		`providers:${live.length + fallback.length}`,
		`models:${totalModels}`,
	];
	if (live.length > 0) parts.push(`live[${live.length}]: ${formatProviderSummary(live)}`);
	if (fallback.length > 0) parts.push(`fallback[${fallback.length}]: ${formatProviderSummary(fallback)}`);
	if (catalogOnly.length > 0) parts.push(`built-in[${catalogOnly.length}]: ${formatProviderSummary(catalogOnly)}`);
	if (skipped.length > 0) parts.push(`skipped[${skipped.length}]: ${formatProviderSummary(skipped)}`);
	parts.push("Run /reload for native ctrl+l.");
	return parts.join(" ");
}

function hasManagedOpenRouterCatalog(): boolean {
	const file = readJsonFile<MultiAuthModelsFile>(getAgentModelsPath(), { providers: {} });
	return (file.providers?.openrouter?.models?.length ?? 0) > 0;
}

function updateStatus(ctx: ExtensionContext | undefined): void {
	let label = "models:discover";
	if (hasManagedOpenRouterCatalog()) label += " • OR:managed";
	ctx?.ui?.setStatus?.(STATUS_ID, label);
}

function notify(ctx: ExtensionContext, message: string, level: "info" | "success" | "warning" = "info"): void {
	ctx.ui?.notify?.(message, level);
}

function parseCommandArgs(rawArgs: string | undefined): string[] {
	return (rawArgs ?? "")
		.trim()
		.toLowerCase()
		.split(/\s+/)
		.filter(Boolean);
}

// ---------------------------------------------------------------------------
// Extension entry
// ---------------------------------------------------------------------------

export default function aocModelsExtension(pi: ExtensionAPI): void {
	applyManagedSettings();

	pi.on("session_start", async (_event, ctx) => {
		applyManagedSettings();
		ctx.modelRegistry.refresh();
		updateStatus(ctx);
	});

	pi.on("session_shutdown", async (_event, ctx) => {
		ctx.ui?.setStatus?.(STATUS_ID, undefined);
	});

	pi.registerCommand("aoc-models", {
		description: "AOC model operations. Usage: /aoc-models [discover [configured|all|<provider>... ]|status|scope]",
		handler: async (args, ctx) => {
			const tokens = parseCommandArgs(args);
			const sub = tokens[0] ?? "";

			if (sub === "status" || sub === "") {
				const openrouterModels = readJsonFile<MultiAuthModelsFile>(getAgentModelsPath(), { providers: {} }).providers?.openrouter?.models?.length ?? 0;
				notify(
					ctx,
					`AOC Models\n  providers: openai-codex, opencode, openrouter\n  scope owner: Pi built-in /scoped-models\n  OpenRouter managed catalog: ${openrouterModels > 0 ? `${openrouterModels} models` : "off"}`,
					"info",
				);
				updateStatus(ctx);
				return;
			}

			if (sub === "scope") {
				notify(ctx, "Use Pi built-in /scoped-models to manage live and persisted scoped models.", "info");
				return;
			}

			if (sub === "discover") {
				const discoverArgs = tokens.slice(1);
				const discoverMode = discoverArgs[0] === "all" ? "all" : "configured";
				const explicitProviders = discoverArgs.length > 0 && discoverArgs[0] !== "all" && discoverArgs[0] !== "configured"
					? discoverArgs
					: [];
				try {
					const summary = await discoverAndSyncModels(ctx, discoverMode, explicitProviders);
					notify(ctx, summary, "success");
				} catch (error) {
					notify(ctx, `AOC model discovery failed: ${getErrorMessage(error)}`, "warning");
				}
				return;
			}

			notify(ctx, "Unknown subcommand. Use /aoc-models status, /aoc-models discover, or /scoped-models.", "warning");
		},
	});

	pi.registerCommand("aoc-model-mode", {
		description: "Deprecated. Use Pi built-in /scoped-models for scope management.",
		handler: async (_args, ctx) => {
			notify(ctx, "AOC starred/scoped model UI was removed. Use Pi built-in /scoped-models.", "info");
			updateStatus(ctx);
		},
	});

	pi.registerCommand("aoc-model-status", {
		description: "Show AOC model ops status.",
		handler: async (_args, ctx) => {
			const openrouterModels = readJsonFile<MultiAuthModelsFile>(getAgentModelsPath(), { providers: {} }).providers?.openrouter?.models?.length ?? 0;
			notify(ctx, `AOC Models: providers=openai-codex,opencode,openrouter | OpenRouter catalog=${openrouterModels}`, "info");
			updateStatus(ctx);
		},
	});
}
