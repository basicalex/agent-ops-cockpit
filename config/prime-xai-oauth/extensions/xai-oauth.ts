/**
 * xai-oauth — Grok subscription (SuperGrok / X Premium+) provider for prime-agent.
 *
 * Registers an `xai-oauth` provider that authenticates with xAI's RFC 8628
 * device-authorization flow (client id and scopes from the official Grok CLI)
 * and sends chat traffic to the CLI proxy at cli-chat-proxy.grok.com. The
 * public api.x.ai endpoint rejects subscription OAuth tokens with a 402
 * spending-limit error; the proxy serves them, but validates the
 * client-identity headers set below.
 *
 * Managed by agent-ops-cockpit (config/prime-xai-oauth); installed by
 * bin/aoc-prime-xai-oauth-install. Edit in the repo, not in ~/.prime/agent/.
 *
 * Login: /login inside prime-agent, pick "xAI Grok OAuth". Tokens live in
 * ~/.prime/agent/auth.json and refresh automatically.
 */
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import type { OAuthCredentials, OAuthLoginCallbacks } from "@earendil-works/pi-ai";

const ISSUER = "https://auth.x.ai";
const DISCOVERY_URL = `${ISSUER}/.well-known/openid-configuration`;
const DEVICE_CODE_URL = `${ISSUER}/oauth2/device/code`;
const CLIENT_ID = "b1a00492-073a-47ea-816f-4c329264a828";
const SCOPE = "openid profile email offline_access grok-cli:access api:access";

const CHAT_PROXY_BASE_URL = "https://cli-chat-proxy.grok.com/v1";
// The proxy 426s any request without a plausible official-client identity.
const CLIENT_HEADERS = {
	"x-grok-client-version": "0.2.103",
	"x-grok-client-identifier": "grok-shell",
	"User-Agent": "xai-grok-cli",
	"X-XAI-Token-Auth": "xai-grok-cli",
};

// Refresh access tokens 5 minutes before their server-side expiry.
const ACCESS_TOKEN_CLIENT_SKEW_MS = 5 * 60 * 1000;
const REQUEST_TIMEOUT_MS = 20_000;

function assertXaiAuthUrl(url: string, field: string): string {
	const parsed = new URL(url);
	const host = parsed.hostname.toLowerCase();
	if (parsed.protocol !== "https:" || (host !== "x.ai" && !host.endsWith(".x.ai"))) {
		throw new Error(`Invalid xAI ${field}: ${url}`);
	}
	return url;
}

async function discoverTokenEndpoint(): Promise<string> {
	const response = await fetch(DISCOVERY_URL, {
		headers: { Accept: "application/json" },
		signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
	});
	if (!response.ok) throw new Error(`xAI OIDC discovery returned ${response.status}`);
	const payload = (await response.json()) as { token_endpoint?: string };
	const tokenEndpoint = payload.token_endpoint?.trim();
	if (!tokenEndpoint) throw new Error("xAI OIDC discovery response missing token_endpoint");
	return assertXaiAuthUrl(tokenEndpoint, "token_endpoint");
}

function parseTokenResponse(payload: Record<string, unknown>, fallbackRefresh?: string): OAuthCredentials {
	const access = typeof payload.access_token === "string" ? payload.access_token : "";
	const refresh = (typeof payload.refresh_token === "string" && payload.refresh_token) || fallbackRefresh || "";
	const expiresIn = payload.expires_in;
	if (!access || !refresh || typeof expiresIn !== "number" || !Number.isFinite(expiresIn)) {
		throw new Error("xAI token response missing access_token, refresh_token, or expires_in");
	}
	return {
		access,
		refresh,
		expires: Date.now() + expiresIn * 1000 - ACCESS_TOKEN_CLIENT_SKEW_MS,
	};
}

async function login(callbacks: OAuthLoginCallbacks): Promise<OAuthCredentials> {
	const tokenEndpoint = await discoverTokenEndpoint();

	const deviceResponse = await fetch(DEVICE_CODE_URL, {
		method: "POST",
		headers: { "Content-Type": "application/x-www-form-urlencoded", Accept: "application/json" },
		body: new URLSearchParams({ client_id: CLIENT_ID, scope: SCOPE }),
		signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
	});
	if (!deviceResponse.ok) throw new Error(`xAI device-code request failed: ${deviceResponse.status}`);
	const device = (await deviceResponse.json()) as {
		device_code?: string;
		user_code?: string;
		verification_uri?: string;
		verification_uri_complete?: string;
		expires_in?: number;
		interval?: number;
	};
	if (!device.device_code || !device.user_code || !device.verification_uri_complete) {
		throw new Error("xAI device-code response missing required fields");
	}
	assertXaiAuthUrl(device.verification_uri_complete, "verification_uri_complete");

	// prime-agent 0.7.x implements only onAuth/onPrompt/onProgress (no
	// onDeviceCode); calling anything else kills the login dialog silently.
	callbacks.onAuth({
		url: device.verification_uri_complete,
		instructions: `Approve the login in your browser. If asked for a code, enter: ${device.user_code}`,
	});
	callbacks.onProgress?.("Waiting for xAI device authorization...");

	let intervalMs = Math.max(1, device.interval ?? 5) * 1000;
	const deadline = Date.now() + Math.max(60, device.expires_in ?? 900) * 1000;

	while (Date.now() < deadline) {
		if (callbacks.signal?.aborted) throw new Error("xAI login cancelled");
		await new Promise(resolve => setTimeout(resolve, intervalMs));
		const response = await fetch(tokenEndpoint, {
			method: "POST",
			headers: { "Content-Type": "application/x-www-form-urlencoded", Accept: "application/json" },
			body: new URLSearchParams({
				grant_type: "urn:ietf:params:oauth:grant-type:device_code",
				client_id: CLIENT_ID,
				device_code: device.device_code,
			}),
			signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
		});
		const payload = (await response.json()) as Record<string, unknown>;
		if (response.ok) return parseTokenResponse(payload);
		const code = typeof payload.error === "string" ? payload.error : "";
		if (code === "authorization_pending") continue;
		if (code === "slow_down") {
			intervalMs += 5000;
			continue;
		}
		const detail = typeof payload.error_description === "string" ? payload.error_description : code;
		throw new Error(`xAI device authorization failed: ${detail || response.status}`);
	}
	throw new Error("xAI device authorization timed out; run /login again");
}

async function refreshToken(credentials: OAuthCredentials): Promise<OAuthCredentials> {
	if (!credentials.refresh) throw new Error("xAI OAuth credentials missing refresh token");
	const tokenEndpoint = await discoverTokenEndpoint();
	const response = await fetch(tokenEndpoint, {
		method: "POST",
		headers: { "Content-Type": "application/x-www-form-urlencoded", Accept: "application/json" },
		body: new URLSearchParams({
			grant_type: "refresh_token",
			client_id: CLIENT_ID,
			refresh_token: credentials.refresh,
		}),
		signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
	});
	if (!response.ok) throw new Error(`xAI token refresh failed: ${response.status}`);
	const payload = (await response.json()) as Record<string, unknown>;
	return parseTokenResponse(payload, credentials.refresh);
}

export default function (pi: ExtensionAPI) {
	pi.registerProvider("xai-oauth", {
		name: "xAI Grok OAuth (SuperGrok / X Premium+)",
		baseUrl: CHAT_PROXY_BASE_URL,
		api: "openai-completions",
		headers: CLIENT_HEADERS,
		models: [
			{
				id: "grok-4.6",
				name: "Grok 4.6 (subscription)",
				reasoning: true,
				input: ["text", "image"],
				contextWindow: 500000,
				maxTokens: 64000,
				cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
				// Grok 4.6 always reasons; the proxy accepts reasoning_effort low..high.
				thinkingLevelMap: { off: null, minimal: "low", xhigh: "high" },
				compat: { maxTokensField: "max_tokens" },
			},
		],
		oauth: {
			name: "xAI Grok OAuth (SuperGrok / X Premium+)",
			login,
			refreshToken,
			getApiKey: (credentials: OAuthCredentials) => credentials.access,
		},
	});
}
