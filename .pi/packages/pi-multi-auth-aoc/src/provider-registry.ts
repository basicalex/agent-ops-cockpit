import { readFile, stat } from "node:fs/promises";
import { getModels, type Api, type Model } from "@mariozechner/pi-ai";
import { getOAuthProvider, getOAuthProviders } from "./oauth-compat.js";
import { AuthWriter } from "./auth-writer.js";
import { resolveAgentRuntimePath } from "./runtime-paths.js";
import {
	LEGACY_SUPPORTED_PROVIDERS,
	type ProviderModelDefinition,
	type ProviderRegistrationMetadata,
	type SupportedProviderId,
} from "./types.js";

interface ProviderModelOverride {
	name?: string;
	api?: Api;
	reasoning?: boolean;
	input?: ("text" | "image")[];
	cost?: Partial<ProviderModelDefinition["cost"]>;
	contextWindow?: number;
	maxTokens?: number;
	headers?: Record<string, string>;
	compat?: Record<string, unknown>;
}

interface ModelsProviderEntry {
	api?: Api;
	baseUrl?: string;
	apiKey?: string;
	authHeader?: boolean;
	headers?: Record<string, string>;
	compat?: Record<string, unknown>;
	modelOverrides?: Record<string, ProviderModelOverride>;
	models: ProviderModelDefinition[];
}

interface ModelsFileData {
	providers: Record<string, ModelsProviderEntry>;
}

interface ModelsFileCacheEntry {
	cacheKey: string;
	data: ModelsFileData;
}

export interface ProviderCapabilities {
	provider: SupportedProviderId;
	supportsApiKey: boolean;
	supportsOAuth: boolean;
}

export interface AvailableOAuthProvider {
	provider: SupportedProviderId;
	name: string;
}

const EMPTY_MODELS_FILE: ModelsFileData = {
	providers: {},
};

const DEFAULT_OPENROUTER_BASE_URL = "https://openrouter.ai/api/v1";
const DEFAULT_OPENROUTER_HEADERS = {
	"HTTP-Referer": "https://github.com/ceii/agent-ops-cockpit",
	"X-Title": "AOC",
};
const LEGACY_OPENROUTER_KEY_HELPER = "openrouter-key-from-multi-auth";

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function toNumberOrDefault(value: unknown, fallback: number): number {
	if (typeof value === "number" && Number.isFinite(value) && value > 0) {
		return value;
	}
	return fallback;
}

function toBooleanOrDefault(value: unknown, fallback: boolean): boolean {
	if (typeof value === "boolean") {
		return value;
	}
	return fallback;
}

function normalizeModelId(provider: string, value: string): string {
	const normalized = value.trim();
	const prefix = `${provider}/`;
	return normalized.startsWith(prefix) ? normalized.slice(prefix.length) : normalized;
}

function toInputList(value: unknown): ("text" | "image")[] {
	if (!Array.isArray(value)) {
		return ["text"];
	}

	const parsed = value
		.filter((item): item is "text" | "image" => item === "text" || item === "image")
		.slice(0, 2);

	return parsed.length > 0 ? parsed : ["text"];
}

function toCost(value: unknown): ProviderModelDefinition["cost"] {
	if (!isRecord(value)) {
		return {
			input: 0,
			output: 0,
			cacheRead: 0,
			cacheWrite: 0,
		};
	}

	return {
		input: typeof value.input === "number" ? value.input : 0,
		output: typeof value.output === "number" ? value.output : 0,
		cacheRead: typeof value.cacheRead === "number" ? value.cacheRead : 0,
		cacheWrite: typeof value.cacheWrite === "number" ? value.cacheWrite : 0,
	};
}

function normalizeStringMap(value: unknown): Record<string, string> | undefined {
	if (!isRecord(value)) {
		return undefined;
	}

	const entries = Object.entries(value)
		.filter((entry): entry is [string, string] => typeof entry[1] === "string")
		.map(([key, item]) => [key.trim(), item.trim()] as const)
		.filter(([key, item]) => key.length > 0 && item.length > 0);

	return entries.length > 0 ? Object.fromEntries(entries) : undefined;
}

function normalizeUnknownMap(value: unknown): Record<string, unknown> | undefined {
	if (!isRecord(value)) {
		return undefined;
	}
	return { ...value };
}

function mergeHeaders(
	providerHeaders: Record<string, string> | undefined,
	modelHeaders: Record<string, string> | undefined,
): Record<string, string> | undefined {
	if (!providerHeaders && !modelHeaders) {
		return undefined;
	}
	return {
		...(providerHeaders ?? {}),
		...(modelHeaders ?? {}),
	};
}

function mergeCompat(
	providerCompat: Record<string, unknown> | undefined,
	modelCompat: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined {
	if (!providerCompat && !modelCompat) {
		return undefined;
	}
	return {
		...(providerCompat ?? {}),
		...(modelCompat ?? {}),
	};
}

function mergeCost(
	base: ProviderModelDefinition["cost"],
	override: Partial<ProviderModelDefinition["cost"]> | undefined,
): ProviderModelDefinition["cost"] {
	if (!override) {
		return { ...base };
	}
	return {
		input: typeof override.input === "number" ? override.input : base.input,
		output: typeof override.output === "number" ? override.output : base.output,
		cacheRead: typeof override.cacheRead === "number" ? override.cacheRead : base.cacheRead,
		cacheWrite: typeof override.cacheWrite === "number" ? override.cacheWrite : base.cacheWrite,
	};
}

function normalizeModelRecord(
	model: unknown,
	providerId: string,
	providerApi: Api | undefined,
	providerHeaders: Record<string, string> | undefined,
	providerCompat: Record<string, unknown> | undefined,
): ProviderModelDefinition | null {
	if (!isRecord(model) || typeof model.id !== "string" || !model.id.trim()) {
		return null;
	}

	const modelId = normalizeModelId(providerId, model.id);
	const modelHeaders = normalizeStringMap(model.headers);
	const modelCompat = normalizeUnknownMap(model.compat);

	return {
		id: modelId,
		name: typeof model.name === "string" && model.name.trim() ? model.name.trim() : modelId,
		api: typeof model.api === "string" && model.api.trim() ? (model.api.trim() as Api) : providerApi,
		reasoning: toBooleanOrDefault(model.reasoning, false),
		input: toInputList(model.input),
		cost: toCost(model.cost),
		contextWindow: toNumberOrDefault(model.contextWindow, 128_000),
		maxTokens: toNumberOrDefault(model.maxTokens, 8_192),
		headers: mergeHeaders(providerHeaders, modelHeaders),
		compat: mergeCompat(providerCompat, modelCompat),
	};
}

function normalizeModelOverride(
	providerId: string,
	value: unknown,
): ProviderModelOverride | null {
	if (!isRecord(value)) {
		return null;
	}

	return {
		name: typeof value.name === "string" && value.name.trim() ? value.name.trim() : undefined,
		api: typeof value.api === "string" && value.api.trim() ? (value.api.trim() as Api) : undefined,
		reasoning: typeof value.reasoning === "boolean" ? value.reasoning : undefined,
		input: Array.isArray(value.input) ? toInputList(value.input) : undefined,
		cost: isRecord(value.cost) ? value.cost as Partial<ProviderModelDefinition["cost"]> : undefined,
		contextWindow: typeof value.contextWindow === "number" ? value.contextWindow : undefined,
		maxTokens: typeof value.maxTokens === "number" ? value.maxTokens : undefined,
		headers: normalizeStringMap(value.headers),
		compat: normalizeUnknownMap(value.compat),
	};
}

function normalizeModelOverrides(
	providerId: string,
	value: unknown,
): Record<string, ProviderModelOverride> | undefined {
	if (!isRecord(value)) {
		return undefined;
	}

	const entries = Object.entries(value)
		.map(([modelId, override]) => [normalizeModelId(providerId, modelId), normalizeModelOverride(providerId, override)] as const)
		.filter((entry): entry is readonly [string, ProviderModelOverride] => Boolean(entry[0]) && entry[1] !== null);

	return entries.length > 0 ? Object.fromEntries(entries) : undefined;
}

function mapBuiltInModel(providerId: string, model: Model<Api>): ProviderModelDefinition {
	const compat = isRecord((model as { compat?: unknown }).compat)
		? { ...((model as { compat?: Record<string, unknown> }).compat ?? {}) }
		: undefined;

	return {
		id: normalizeModelId(providerId, model.id),
		name: model.name,
		api: model.api,
		reasoning: model.reasoning,
		input: [...model.input],
		cost: {
			input: model.cost.input,
			output: model.cost.output,
			cacheRead: model.cost.cacheRead,
			cacheWrite: model.cost.cacheWrite,
		},
		contextWindow: model.contextWindow,
		maxTokens: model.maxTokens,
		headers: model.headers ? { ...model.headers } : undefined,
		compat,
	};
}

function applyModelOverride(
	model: ProviderModelDefinition,
	override: ProviderModelOverride | undefined,
): ProviderModelDefinition {
	if (!override) {
		return model;
	}

	return {
		...model,
		name: override.name ?? model.name,
		api: override.api ?? model.api,
		reasoning: override.reasoning ?? model.reasoning,
		input: override.input ?? model.input,
		cost: mergeCost(model.cost, override.cost),
		contextWindow: override.contextWindow ?? model.contextWindow,
		maxTokens: override.maxTokens ?? model.maxTokens,
		headers: mergeHeaders(model.headers, override.headers),
		compat: mergeCompat(model.compat, override.compat),
	};
}

function applyProviderDefaults(
	model: ProviderModelDefinition,
	entry: ModelsProviderEntry | undefined,
): ProviderModelDefinition {
	if (!entry) {
		return model;
	}
	return {
		...model,
		api: model.api ?? entry.api,
		headers: mergeHeaders(entry.headers, model.headers),
		compat: mergeCompat(entry.compat, model.compat),
	};
}

function recordsEqual(a: Record<string, string> | undefined, b: Record<string, string> | undefined): boolean {
	if (!a && !b) return true;
	if (!a || !b) return false;
	const aKeys = Object.keys(a).sort();
	const bKeys = Object.keys(b).sort();
	if (aKeys.length !== bKeys.length) return false;
	return aKeys.every((key, index) => key === bKeys[index] && a[key] === b[key]);
}

function isLegacyManagedOpenRouterEntry(entry: ModelsProviderEntry | undefined): boolean {
	if (!entry) {
		return false;
	}
	const helperManaged = typeof entry.apiKey === "string" && entry.apiKey.includes(LEGACY_OPENROUTER_KEY_HELPER);
	const looksLikeManagedCatalog = entry.models.length >= 100;
	const headersMatchDefault = recordsEqual(entry.headers, DEFAULT_OPENROUTER_HEADERS);
	const hasExplicitCustomizations = Boolean(entry.compat)
		|| Boolean(entry.modelOverrides && Object.keys(entry.modelOverrides).length > 0);
	return helperManaged || (!hasExplicitCustomizations && entry.authHeader === true && headersMatchDefault && looksLikeManagedCatalog);
}

function stripLegacyManagedOpenRouterEntry(entry: ModelsProviderEntry): ModelsProviderEntry {
	const next: ModelsProviderEntry = {
		models: [],
	};

	if (entry.baseUrl && entry.baseUrl !== DEFAULT_OPENROUTER_BASE_URL) {
		next.baseUrl = entry.baseUrl;
	}
	if (entry.api && entry.api !== "openai-completions") {
		next.api = entry.api;
	}
	if (entry.headers && !recordsEqual(entry.headers, DEFAULT_OPENROUTER_HEADERS)) {
		next.headers = { ...entry.headers };
	}
	if (entry.compat) {
		next.compat = { ...entry.compat };
	}
	if (entry.modelOverrides && Object.keys(entry.modelOverrides).length > 0) {
		next.modelOverrides = { ...entry.modelOverrides };
	}
	return next;
}

function normalizeModelsFileData(parsed: unknown): ModelsFileData {
	if (!isRecord(parsed) || !isRecord(parsed.providers)) {
		return EMPTY_MODELS_FILE;
	}

	const providers: Record<string, ModelsProviderEntry> = {};
	for (const [providerId, rawProvider] of Object.entries(parsed.providers)) {
		if (!isRecord(rawProvider)) {
			continue;
		}

		const api = typeof rawProvider.api === "string" && rawProvider.api.trim()
			? (rawProvider.api.trim() as Api)
			: undefined;
		const baseUrl = typeof rawProvider.baseUrl === "string" && rawProvider.baseUrl.trim()
			? rawProvider.baseUrl.trim()
			: undefined;
		const headers = normalizeStringMap(rawProvider.headers);
		const compat = normalizeUnknownMap(rawProvider.compat);
		const modelOverrides = normalizeModelOverrides(providerId, rawProvider.modelOverrides);
		const models = Array.isArray(rawProvider.models)
			? rawProvider.models
				.map((model) => normalizeModelRecord(model, providerId, api, headers, compat))
				.filter((model): model is ProviderModelDefinition => model !== null)
			: [];

		const entry: ModelsProviderEntry = {
			api,
			baseUrl,
			apiKey: typeof rawProvider.apiKey === "string" ? rawProvider.apiKey.trim() : undefined,
			authHeader: typeof rawProvider.authHeader === "boolean" ? rawProvider.authHeader : undefined,
			headers,
			compat,
			modelOverrides,
			models,
		};

		if (
			entry.api
			|| entry.baseUrl
			|| entry.apiKey
			|| typeof entry.authHeader === "boolean"
			|| entry.headers
			|| entry.compat
			|| entry.modelOverrides
			|| entry.models.length > 0
		) {
			providers[providerId] = entry;
		}
	}

	return { providers };
}

function getDefaultModelsPath(): string {
	return resolveAgentRuntimePath("models.json");
}

function createModelsFileCacheKey(fileStats: {
	mtimeMs: number;
	ctimeMs: number;
	size: number;
}): string {
	return `${fileStats.mtimeMs}:${fileStats.ctimeMs}:${fileStats.size}`;
}

function isMissingFileError(error: unknown): boolean {
	return (
		error instanceof Error &&
		"code" in error &&
		typeof error.code === "string" &&
		error.code === "ENOENT"
	);
}

function mergeBuiltInModels(
	provider: SupportedProviderId,
	builtInModels: Model<Api>[],
	entry: ModelsProviderEntry | undefined,
	primaryApi: Api,
): ProviderModelDefinition[] {
	const merged = new Map<string, ProviderModelDefinition>();

	for (const builtInModel of builtInModels) {
		const normalized = applyProviderDefaults(mapBuiltInModel(provider, builtInModel), entry);
		const override = entry?.modelOverrides?.[normalized.id];
		merged.set(normalized.id, applyModelOverride(normalized, override));
	}

	for (const customModel of entry?.models ?? []) {
		const normalized = applyProviderDefaults(
			{
				...customModel,
				api: customModel.api ?? primaryApi,
			},
			entry,
		);
		merged.set(normalized.id, normalized);
	}

	return [...merged.values()].sort((a, b) => a.id.localeCompare(b.id));
}

export class ProviderRegistry {
	private modelsFileCache: ModelsFileCacheEntry | null = null;
	private modelsFileLoadPromise: Promise<ModelsFileData> | null = null;

	constructor(
		private readonly authWriter: AuthWriter = new AuthWriter(),
		private readonly modelsPath: string = getDefaultModelsPath(),
		private readonly legacyProviders: readonly string[] = LEGACY_SUPPORTED_PROVIDERS,
	) {}

	async discoverProviderIds(): Promise<SupportedProviderId[]> {
		const modelsFile = await this.readModelsFile();
		const seedProviders = [...this.legacyProviders, ...Object.keys(modelsFile.providers)];
		const authProviders = await this.authWriter.listProviderIds(seedProviders);

		const ordered: string[] = [];
		const seenProviders = new Set<string>();
		const pushUnique = (provider: string): void => {
			const normalized = provider.trim();
			if (!normalized || seenProviders.has(normalized)) {
				return;
			}
			seenProviders.add(normalized);
			ordered.push(normalized);
		};

		for (const provider of this.legacyProviders) {
			pushUnique(provider);
		}
		for (const provider of Object.keys(modelsFile.providers)) {
			pushUnique(provider);
		}
		for (const provider of authProviders) {
			pushUnique(provider);
		}

		return ordered;
	}

	getProviderCapabilities(provider: SupportedProviderId): ProviderCapabilities {
		return {
			provider,
			supportsApiKey: true,
			supportsOAuth: Boolean(
				getOAuthProvider(provider as Parameters<typeof getOAuthProvider>[0]),
			),
		};
	}

	listAvailableOAuthProviders(): AvailableOAuthProvider[] {
		const seenProviders = new Set<SupportedProviderId>();
		const providers: AvailableOAuthProvider[] = [];
		for (const provider of getOAuthProviders()) {
			const providerId = provider.id.trim();
			if (!providerId || seenProviders.has(providerId)) {
				continue;
			}
			seenProviders.add(providerId);
			providers.push({
				provider: providerId,
				name: provider.name.trim() || providerId,
			});
		}
		return providers;
	}

	/**
	 * Returns true when provider has model metadata from built-in registry or models.json.
	 */
	async hasModelMetadata(provider: SupportedProviderId): Promise<boolean> {
		const builtInModels = getModels(provider as Parameters<typeof getModels>[0]);
		if (builtInModels.length > 0) {
			return true;
		}

		const modelsFile = await this.readModelsFile();
		return Boolean(modelsFile.providers[provider]?.models.length);
	}

	/**
	 * Returns true for providers that only have OAuth credentials but no model metadata,
	 * such as integrations used by non-chat features.
	 */
	async isCredentialOnlyOAuthProvider(provider: SupportedProviderId): Promise<boolean> {
		const hasMetadata = await this.hasModelMetadata(provider);
		if (hasMetadata) {
			return false;
		}

		const supportsOAuth = Boolean(
			getOAuthProvider(provider as Parameters<typeof getOAuthProvider>[0]),
		);
		if (supportsOAuth) {
			return true;
		}

		const credentialIds = await this.authWriter.listProviderCredentialIds(provider);
		for (const credentialId of credentialIds) {
			const credential = await this.authWriter.getCredential(credentialId);
			if (credential?.type === "oauth") {
				return true;
			}
		}

		return false;
	}

	async resolveProviderRegistrationMetadata(
		provider: SupportedProviderId,
	): Promise<ProviderRegistrationMetadata | null> {
		const modelsFile = await this.readModelsFile();
		let fromFile = modelsFile.providers[provider];
		if (provider === "openrouter" && isLegacyManagedOpenRouterEntry(fromFile)) {
			fromFile = stripLegacyManagedOpenRouterEntry(fromFile!);
		}

		const builtInModels = getModels(provider as Parameters<typeof getModels>[0]);
		if (builtInModels.length > 0) {
			const firstModel = builtInModels[0];
			const api = fromFile?.api ?? firstModel.api;
			const baseUrl = fromFile?.baseUrl ?? firstModel.baseUrl;
			if (!baseUrl) {
				return null;
			}

			const models = mergeBuiltInModels(provider, builtInModels, fromFile, api);
			const apis = [...new Set(models.map((model) => model.api ?? api))];
			return {
				provider,
				api,
				apis,
				baseUrl,
				models,
			};
		}

		if (fromFile && fromFile.models.length > 0 && fromFile.api && fromFile.baseUrl) {
			const apis = [...new Set(fromFile.models.map((model) => model.api ?? fromFile!.api))];
			return {
				provider,
				api: fromFile.api,
				apis,
				baseUrl: fromFile.baseUrl,
				models: [...fromFile.models],
			};
		}

		return null;
	}

	private async readModelsFile(): Promise<ModelsFileData> {
		if (this.modelsFileLoadPromise) {
			return this.modelsFileLoadPromise;
		}

		const loadPromise = this.loadModelsFile();
		const wrappedPromise = loadPromise.finally(() => {
			if (this.modelsFileLoadPromise === wrappedPromise) {
				this.modelsFileLoadPromise = null;
			}
		});
		this.modelsFileLoadPromise = wrappedPromise;
		return wrappedPromise;
	}

	private async loadModelsFile(): Promise<ModelsFileData> {
		let fileStats: Awaited<ReturnType<typeof stat>>;
		try {
			fileStats = await stat(this.modelsPath);
		} catch (error) {
			if (!isMissingFileError(error)) {
				this.modelsFileCache = null;
			}
			return EMPTY_MODELS_FILE;
		}

		const cacheKey = createModelsFileCacheKey(fileStats);
		if (this.modelsFileCache?.cacheKey === cacheKey) {
			return this.modelsFileCache.data;
		}

		let parsed: unknown;
		try {
			const content = await readFile(this.modelsPath, "utf-8");
			parsed = JSON.parse(content);
		} catch {
			const empty = EMPTY_MODELS_FILE;
			this.modelsFileCache = { cacheKey, data: empty };
			return empty;
		}

		const data = normalizeModelsFileData(parsed);
		this.modelsFileCache = { cacheKey, data };
		return data;
	}
}
