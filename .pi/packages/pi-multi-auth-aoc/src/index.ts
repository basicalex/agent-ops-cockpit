import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { getModels } from "@mariozechner/pi-ai";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { AccountManager } from "./account-manager.js";
import {
	registerGlobalKeyDistributor,
	unregisterGlobalKeyDistributor,
} from "./balancer/index.js";
import { registerMultiAuthCommands } from "./commands.js";
import { loadMultiAuthConfig } from "./config.js";
import { multiAuthDebugLogger } from "./debug-logger.js";
import { registerMultiAuthProviders } from "./provider.js";
import {
	isDelegatedSubagentRuntime,
	resolveRequestedProviderFromArgv,
} from "./runtime-context.js";

const STARTUP_WARMUP_DELAY_MS = 0;
const STARTUP_REFINEMENT_DELAY_MS = 1_500;
const PREFERRED_OPENAI_CODEX_MODEL = "gpt-5.5";
const FALLBACK_OPENAI_CODEX_MODEL = "gpt-5.4";
const PREFERRED_OPENAI_CODEX_MODEL_REF = `openai-codex/${PREFERRED_OPENAI_CODEX_MODEL}`;
const FALLBACK_OPENAI_CODEX_MODEL_REF = `openai-codex/${FALLBACK_OPENAI_CODEX_MODEL}`;

const ENV_API_KEY_PROVIDERS = [
	{ provider: "openrouter", envVar: "OPENROUTER_API_KEY" },
	{ provider: "opencode", envVar: "OPENCODE_API_KEY" },
] as const;

function getErrorMessage(error: unknown): string {
	if (error instanceof Error) {
		return error.message;
	}
	return String(error);
}

type ProjectSettings = {
	defaultProvider?: string;
	defaultModel?: string;
	enabledModels?: string[];
};

function getProjectSettingsPath(): string {
	return resolve(process.cwd(), ".pi", "settings.json");
}

function readProjectSettings(): ProjectSettings | null {
	try {
		return JSON.parse(readFileSync(getProjectSettingsPath(), "utf8")) as ProjectSettings;
	} catch {
		return null;
	}
}

function writeProjectSettings(settings: ProjectSettings): void {
	const path = getProjectSettingsPath();
	mkdirSync(dirname(path), { recursive: true });
	writeFileSync(path, `${JSON.stringify(settings, null, 2)}\n`, "utf8");
}

function shouldManageOpenAiCodexDefault(settings: ProjectSettings): boolean {
	const enabled = Array.isArray(settings.enabledModels) ? settings.enabledModels : [];
	return settings.defaultProvider === "openai-codex"
		&& (
			settings.defaultModel === PREFERRED_OPENAI_CODEX_MODEL
			|| settings.defaultModel === FALLBACK_OPENAI_CODEX_MODEL
			|| enabled.includes(PREFERRED_OPENAI_CODEX_MODEL_REF)
			|| enabled.includes(FALLBACK_OPENAI_CODEX_MODEL_REF)
		);
}

function reconcileOpenAiCodexProjectDefaults(): { changed: boolean; effectiveModel?: string } {
	const settings = readProjectSettings();
	if (!settings || !shouldManageOpenAiCodexDefault(settings)) {
		return { changed: false };
	}

	const availableModels = new Set(getModels("openai-codex").map((model) => model.id.trim()));
	const preferredAvailable = availableModels.has(PREFERRED_OPENAI_CODEX_MODEL);
	const effectiveModel = preferredAvailable
		? PREFERRED_OPENAI_CODEX_MODEL
		: FALLBACK_OPENAI_CODEX_MODEL;

	const enabledModels = Array.isArray(settings.enabledModels) ? [...settings.enabledModels] : [];
	for (const ref of [PREFERRED_OPENAI_CODEX_MODEL_REF, FALLBACK_OPENAI_CODEX_MODEL_REF]) {
		if (!enabledModels.includes(ref)) {
			enabledModels.unshift(ref);
		}
	}

	const changed = settings.defaultModel !== effectiveModel
		|| enabledModels.length !== (settings.enabledModels ?? []).length
		|| enabledModels.some((model, index) => model !== (settings.enabledModels ?? [])[index]);
	if (!changed) {
		return { changed: false, effectiveModel };
	}

	writeProjectSettings({
		...settings,
		defaultModel: effectiveModel,
		enabledModels,
	});
	return { changed: true, effectiveModel };
}

async function bootstrapEnvApiKeys(accountManager: AccountManager): Promise<void> {
	for (const { provider, envVar } of ENV_API_KEY_PROVIDERS) {
		const rawValue = process.env[envVar];
		const apiKey = typeof rawValue === "string" ? rawValue.trim() : "";
		if (!apiKey) {
			continue;
		}
		await accountManager.addApiKeyCredential(provider, apiKey);
	}
}

/**
 * pi-multi-auth extension entry point for multi-account OAuth credential management and rotation.
 */
export default async function multiAuthExtension(pi: ExtensionAPI): Promise<void> {
	const configLoadResult = loadMultiAuthConfig();
	const isSubagentRuntime = isDelegatedSubagentRuntime();
	const requestedSubagentProvider = isSubagentRuntime
		? resolveRequestedProviderFromArgv()
		: undefined;
	const startupWarnings = new Set<string>();
	const recordStartupWarning = (
		message: string,
		context: string,
		error?: unknown,
		onError?: (message: string) => void,
	): void => {
		const normalizedMessage = message.trim();
		if (!normalizedMessage) {
			return;
		}
		startupWarnings.add(normalizedMessage);
		multiAuthDebugLogger.log("startup_warning", {
			context,
			message: normalizedMessage,
			error: error ? getErrorMessage(error) : undefined,
		});
		onError?.(normalizedMessage);
	};
	if (configLoadResult.warning) {
		recordStartupWarning(configLoadResult.warning, "config_load");
	}

	const accountManager = new AccountManager(
		undefined,
		undefined,
		undefined,
		undefined,
		undefined,
		configLoadResult.config,
		{
			startOAuthRefreshScheduler: !isSubagentRuntime,
		},
	);
	const keyDistributor = accountManager.getKeyDistributor();
	registerGlobalKeyDistributor(keyDistributor);

	try {
		await bootstrapEnvApiKeys(accountManager);
	} catch (error) {
		recordStartupWarning(
			`Failed to seed API-key credentials from environment: ${getErrorMessage(error)}`,
			"env_api_key_bootstrap",
			error,
		);
	}

	let warmupInFlight: Promise<void> | null = null;
	let warmupTimer: ReturnType<typeof setTimeout> | null = null;
	let warmupCompleted = false;
	let refinementInFlight: Promise<void> | null = null;
	let refinementTimer: ReturnType<typeof setTimeout> | null = null;

	const scheduleRefinement = (onError?: (message: string) => void): void => {
		if (refinementInFlight || refinementTimer) {
			return;
		}

		refinementTimer = setTimeout(() => {
			refinementTimer = null;
			if (warmupInFlight) {
				scheduleRefinement(onError);
				return;
			}

			refinementInFlight = accountManager
				.autoActivatePreferredCredentials()
				.catch((error: unknown) => {
					recordStartupWarning(
						getErrorMessage(error),
						"startup_refinement",
						error,
						onError,
					);
				})
				.finally(() => {
					refinementInFlight = null;
				});
		}, STARTUP_REFINEMENT_DELAY_MS);
	};

	const startWarmup = (onError?: (message: string) => void): void => {
		if (warmupInFlight) {
			return;
		}

		warmupInFlight = (async () => {
			await accountManager.ensureInitialized();
			await accountManager.autoActivatePreferredCredentials({ avoidUsageApi: true });
		})()
			.then(() => {
				warmupCompleted = true;
				scheduleRefinement(onError);
			})
			.catch((error: unknown) => {
				recordStartupWarning(
					getErrorMessage(error),
					"startup_warmup",
					error,
					onError,
				);
			})
			.finally(() => {
				warmupInFlight = null;
			});
	};

	const scheduleWarmup = (onError?: (message: string) => void): void => {
		if (warmupInFlight || warmupTimer) {
			return;
		}

		warmupTimer = setTimeout(() => {
			warmupTimer = null;
			startWarmup(onError);
		}, STARTUP_WARMUP_DELAY_MS);
	};

	const scheduleStartupWork = (onError?: (message: string) => void): void => {
		if (!warmupCompleted) {
			scheduleWarmup(onError);
			return;
		}
		scheduleRefinement(onError);
	};

	const flushStartupWarnings = (notify?: (message: string) => void): void => {
		if (!notify) {
			return;
		}
		for (const warning of startupWarnings) {
			notify(warning);
		}
	};

	if (!isSubagentRuntime) {
		registerMultiAuthCommands(pi, accountManager);
	}

	try {
		await registerMultiAuthProviders(pi, accountManager, {
			excludeProviders: configLoadResult.config.excludeProviders,
			includeProviders:
				isSubagentRuntime && requestedSubagentProvider
					? [requestedSubagentProvider]
					: undefined,
			streamTimeouts: configLoadResult.config.streamTimeouts,
		});
	} catch (error) {
		recordStartupWarning(
			`Failed to register provider wrappers: ${getErrorMessage(error)}`,
			"provider_registration",
			error,
		);
	}

	pi.on("session_start", (_event, ctx) => {
		registerGlobalKeyDistributor(keyDistributor);
		flushStartupWarnings((message) => {
			ctx.ui.notify(`multi-auth startup warning: ${message}`, "warning");
		});
		try {
			const modelReconcile = reconcileOpenAiCodexProjectDefaults();
			if (modelReconcile.changed) {
				ctx.modelRegistry.refresh();
				ctx.ui.notify(
					`AOC model default synced to ${modelReconcile.effectiveModel} (${PREFERRED_OPENAI_CODEX_MODEL} ${modelReconcile.effectiveModel === PREFERRED_OPENAI_CODEX_MODEL ? "available" : "not available yet"}).`,
					"info",
				);
			}
		} catch (error) {
			ctx.ui.notify(`multi-auth model sync warning: ${getErrorMessage(error)}`, "warning");
		}
		if (!isSubagentRuntime) {
			scheduleStartupWork((message) => {
				ctx.ui.notify(`multi-auth initialization warning: ${message}`, "warning");
			});
		}
	});

	pi.on("session_shutdown", () => {
		if (warmupTimer !== null) {
			clearTimeout(warmupTimer);
			warmupTimer = null;
		}
		if (refinementTimer !== null) {
			clearTimeout(refinementTimer);
			refinementTimer = null;
		}
		accountManager.shutdown();
		unregisterGlobalKeyDistributor(keyDistributor);
	});
}
