import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

// Prefer an explicit workspace-scoped endpoint from env when available.
// Example from Alibaba Model Studio EU workspace UI:
// https://ws-<workspace>.eu-central-1.maas.aliyuncs.com/compatible-mode/v1
// Fallback remains the public intl endpoint.
const DEFAULT_ALIBABA_MODEL_STUDIO_BASE_URL = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1";

function resolveAlibabaModelStudioBaseUrl(): string {
	const explicit = process.env.DASHSCOPE_BASE_URL?.trim() || process.env.ALIBABA_MODEL_STUDIO_BASE_URL?.trim();
	if (explicit) return explicit;
	return DEFAULT_ALIBABA_MODEL_STUDIO_BASE_URL;
}

/**
 * Alibaba Cloud Model Studio / DashScope provider.
 *
 * Notes:
 * - Pi auto-discovers .pi/extensions/*.ts, so this provider shows up in /model
 *   without any .pi/settings.json extension entry.
 * - Alibaba's OpenAI-compatible docs use DASHSCOPE_API_KEY and region-specific
 *   compatible-mode base URLs.
 * - Alibaba workspace UIs may issue workspace-scoped OpenAI-compatible endpoints
 *   such as https://ws-<workspace>.eu-central-1.maas.aliyuncs.com/compatible-mode/v1.
 * - Allow explicit overrides via DASHSCOPE_BASE_URL / ALIBABA_MODEL_STUDIO_BASE_URL,
 *   which take precedence over the public intl fallback.
 * - Qwen3.6-Plus defaults to thinking mode and expects OpenAI-compatible
 *   requests with Qwen-specific thinking flags.
 */
export default function (pi: ExtensionAPI) {
	pi.registerProvider("alibaba", {
		baseUrl: resolveAlibabaModelStudioBaseUrl(),
		apiKey: "DASHSCOPE_API_KEY",
		authHeader: true,
		api: "openai-completions",
		models: [
			{
				id: "qwen3.6-plus",
				name: "Alibaba Qwen 3.6 Plus",
				reasoning: true,
				input: ["text", "image"],
				cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
				contextWindow: 1_000_000,
				maxTokens: 65_536,
				compat: {
					supportsDeveloperRole: false,
					supportsReasoningEffort: false,
					maxTokensField: "max_tokens",
					thinkingFormat: "qwen",
				},
			},
		],
	});
}
