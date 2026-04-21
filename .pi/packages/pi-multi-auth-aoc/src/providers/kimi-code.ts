import { randomBytes } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import os from "node:os";
import { dirname, join } from "node:path";
import type {
	Api,
	AssistantMessageEvent,
	CacheRetention,
	Context,
	Model,
	OAuthCredentials,
	OAuthLoginCallbacks,
	OAuthProviderInterface,
	SimpleStreamOptions,
	ThinkingLevel,
} from "@mariozechner/pi-ai";
import {
	AssistantMessageEventStream,
	streamSimpleAnthropic,
	streamSimpleOpenAICompletions,
} from "@mariozechner/pi-ai";
import { AuthStorage } from "@mariozechner/pi-coding-agent";
import type { ProviderRegistrationMetadata } from "../types.js";

export const KIMI_CODING_PROVIDER_ID = "kimi-coding";
export const KIMI_CODING_MODEL_ID = "kimi-for-coding";

const KIMI_CLIENT_ID = "17e5f671-d194-4dfb-9706-5516cb48c098";
const KIMI_DEFAULT_OAUTH_HOST = "https://auth.kimi.com";
const KIMI_CLI_VERSION = "1.30.0";
const KIMI_CLI_USER_AGENT = `KimiCLI/${KIMI_CLI_VERSION}`;
const KIMI_PLATFORM = "kimi_cli";
const KIMI_DEFAULT_INLINE_UPLOAD_THRESHOLD_BYTES = 1 * 1024 * 1024;
const KIMI_EMPTY_RESPONSE_PREFIX = "(Empty response:";
const KIMI_DEVICE_ID_PATH = join(
	os.homedir(),
	".pi",
	"providers",
	KIMI_CODING_PROVIDER_ID,
	"device_id",
);

type KimiApi = "anthropic-messages" | "openai-completions";
type JsonRecord = Record<string, unknown>;
type Uploader = (mimeType: string, data: string) => Promise<string | null>;

interface DeviceAuthorization {
	user_code: string;
	device_code: string;
	verification_uri: string;
	verification_uri_complete: string;
	expires_in: number;
	interval: number;
}

interface TokenResponse {
	access_token: string;
	refresh_token: string;
	expires_in: number;
	scope?: string;
	token_type?: string;
}

interface KimiEnvOverrides {
	temperature?: number;
	topP?: number;
	maxTokens?: number;
}

interface KimiPayloadContext {
	api: KimiApi;
	upload?: Uploader;
	cacheKey?: string;
	cacheRetention: CacheRetention;
	reasoning?: ThinkingLevel;
	envOverrides: KimiEnvOverrides;
}

function getErrorMessage(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

function isRecord(value: unknown): value is JsonRecord {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function createDeviceId(): string {
	return randomBytes(16).toString("hex");
}

function ensurePrivateFile(path: string): void {
	try {
		chmodSync(path, 0o600);
	} catch {
		// Ignore chmod failures on unsupported filesystems.
	}
}

function readPersistedDeviceId(): string | null {
	try {
		if (!existsSync(KIMI_DEVICE_ID_PATH)) {
			return null;
		}
		const value = readFileSync(KIMI_DEVICE_ID_PATH, "utf8").trim();
		return value || null;
	} catch {
		return null;
	}
}

function persistDeviceId(deviceId: string): void {
	try {
		mkdirSync(dirname(KIMI_DEVICE_ID_PATH), { recursive: true });
		writeFileSync(KIMI_DEVICE_ID_PATH, deviceId, "utf8");
		ensurePrivateFile(KIMI_DEVICE_ID_PATH);
	} catch {
		// Ignore persistence failures and fall back to process-local device id.
	}
}

let cachedDeviceId: string | null = null;

function getStableDeviceId(): string {
	if (cachedDeviceId) {
		return cachedDeviceId;
	}

	const persisted = readPersistedDeviceId();
	if (persisted) {
		cachedDeviceId = persisted;
		return cachedDeviceId;
	}

	cachedDeviceId = createDeviceId();
	persistDeviceId(cachedDeviceId);
	return cachedDeviceId;
}

function asciiHeaderValue(value: string, fallback = "unknown"): string {
	const trimmed = value.trim();
	/* oxlint-disable-next-line no-control-regex */
	if (/^[\x00-\x7F]*$/.test(trimmed)) {
		return trimmed;
	}
	/* oxlint-disable-next-line no-control-regex */
	const sanitized = trimmed.replace(/[^\x00-\x7F]/g, "").trim();
	return sanitized || fallback;
}

function getDeviceModel(): string {
	const platform = process.platform;
	const arch = os.machine() || process.arch;
	const release = os.release();
	if (platform === "darwin") {
		return `macOS ${release} ${arch}`;
	}
	if (platform === "win32") {
		return `Windows ${release} ${arch}`;
	}
	return `${platform} ${release} ${arch}`;
}

export function getKimiOAuthHost(): string {
	const value = process.env.KIMI_CODE_OAUTH_HOST || process.env.KIMI_OAUTH_HOST;
	const normalized = typeof value === "string" ? value.trim() : "";
	return normalized || KIMI_DEFAULT_OAUTH_HOST;
}

export function getKimiProtocol(): KimiApi {
	return process.env.KIMI_CODE_PROTOCOL === "openai"
		? "openai-completions"
		: "anthropic-messages";
}

export function getKimiBaseUrl(): string {
	const protocol = getKimiProtocol();
	const fallback =
		protocol === "openai-completions"
			? "https://api.kimi.com/coding/v1"
			: "https://api.kimi.com/coding";
	const value = process.env.KIMI_CODE_BASE_URL;
	const normalized = typeof value === "string" ? value.trim() : "";
	return normalized || fallback;
}

export function getKimiCommonHeaders(): Record<string, string> {
	const headers = {
		"User-Agent": KIMI_CLI_USER_AGENT,
		"X-Msh-Platform": KIMI_PLATFORM,
		"X-Msh-Version": KIMI_CLI_VERSION,
		"X-Msh-Device-Name": os.hostname(),
		"X-Msh-Device-Model": getDeviceModel(),
		"X-Msh-Os-Version": os.release(),
		"X-Msh-Device-Id": getStableDeviceId(),
	};
	return Object.fromEntries(
		Object.entries(headers).map(([key, value]) => [key, asciiHeaderValue(value)]),
	) as Record<string, string>;
}

function getKimiMetadataModels(): ProviderRegistrationMetadata["models"] {
	return [
		{
			id: KIMI_CODING_MODEL_ID,
			name: "Kimi for Coding",
			api: getKimiProtocol(),
			reasoning: true,
			input: ["text", "image"],
			cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
			contextWindow: 262_144,
			maxTokens: 32_000,
			headers: getKimiCommonHeaders(),
			compat:
				getKimiProtocol() === "openai-completions"
					? {
						supportsDeveloperRole: false,
						supportsReasoningEffort: true,
						reasoningEffortMap: {
							minimal: "low",
							low: "low",
							medium: "medium",
							high: "high",
							xhigh: "high",
						},
						maxTokensField: "max_tokens",
					}
					: undefined,
		},
	];
}

export function getKimiProviderRegistrationMetadata(): ProviderRegistrationMetadata {
	const api = getKimiProtocol();
	return {
		provider: KIMI_CODING_PROVIDER_ID,
		api,
		apis: [api],
		baseUrl: getKimiBaseUrl(),
		models: getKimiMetadataModels(),
	};
}

async function requestDeviceAuthorization(): Promise<DeviceAuthorization> {
	const response = await fetch(`${getKimiOAuthHost()}/api/oauth/device_authorization`, {
		method: "POST",
		headers: {
			"Content-Type": "application/x-www-form-urlencoded",
			...getKimiCommonHeaders(),
		},
		body: new URLSearchParams({ client_id: KIMI_CLIENT_ID }),
	});

	if (!response.ok) {
		const text = await response.text().catch(() => "");
		throw new Error(`Device authorization failed: ${response.status} ${text}`);
	}

	const data = (await response.json()) as Partial<DeviceAuthorization>;
	if (!data.user_code || !data.device_code || !data.verification_uri_complete) {
		throw new Error("Invalid Kimi device authorization response");
	}

	return {
		user_code: data.user_code,
		device_code: data.device_code,
		verification_uri: data.verification_uri || data.verification_uri_complete,
		verification_uri_complete: data.verification_uri_complete,
		expires_in: data.expires_in || 1_800,
		interval: data.interval || 5,
	};
}

async function requestDeviceToken(auth: DeviceAuthorization): Promise<TokenResponse | null> {
	const response = await fetch(`${getKimiOAuthHost()}/api/oauth/token`, {
		method: "POST",
		headers: {
			"Content-Type": "application/x-www-form-urlencoded",
			...getKimiCommonHeaders(),
		},
		body: new URLSearchParams({
			client_id: KIMI_CLIENT_ID,
			device_code: auth.device_code,
			grant_type: "urn:ietf:params:oauth:grant-type:device_code",
		}),
	});

	if (response.status === 200) {
		const data = (await response.json()) as TokenResponse;
		if (data.access_token && data.refresh_token) {
			return data;
		}
		throw new Error("Kimi token response missing required fields");
	}

	if (response.status === 400) {
		const data = (await response.json()) as { error?: string; error_description?: string };
		if (data.error === "authorization_pending") {
			return null;
		}
		if (data.error === "expired_token") {
			throw new Error("expired_token");
		}
		throw new Error(data.error_description || data.error || "Kimi token request failed");
	}

	const text = await response.text().catch(() => "");
	throw new Error(`Kimi token request failed: ${response.status} ${text}`);
}

async function refreshKimiTokenResponse(refreshToken: string): Promise<TokenResponse> {
	const response = await fetch(`${getKimiOAuthHost()}/api/oauth/token`, {
		method: "POST",
		headers: {
			"Content-Type": "application/x-www-form-urlencoded",
			...getKimiCommonHeaders(),
		},
		body: new URLSearchParams({
			client_id: KIMI_CLIENT_ID,
			grant_type: "refresh_token",
			refresh_token: refreshToken,
		}),
	});

	if (!response.ok) {
		const text = await response.text().catch(() => "");
		throw new Error(`Kimi token refresh failed: ${response.status} ${text}`);
	}

	const data = (await response.json()) as TokenResponse;
	if (!data.access_token || !data.refresh_token) {
		throw new Error("Kimi token refresh response missing required fields");
	}
	return data;
}

async function loginKimiCode(callbacks: OAuthLoginCallbacks): Promise<OAuthCredentials> {
	while (true) {
		const auth = await requestDeviceAuthorization();
		callbacks.onAuth({
			url: auth.verification_uri_complete,
			instructions: `Please visit the URL to authorize. Your code: ${auth.user_code}`,
		});

		const intervalMs = Math.max(auth.interval, 1) * 1_000;
		const expiresAt = Date.now() + auth.expires_in * 1_000;
		let token: TokenResponse | null = null;
		let printedWaiting = false;

		while (Date.now() < expiresAt) {
			if (callbacks.signal?.aborted) {
				throw new Error("Authorization aborted");
			}

			try {
				token = await requestDeviceToken(auth);
				if (token) {
					break;
				}
			} catch (error) {
				if (error instanceof Error && error.message === "expired_token") {
					callbacks.onProgress?.("Device code expired, restarting...");
					break;
				}
				throw error;
			}

			if (!printedWaiting) {
				callbacks.onProgress?.("Waiting for Kimi authorization...");
				printedWaiting = true;
			}
			await new Promise((resolve) => setTimeout(resolve, intervalMs));
		}

		if (token) {
			return {
				access: token.access_token,
				refresh: token.refresh_token,
				expires: Date.now() + token.expires_in * 1_000,
			};
		}
	}
}

async function refreshKimiCodeToken(credentials: OAuthCredentials): Promise<OAuthCredentials> {
	const token = await refreshKimiTokenResponse(credentials.refresh);
	return {
		access: token.access_token,
		refresh: token.refresh_token,
		expires: Date.now() + token.expires_in * 1_000,
	};
}

export function getKimiOAuthProvider(): OAuthProviderInterface {
	return {
		id: KIMI_CODING_PROVIDER_ID,
		name: "Kimi Code",
		login: loginKimiCode,
		refreshToken: refreshKimiCodeToken,
		getApiKey: (credentials) => credentials.access,
	};
}

function resolveCacheRetention(value?: CacheRetention): CacheRetention {
	if (value === "none" || value === "short" || value === "long") {
		return value;
	}
	return process.env.PI_CACHE_RETENTION === "long" ? "long" : "short";
}

function mapThinkingLevel(level?: string): { effort: string | null; enabled: boolean } | undefined {
	if (!level || level === "none" || level === "off") {
		return { effort: null, enabled: false };
	}
	if (level === "minimal" || level === "low") {
		return { effort: "low", enabled: true };
	}
	if (level === "medium") {
		return { effort: "medium", enabled: true };
	}
	if (level === "high" || level === "xhigh") {
		return { effort: "high", enabled: true };
	}
	return undefined;
}

function parseInlineUploadThreshold(raw: string | undefined): number {
	const parsed = Number.parseInt(raw ?? "", 10);
	return Number.isFinite(parsed) && parsed >= 0
		? parsed
		: KIMI_DEFAULT_INLINE_UPLOAD_THRESHOLD_BYTES;
}

function deriveFilesBaseUrl(baseUrl: string): string {
	const trimmed = baseUrl.replace(/\/$/, "");
	return trimmed.endsWith("/v1") ? trimmed : `${trimmed}/v1`;
}

function parseDataUrl(url: string): { mimeType: string; data: string } | null {
	const match = url.match(/^data:([^;,]+);base64,([A-Za-z0-9+/=]+)$/);
	return match ? { mimeType: match[1], data: match[2] } : null;
}

function getUploadFilename(mimeType: string): string {
	const names: Record<string, string> = {
		"image/jpeg": "upload.jpg",
		"image/png": "upload.png",
		"image/gif": "upload.gif",
		"image/webp": "upload.webp",
		"video/mp4": "upload.mp4",
		"video/quicktime": "upload.mov",
	};
	return names[mimeType] ?? (mimeType.startsWith("video/") ? "upload.mp4" : "upload.bin");
}

function readEnvOverrides(): KimiEnvOverrides {
	const overrides: KimiEnvOverrides = {};
	const temperature = process.env.KIMI_MODEL_TEMPERATURE;
	const topP = process.env.KIMI_MODEL_TOP_P;
	const maxTokens = process.env.KIMI_MODEL_MAX_TOKENS;
	if (temperature) {
		overrides.temperature = Number.parseFloat(temperature);
	}
	if (topP) {
		overrides.topP = Number.parseFloat(topP);
	}
	if (maxTokens) {
		overrides.maxTokens = Number.parseInt(maxTokens, 10);
	}
	return overrides;
}

async function uploadKimiFile(apiKey: string, mimeType: string, data: string): Promise<string | null> {
	const buffer = Buffer.from(data, "base64");
	const isVideo = mimeType.startsWith("video/");
	if (!isVideo && buffer.length <= parseInlineUploadThreshold(process.env.KIMI_CODE_UPLOAD_THRESHOLD_BYTES)) {
		return null;
	}

	const formData = new FormData();
	formData.append("file", new Blob([buffer], { type: mimeType }), getUploadFilename(mimeType));
	formData.append("purpose", isVideo ? "video" : "image");

	const response = await fetch(`${deriveFilesBaseUrl(getKimiBaseUrl())}/files`, {
		method: "POST",
		headers: {
			Authorization: `Bearer ${apiKey}`,
			...getKimiCommonHeaders(),
		},
		body: formData,
	});
	if (!response.ok) {
		const text = await response.text().catch(() => "");
		throw new Error(`Kimi upload failed: ${response.status} ${text}`);
	}

	const fileObject = (await response.json()) as { id?: string };
	return fileObject.id ? `ms://${fileObject.id}` : null;
}

async function transformOpenAIPayloadFiles(payload: JsonRecord, upload: Uploader): Promise<void> {
	if (!Array.isArray(payload.messages)) {
		return;
	}
	const cache = new Map<string, string>();

	for (const message of payload.messages) {
		if (!isRecord(message) || !Array.isArray(message.content)) {
			continue;
		}
		for (const block of message.content) {
			if (!isRecord(block)) {
				continue;
			}
			const key = block.type === "image_url" ? "image_url" : block.type === "video_url" ? "video_url" : null;
			if (!key) {
				continue;
			}
			const field = block[key];
			const urlValue =
				typeof field === "string"
					? field
					: isRecord(field) && typeof field.url === "string"
						? field.url
						: null;
			if (!urlValue || urlValue.startsWith("ms://")) {
				continue;
			}
			const parsed = parseDataUrl(urlValue);
			if (!parsed) {
				continue;
			}
			const uploaded = cache.get(urlValue) ?? (await upload(parsed.mimeType, parsed.data));
			if (!uploaded) {
				continue;
			}
			cache.set(urlValue, uploaded);
			block[key] = typeof field === "string" ? uploaded : { ...(field as JsonRecord), url: uploaded };
		}
	}
}

async function transformAnthropicPayloadFiles(payload: JsonRecord, upload: Uploader): Promise<void> {
	if (!Array.isArray(payload.messages)) {
		return;
	}
	const cache = new Map<string, string>();

	const transformImageBlock = async (block: unknown): Promise<unknown> => {
		if (!isRecord(block) || block.type !== "image") {
			return block;
		}
		const source = block.source;
		if (!isRecord(source) || source.type !== "base64") {
			return block;
		}
		const mediaType = source.media_type;
		const data = source.data;
		if (typeof mediaType !== "string" || typeof data !== "string") {
			return block;
		}
		const cacheKey = `${mediaType}:${data}`;
		const uploaded = cache.get(cacheKey) ?? (await upload(mediaType, data));
		if (!uploaded) {
			return block;
		}
		cache.set(cacheKey, uploaded);
		const next: JsonRecord = { type: "image", source: { type: "url", url: uploaded } };
		if (block.cache_control !== undefined) {
			next.cache_control = block.cache_control;
		}
		return next;
	};

	for (const message of payload.messages) {
		if (!isRecord(message) || !Array.isArray(message.content)) {
			continue;
		}
		for (let index = 0; index < message.content.length; index += 1) {
			const block = message.content[index];
			if (isRecord(block) && block.type === "tool_result" && Array.isArray(block.content)) {
				for (let innerIndex = 0; innerIndex < block.content.length; innerIndex += 1) {
					block.content[innerIndex] = await transformImageBlock(block.content[innerIndex]);
				}
				continue;
			}
			message.content[index] = await transformImageBlock(block);
		}
	}
}

async function applyKimiPayloadMutations(payload: JsonRecord, context: KimiPayloadContext): Promise<void> {
	if (Array.isArray(payload.messages)) {
		payload.messages = payload.messages.map((message) =>
			isRecord(message) && message.role === "developer" ? { ...message, role: "system" } : message,
		);
	}

	if (context.upload) {
		if (context.api === "openai-completions") {
			await transformOpenAIPayloadFiles(payload, context.upload);
		} else {
			await transformAnthropicPayloadFiles(payload, context.upload);
		}
	}

	if (context.cacheRetention !== "none") {
		const existing = payload.prompt_cache_key;
		const resolved = (typeof existing === "string" && existing) || context.cacheKey;
		if (resolved) {
			payload.prompt_cache_key = resolved;
		}
	}

	const { temperature, topP, maxTokens } = context.envOverrides;
	if (temperature !== undefined) {
		payload.temperature = temperature;
	}
	if (topP !== undefined) {
		payload.top_p = topP;
	}
	if (maxTokens !== undefined) {
		payload.max_tokens = maxTokens;
	}

	if (context.reasoning) {
		const mapped = mapThinkingLevel(context.reasoning);
		if (mapped) {
			payload.reasoning_effort = mapped.effort;
			const extraBody = isRecord(payload.extra_body) ? payload.extra_body : {};
			extraBody.thinking = { type: mapped.enabled ? "enabled" : "disabled" };
			payload.extra_body = extraBody;
		}
	}
}

async function* filterEmptyResponseStream(
	upstream: AsyncIterable<AssistantMessageEvent>,
): AsyncIterable<AssistantMessageEvent> {
	const suppressedIndices = new Set<number>();
	let textBuffer: AssistantMessageEvent[] = [];
	let bufferingIndex: number | null = null;

	for await (const event of upstream) {
		if (event.type === "text_start") {
			bufferingIndex = event.contentIndex;
			textBuffer = [event];
			continue;
		}

		if (bufferingIndex !== null && "contentIndex" in event && event.contentIndex === bufferingIndex) {
			if (event.type === "text_delta") {
				textBuffer.push(event);
				continue;
			}
			if (event.type === "text_end") {
				if (event.content.startsWith(KIMI_EMPTY_RESPONSE_PREFIX)) {
					suppressedIndices.add(bufferingIndex);
				} else {
					for (const buffered of textBuffer) {
						yield buffered;
					}
					yield event;
				}
				textBuffer = [];
				bufferingIndex = null;
				continue;
			}
		}

		if ("contentIndex" in event && suppressedIndices.has(event.contentIndex)) {
			continue;
		}

		if (event.type === "done" && suppressedIndices.size > 0) {
			event.message.content = event.message.content.filter(
				(block) => !(block.type === "text" && typeof block.text === "string" && block.text.startsWith(KIMI_EMPTY_RESPONSE_PREFIX)),
			);
		}

		yield event;
	}
}

async function refreshKimiAuthToken(currentKey: string): Promise<string | null> {
	try {
		const storage = AuthStorage.create();
		const credential = storage.get(KIMI_CODING_PROVIDER_ID);
		if (!credential || credential.type !== "oauth") {
			return null;
		}
		if (credential.access !== currentKey && Date.now() < credential.expires) {
			return credential.access;
		}
		const refreshed = await refreshKimiTokenResponse(credential.refresh);
		storage.set(KIMI_CODING_PROVIDER_ID, {
			type: "oauth",
			access: refreshed.access_token,
			refresh: refreshed.refresh_token,
			expires: Date.now() + refreshed.expires_in * 1_000,
		});
		return refreshed.access_token;
	} catch {
		return null;
	}
}

export function streamSimpleKimi(
	model: Model<Api>,
	context: Context,
	options?: SimpleStreamOptions,
): AssistantMessageEventStream {
	const filtered = new AssistantMessageEventStream();
	const initialKey = options?.apiKey || process.env.KIMI_API_KEY || "";
	const cacheKeyOverride = (options as (SimpleStreamOptions & { prompt_cache_key?: unknown }) | undefined)
		?.prompt_cache_key;
	const cacheKey = (typeof cacheKeyOverride === "string" && cacheKeyOverride) || options?.sessionId;
	const cacheRetention = resolveCacheRetention(options?.cacheRetention);
	const envOverrides = readEnvOverrides();
	const originalOnPayload = options?.onPayload;

	const buildPatchedOptions = (apiKey: string): SimpleStreamOptions => {
		const upload: Uploader | undefined = apiKey
			? (mimeType, data) => uploadKimiFile(apiKey, mimeType, data)
			: undefined;
		return {
			...options,
			apiKey,
			onPayload: async (payload, modelData) => {
				let nextPayload: unknown = payload;
				if (isRecord(nextPayload)) {
					await applyKimiPayloadMutations(nextPayload, {
						api: model.api as KimiApi,
						upload,
						cacheKey,
						cacheRetention,
						reasoning: options?.reasoning,
						envOverrides,
					});
				}
				if (originalOnPayload) {
					const result = await originalOnPayload(nextPayload, modelData);
					if (result !== undefined) {
						nextPayload = result;
					}
				}
				return nextPayload;
			},
		};
	};

	void (async () => {
		let attempt = 0;
		let currentKey = initialKey;

		while (true) {
			const patchedOptions = buildPatchedOptions(currentKey);
			const upstream =
				model.api === "openai-completions"
					? streamSimpleOpenAICompletions(model as Model<"openai-completions">, context, patchedOptions)
					: streamSimpleAnthropic(model as Model<"anthropic-messages">, context, patchedOptions);

			let pushedAny = false;
			let shouldRetry = false;

			try {
				for await (const event of filterEmptyResponseStream(upstream)) {
					if (!pushedAny && attempt === 0 && event.type === "error") {
						const refreshed = await refreshKimiAuthToken(currentKey);
						if (refreshed && refreshed !== currentKey) {
							currentKey = refreshed;
							shouldRetry = true;
							break;
						}
					}
					filtered.push(event);
					pushedAny = true;
				}
			} catch (error) {
				filtered.push({
					type: "error",
					reason: "error",
					error: {
						role: "assistant",
						content: [],
						api: model.api,
						provider: model.provider,
						model: model.id,
						usage: {
							input: 0,
							output: 0,
							cacheRead: 0,
							cacheWrite: 0,
							totalTokens: 0,
							cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
						},
						stopReason: "error",
						errorMessage: `Kimi stream failed: ${getErrorMessage(error)}`,
						timestamp: Date.now(),
					},
				});
			}

			if (shouldRetry) {
				attempt += 1;
				continue;
			}
			break;
		}

		filtered.end();
	})();

	return filtered;
}
