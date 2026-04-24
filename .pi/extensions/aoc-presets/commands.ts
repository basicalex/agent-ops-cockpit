import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import { Box, matchesKey, truncateToWidth, visibleWidth, wrapTextWithAnsi } from "@mariozechner/pi-tui";
import type { PresetRecord } from "./manifest.ts";
import { applyPresetSkillFilters } from "./skill-filters.ts";
import { applyStatus, describeState, formatHandoff, formatHistory, materializeState, normalizeMode, persistState, type PresetRuntimeState } from "./state.ts";
import { CAVEMAN_EVENT_SET_LEVEL, CAVEMAN_LEVELS, type CavemanLevel } from "../lib/caveman.ts";

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

function commitState(pi: ExtensionAPI, ctx: ExtensionContext, bindings: CommandBindings, next: PresetRuntimeState, notice?: string) {
  bindings.setState(next);
  persistState(pi, next);
  applyStatus(ctx, bindings.registry, next);
  if (notice) notify(ctx, notice, next.preset ? "success" : "info");
}

async function syncSkillFiltersAndNotifyReload(_pi: ExtensionAPI, ctx: ExtensionContext, next: PresetRuntimeState): Promise<void> {
  const result = applyPresetSkillFilters(process.cwd(), next);
  if (!result.changed) return;
  const label = next.preset
    ? `${next.preset}/${next.mode || "default"}${next.submode ? `/${next.submode}` : ""}`
    : "preset off";
  const loaded = result.visibleManagedSkills.length ? result.visibleManagedSkills.join(", ") : "base AOC only";
  notify(ctx, `Preset skill inventory updated for ${label}: ${loaded}. Run /reload to refresh visible skills.`, "info");
}

async function activatePreset(pi: ExtensionAPI, ctx: ExtensionContext, bindings: CommandBindings, presetId: string, mode?: string, submode?: string, source = "command") {
  const record = bindings.registry.get(presetId);
  if (!record) {
    notify(ctx, `Unknown preset '${presetId}'`, "warning");
    return;
  }
  const prev = bindings.getState();
  const normalized = { ...normalizeMode(record, mode, submode), source, updatedAt: Date.now() };
  const next = materializeState(bindings.registry, prev, normalized);
  const summary = `${presetId}:${next.mode}${next.submode ? `/${next.submode}` : ""}`;
  commitState(pi, ctx, bindings, next, `preset active: ${summary}`);
  if (record.warnings.length > 0) notify(ctx, `preset warnings: ${record.warnings.join("; ")}`, "warning");
  await syncSkillFiltersAndNotifyReload(pi, ctx, next);
}

async function disablePreset(pi: ExtensionAPI, ctx: ExtensionContext, bindings: CommandBindings) {
  const prev = bindings.getState();
  const next = materializeState(bindings.registry, prev, {
    preset: undefined,
    mode: undefined,
    submode: undefined,
    source: "command",
    updatedAt: Date.now(),
    activeSkills: [],
    recommendedSkills: [],
  });
  commitState(pi, ctx, bindings, next, "preset: off");
  await syncSkillFiltersAndNotifyReload(pi, ctx, next);
}

function readCurrentCavemanLevel(ctx: ExtensionContext): CavemanLevel {
  for (const entry of [...ctx.sessionManager.getBranch()].reverse()) {
    if ((entry as any).type !== "custom" || (entry as any).customType !== "caveman-level-v1") continue;
    const data = (entry as any).data;
    const restored = data?.cavemanLevel ?? data?.level;
    if (restored && CAVEMAN_LEVELS.includes(restored)) return restored;
  }
  return "off";
}

function samePresetTarget(current: PresetRuntimeState, action?: MenuAction): boolean {
  if (!action) return false;
  if (action.off) return !current.preset;
  return action.preset === current.preset
    && (action.mode || undefined) === (current.mode || undefined)
    && (action.submode || undefined) === (current.submode || undefined);
}

async function applyMenuCommit(pi: ExtensionAPI, ctx: ExtensionContext, bindings: CommandBindings, selected: MenuCommit | null): Promise<void> {
  if (!selected) return;
  const preset = selected.presetAction;
  const current = bindings.getState();
  if (preset && !samePresetTarget(current, preset)) {
    if (preset.off) {
      await disablePreset(pi, ctx, bindings);
    } else if (preset.preset) {
      await activatePreset(pi, ctx, bindings, preset.preset, preset.mode, preset.submode, "menu");
    }
  }
  if (selected.cavemanLevel !== readCurrentCavemanLevel(ctx)) {
    pi.events.emit(CAVEMAN_EVENT_SET_LEVEL, { level: selected.cavemanLevel });
  }
}

function clearHandoff(pi: ExtensionAPI, ctx: ExtensionContext, bindings: CommandBindings) {
  const current = bindings.getState();
  const next = { ...current, handoff: undefined, updatedAt: Date.now() };
  commitState(pi, ctx, bindings, next, current.preset ? "preset handoff cleared" : "handoff cleared");
}

function validMode(mode: string): mode is (typeof DESIGN_MODES)[number] {
  return (DESIGN_MODES as readonly string[]).includes(mode);
}

function validMotionSubmode(mode: string): mode is (typeof MOTION_SUBMODES)[number] {
  return (MOTION_SUBMODES as readonly string[]).includes(mode);
}

function showSkills(ctx: ExtensionContext, state: PresetRuntimeState) {
  notify(ctx, [
    `active skills: ${state.activeSkills?.join(", ") || "none"}`,
    `recommended skills: ${state.recommendedSkills?.join(", ") || "none"}`,
  ].join("\n"), "info");
}

interface MenuNode {
  id: string;
  label: string;
  description?: string;
  action?: {
    preset?: string;
    mode?: string;
    submode?: string;
    off?: boolean;
  };
  children?: MenuNode[];
}

function deriveSubmodes(record: PresetRecord, mode: string): string[] {
  if (mode !== "motion") return [];
  const names = new Set<string>([
    ...Object.keys(record.manifest.skills?.activeBySubmode ?? {}),
    ...Object.keys(record.manifest.skills?.recommendedBySubmode ?? {}),
    ...Object.keys(record.manifest.handoff?.submodeNotes ?? {}),
  ]);
  return [...names].sort();
}

function buildMenuTree(registry: Map<string, PresetRecord>): MenuNode[] {
  const presets = [...registry.values()].sort((a, b) => a.id.localeCompare(b.id));
  return [
    {
      id: "preset-off",
      label: "Preset off",
      description: "Return to neutral AOC routing",
      action: { off: true },
    },
    ...presets.map((record) => {
      const modeNames = Object.keys(record.manifest.components?.modes ?? {});
      const orderedModes = [record.manifest.defaultMode, ...modeNames.filter((name) => name !== record.manifest.defaultMode)].filter((name, index, items) => !!name && items.indexOf(name) === index);
      const children = orderedModes.map((mode) => {
        const submodes = deriveSubmodes(record, mode);
        return {
          id: `${record.id}:${mode}`,
          label: mode,
          description: record.manifest.handoff?.modeNotes?.[mode] || `Switch ${record.id} into ${mode}`,
          action: { preset: record.id, mode },
          children: submodes.length > 0 ? submodes.map((submode) => ({
            id: `${record.id}:${mode}/${submode}`,
            label: submode,
            description: record.manifest.handoff?.submodeNotes?.[submode] || `Switch ${record.id}/${mode} into ${submode}`,
            action: { preset: record.id, mode, submode },
          })) : undefined,
        } satisfies MenuNode;
      });
      return {
        id: record.id,
        label: record.manifest.label || record.id,
        description: `Default mode: ${record.manifest.defaultMode}`,
        action: { preset: record.id, mode: record.manifest.defaultMode },
        children,
      } satisfies MenuNode;
    }),
  ];
}

type MenuAction = NonNullable<MenuNode["action"]>;

interface MenuCommit {
  presetAction?: MenuAction;
  cavemanLevel: CavemanLevel;
}

function openPresetMenu(pi: ExtensionAPI, ctx: ExtensionContext, bindings: CommandBindings) {
  const tree = buildMenuTree(bindings.registry);
  const current = bindings.getState();

  return ctx.ui.custom<MenuCommit | null>((tui, theme, _kb, done) => {
    const stack: Array<{ title: string; nodes: MenuNode[]; index: number }> = [{ title: "Presets", nodes: tree, index: 0 }];
    let pendingCaveman: CavemanLevel = readCurrentCavemanLevel(ctx);
    if (current.preset) {
      const presetIndex = tree.findIndex(node => node.id === current.preset);
      if (presetIndex >= 0) stack[0]!.index = presetIndex;
      const presetNode = presetIndex >= 0 ? tree[presetIndex] : undefined;
      const modeNode = presetNode?.children?.find(node => node.action?.mode === current.mode);
      if (presetNode?.children?.length && modeNode) {
        stack.push({ title: presetNode.label, nodes: presetNode.children, index: presetNode.children.findIndex(node => node.id === modeNode.id) });
        const submodeNode = modeNode.children?.find(node => node.action?.submode === current.submode);
        if (modeNode.children?.length && submodeNode) {
          stack.push({ title: `${presetNode.label}/${modeNode.label}`, nodes: modeNode.children, index: modeNode.children.findIndex(node => node.id === submodeNode.id) });
        }
      }
    }

    function currentLevel() {
      return stack[stack.length - 1]!;
    }

    function currentNode(): MenuNode | undefined {
      return currentLevel().nodes[currentLevel().index];
    }

    function move(delta: number) {
      const level = currentLevel();
      const size = level.nodes.length;
      if (size === 0) return;
      level.index = (level.index + delta + size) % size;
      tui.requestRender();
    }

    function rotateCaveman() {
      const idx = CAVEMAN_LEVELS.indexOf(pendingCaveman);
      pendingCaveman = CAVEMAN_LEVELS[(idx + 1) % CAVEMAN_LEVELS.length];
      tui.requestRender();
    }

    function activateSelected() {
      const selected = currentNode();
      if (!selected) return;
      if (selected.children?.length) {
        stack.push({ title: selected.label, nodes: selected.children, index: 0 });
        tui.requestRender();
        return;
      }
      done({ presetAction: selected.action, cavemanLevel: pendingCaveman });
    }

    function goBack() {
      if (stack.length > 1) {
        stack.pop();
        tui.requestRender();
        return;
      }
      done(null);
    }

    function formatStateLabel(state: PresetRuntimeState): string {
      return state.preset ? `${state.preset}/${state.mode || "default"}${state.submode ? `/${state.submode}` : ""}` : "preset off";
    }

    function isCurrentTarget(node: MenuNode, state: PresetRuntimeState): boolean {
      if (node.action?.off) return !state.preset;
      return node.action?.preset === state.preset
        && (!node.action.mode || node.action.mode === state.mode)
        && (!node.action.submode || node.action.submode === state.submode);
    }

    function buildTargetLabel(node: MenuNode): string {
      if (node.action?.off) return "neutral AOC";
      if (!node.action?.preset) return node.label;
      return `${node.action.preset}/${node.action.mode || "default"}${node.action.submode ? `/${node.action.submode}` : ""}`;
    }

    function padRight(text: string, width: number): string {
      const truncated = truncateToWidth(text, width, "");
      return truncated + " ".repeat(Math.max(0, width - visibleWidth(truncated)));
    }

    function renderPanel(width: number): string[] {
      const state = bindings.getState();
      const level = currentLevel();
      const selected = currentNode();
      const innerWidth = Math.max(58, width - 2);
      const gap = 1;
      const navWidth = Math.max(20, Math.min(28, Math.floor((innerWidth - 3) * 0.28)));
      const detailWidth = Math.max(26, innerWidth - navWidth - gap - 3);
      const bodyHeight = Math.max(10, Math.min(12, level.nodes.length + 2));
      const record = selected?.action?.preset ? bindings.registry.get(selected.action.preset) : undefined;
      const selectedState = selected?.action?.preset
        ? materializeState(bindings.registry, state, {
            preset: selected.action.preset,
            mode: selected.action.mode,
            submode: selected.action.submode,
            source: "preview",
            updatedAt: state.updatedAt,
          })
        : undefined;

      const lines: string[] = [];
      const currentCaveman = readCurrentCavemanLevel(ctx);
      const dim = (text: string) => theme.fg("dim", text);
      const muted = (text: string) => theme.fg("muted", text);
      const accent = (text: string) => theme.fg("accent", text);
      const border = (text: string) => theme.fg("borderMuted", text);
      const strongBorder = (text: string) => theme.fg("borderAccent", text);
      const badge = (text: string, tone: "accent" | "muted" = "muted") => tone === "accent"
        ? theme.bg("selectedBg", theme.fg("accent", ` ${text} `))
        : theme.bg("selectedBg", theme.fg("muted", ` ${text} `));
      const makeRow = (left: string, right = "") => {
        const leftWidth = visibleWidth(left);
        const rightWidth = visibleWidth(right);
        const spacer = " ".repeat(Math.max(1, innerWidth - leftWidth - rightWidth));
        return `│${left}${spacer}${right}│`;
      };
      const pushWrapped = (target: string[], label: string, value: string) => {
        target.push(dim(label));
        target.push(...wrapTextWithAnsi(value, detailWidth));
      };
      const makeDetailRow = (left: string, right = "") => {
        const leftWidth = visibleWidth(left);
        const rightWidth = visibleWidth(right);
        const spacer = " ".repeat(Math.max(1, detailWidth - leftWidth - rightWidth));
        return `${left}${spacer}${right}`;
      };

      lines.push(strongBorder(`╭${"─".repeat(innerWidth)}╮`));
      lines.push(makeRow(`${accent(theme.bold("AOC Presets"))}`, accent(theme.bold(formatStateLabel(state)))));
      lines.push(makeRow(dim(`Presets / ${stack.map(item => item.title).join(" / ")}`), muted("Alt+X reopens")));
      lines.push(border(`├${"─".repeat(innerWidth)}┤`));

      const startIndex = Math.max(0, Math.min(level.index - Math.floor((bodyHeight - 1) / 2), Math.max(0, level.nodes.length - (bodyHeight - 1))));
      const visibleNodes = level.nodes.slice(startIndex, startIndex + (bodyHeight - 1));

      const navLines: string[] = [dim(theme.bold("NAVIGATE"))];
      for (let i = 0; i < visibleNodes.length; i++) {
        const node = visibleNodes[i]!;
        const absoluteIndex = startIndex + i;
        const selectedRow = absoluteIndex === level.index;
        const currentTarget = isCurrentTarget(node, state);
        const hasChildren = !!node.children?.length;
        const stateLabel = currentTarget ? muted(" active") : selectedRow && !hasChildren ? muted(" target") : "";
        const chevron = hasChildren ? muted("  ›") : "";
        const prefix = selectedRow ? accent("▸ ") : "  ";
        const rowText = `${prefix}${node.label}${stateLabel}${chevron}`;
        const padded = padRight(rowText, navWidth);
        navLines.push(selectedRow ? theme.bg("selectedBg", padded) : padded);
      }
      while (navLines.length < bodyHeight) navLines.push(" ".repeat(navWidth));
      if (level.nodes.length > visibleNodes.length) {
        navLines[bodyHeight - 1] = dim(padRight(`${level.index + 1}/${level.nodes.length}`, navWidth));
      }

      const detailLines: string[] = [dim(theme.bold("DETAILS"))];
      detailLines.push(makeDetailRow(`${accent(theme.bold("Caveman"))}`, pendingCaveman === currentCaveman ? badge(`current ${pendingCaveman}`, "accent") : badge(`pending ${pendingCaveman}`)));
      detailLines.push(muted(`Rotate with x. Enter applies preset + caveman together.`));
      detailLines.push("");
      if (selected) {
        const statusBadge = selected.children?.length
          ? badge(`${selected.children.length} choices`)
          : isCurrentTarget(selected, state)
            ? badge("current", "accent")
            : badge("enter to apply");
        detailLines.push(makeDetailRow(`${accent(theme.bold(selected.label))}`, statusBadge));
        detailLines.push(muted(`Target: ${buildTargetLabel(selected)}`));
        const description = selected.description || "No extra guidance for this selection.";
        detailLines.push(...wrapTextWithAnsi(description, detailWidth));
        if (selectedState?.activeSkills?.length) {
          pushWrapped(detailLines, "Active skills", selectedState.activeSkills.join(", "));
        }
        if (selectedState?.recommendedSkills?.length) {
          pushWrapped(detailLines, "Recommended", selectedState.recommendedSkills.join(", "));
        }
        if (record?.manifest.handoff?.modeNotes?.[selected.action?.mode || ""]) {
          pushWrapped(detailLines, "Carry forward", record.manifest.handoff.modeNotes[selected.action!.mode!]!);
        }
      }
      while (detailLines.length < bodyHeight) detailLines.push("");

      for (let row = 0; row < bodyHeight; row++) {
        const left = padRight(navLines[row] || "", navWidth);
        const right = padRight(detailLines[row] || "", detailWidth);
        const divider = row === 0 ? strongBorder("│") : border("│");
        lines.push(`│ ${left}${" ".repeat(gap)}${divider}${" ".repeat(gap)}${right} │`);
      }

      lines.push(border(`├${"─".repeat(innerWidth)}┤`));
      lines.push(makeRow(dim("[j/k] move  [x] caveman  [enter/l] apply  [h/esc] back  [q] close"), muted("selected ▸  active current")));
      lines.push(strongBorder(`╰${"─".repeat(innerWidth)}╯`));
      return lines;
    }

    return {
      render(width: number) {
        const box = new Box(1, 1, (text) => theme.bg("selectedBg", text));
        box.addChild({
          render(innerWidth: number) {
            return renderPanel(innerWidth);
          },
          invalidate() {},
        });
        return box.render(width);
      },
      invalidate() {},
      handleInput(data: string) {
        if (data === "j" || matchesKey(data, "down")) {
          move(1);
          return;
        }
        if (data === "k" || matchesKey(data, "up")) {
          move(-1);
          return;
        }
        if (data === "x") {
          rotateCaveman();
          return;
        }
        if (data === "l" || matchesKey(data, "right") || matchesKey(data, "return")) {
          activateSelected();
          return;
        }
        if (data === "h" || matchesKey(data, "left") || matchesKey(data, "escape")) {
          goBack();
          return;
        }
        if (data === "q") {
          done(null);
        }
      },
    };
  }, {
    overlay: true,
    overlayOptions: {
      width: "62%",
      minWidth: 64,
      maxHeight: "70%",
      anchor: "center",
      margin: 1,
    },
  });
}

export function registerPresetCommands(pi: ExtensionAPI, bindings: CommandBindings): void {
  pi.registerCommand("preset", {
    description: "Show or change the active AOC preset and inspect runtime routing state",
    handler: async (args, ctx) => {
      const raw = String(args || "").trim();
      if (!raw || raw === "status") {
        notify(ctx, describeState(bindings.registry, bindings.getState()), "info");
        return;
      }
      if (raw === "off") {
        await disablePreset(pi, ctx, bindings);
        return;
      }
      if (raw === "skills") {
        showSkills(ctx, bindings.getState());
        return;
      }
      if (raw === "menu" || raw === "select") {
        await applyMenuCommit(pi, ctx, bindings, await openPresetMenu(pi, ctx, bindings));
        return;
      }
      if (raw === "handoff") {
        notify(ctx, formatHandoff(bindings.getState()), "info");
        return;
      }
      if (raw === "clear-handoff") {
        clearHandoff(pi, ctx, bindings);
        return;
      }
      if (raw === "history") {
        notify(ctx, formatHistory(bindings.getState()), "info");
        return;
      }
      const [presetId, mode] = raw.split(/\s+/, 2);
      await activatePreset(pi, ctx, bindings, presetId, mode || undefined, undefined, "command");
    },
  });

  pi.registerCommand("preset-menu", {
    description: "Open an interactive preset navigator",
    handler: async (_args, ctx) => {
      await applyMenuCommit(pi, ctx, bindings, await openPresetMenu(pi, ctx, bindings));
    },
  });

  pi.registerShortcut("alt+x", {
    description: "Open AOC preset mode switcher",
    handler: async (ctx) => {
      notify(ctx, "Opening preset menu…", "info");
      await applyMenuCommit(pi, ctx, bindings, await openPresetMenu(pi, ctx, bindings));
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
      await activatePreset(pi, ctx, bindings, "design", requested, requested === "motion" ? "plan" : undefined, "command");
    },
  });

  pi.registerCommand("design-off", {
    description: "Disable the active design preset",
    handler: async (_args, ctx) => {
      await disablePreset(pi, ctx, bindings);
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
      await activatePreset(pi, ctx, bindings, "design", "motion", requested, "command");
    },
  });

  pi.registerCommand("motion-off", {
    description: "Leave motion mode and return to design critique mode",
    handler: async (_args, ctx) => {
      await activatePreset(pi, ctx, bindings, "design", "critique", undefined, "command");
    },
  });
}
