import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import type { PresetRecord } from "./manifest.ts";
import { applyStatus, describeState, normalizeMode, persistState, type PresetRuntimeState } from "./state.ts";

export interface CommandBindings {
  registry: Map<string, PresetRecord>;
  getState: () => PresetRuntimeState;
  setState: (state: PresetRuntimeState) => void;
}

const DESIGN_MODES = ["critique", "spec", "diff", "handoff", "tokens", "brand", "motion"] as const;
const MOTION_SUBMODES = ["plan", "timeline", "scroll", "svg", "text", "react", "audit"] as const;

function notify(ctx: ExtensionContext, message: string, level: "info" | "success" | "warning" = "info") {
  ctx.ui?.notify?.(message, level);
}

function activatePreset(pi: ExtensionAPI, ctx: ExtensionContext, bindings: CommandBindings, presetId: string, mode?: string, submode?: string, source = "command") {
  const record = bindings.registry.get(presetId);
  if (!record) {
    notify(ctx, `Unknown preset '${presetId}'`, "warning");
    return;
  }
  const next = { ...normalizeMode(record, mode, submode), source, updatedAt: Date.now() };
  bindings.setState(next);
  persistState(pi, next);
  applyStatus(ctx, bindings.registry, next);
  const summary = `${presetId}:${next.mode}${next.submode ? `/${next.submode}` : ""}`;
  notify(ctx, `preset active: ${summary}`, "success");
  if (record.warnings.length > 0) notify(ctx, `preset warnings: ${record.warnings.join("; ")}`, "warning");
}

function disablePreset(pi: ExtensionAPI, ctx: ExtensionContext, bindings: CommandBindings) {
  const next = { preset: undefined, mode: undefined, submode: undefined, source: "command", updatedAt: Date.now() };
  bindings.setState(next);
  persistState(pi, next);
  applyStatus(ctx, bindings.registry, next);
  notify(ctx, "preset: off", "info");
}

function validMode(mode: string): mode is (typeof DESIGN_MODES)[number] {
  return (DESIGN_MODES as readonly string[]).includes(mode);
}

function validMotionSubmode(mode: string): mode is (typeof MOTION_SUBMODES)[number] {
  return (MOTION_SUBMODES as readonly string[]).includes(mode);
}

export function registerPresetCommands(pi: ExtensionAPI, bindings: CommandBindings): void {
  pi.registerCommand("preset", {
    description: "Show or change the active AOC preset",
    handler: async (args, ctx) => {
      const raw = String(args || "").trim();
      if (!raw || raw === "status") {
        notify(ctx, describeState(bindings.registry, bindings.getState()), "info");
        return;
      }
      if (raw === "off") {
        disablePreset(pi, ctx, bindings);
        return;
      }
      const [presetId, mode] = raw.split(/\s+/, 2);
      activatePreset(pi, ctx, bindings, presetId, mode || undefined, undefined, "command");
    },
  });

  pi.registerCommand("design-director", {
    description: "Activate or switch the design preset mode",
    handler: async (args, ctx) => {
      const requested = String(args || "").trim().toLowerCase() || "critique";
      if (!validMode(requested)) {
        notify(ctx, `Unknown design mode '${requested}'. Valid: ${DESIGN_MODES.join(", ")}`, "warning");
        return;
      }
      activatePreset(pi, ctx, bindings, "design", requested, requested === "motion" ? "plan" : undefined, "command");
    },
  });

  pi.registerCommand("design-off", {
    description: "Disable the active design preset",
    handler: async (_args, ctx) => {
      disablePreset(pi, ctx, bindings);
    },
  });

  pi.registerCommand("motion-director", {
    description: "Switch the design preset into motion mode and optional motion submode",
    handler: async (args, ctx) => {
      const requested = String(args || "").trim().toLowerCase() || "plan";
      if (!validMotionSubmode(requested)) {
        notify(ctx, `Unknown motion submode '${requested}'. Valid: ${MOTION_SUBMODES.join(", ")}`, "warning");
        return;
      }
      activatePreset(pi, ctx, bindings, "design", "motion", requested, "command");
    },
  });

  pi.registerCommand("motion-off", {
    description: "Leave motion mode and return to design critique mode",
    handler: async (_args, ctx) => {
      activatePreset(pi, ctx, bindings, "design", "critique", undefined, "command");
    },
  });
}
