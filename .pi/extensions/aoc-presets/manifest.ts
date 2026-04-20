import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

export interface PresetManifest {
  id: string;
  label?: string;
  layout?: string;
  defaultMode: string;
  version?: number;
  runtime?: {
    controller?: string;
    statusBadge?: string;
    persistSessionState?: boolean;
  };
  activation?: {
    envPreset?: string;
    envMode?: string;
  };
  components?: {
    core?: string[];
    default?: string[];
    modes?: Record<string, string[]>;
  };
  commands?: {
    enable?: string[];
    disable?: string[];
  };
  skills?: {
    recommended?: string[];
  };
  integrations?: {
    mind?: {
      policy?: string;
      available?: boolean;
      default?: string;
    };
  };
}

export interface PresetRecord {
  id: string;
  dir: string;
  manifestPath: string;
  componentsDir: string;
  manifest: PresetManifest;
  warnings: string[];
}

function setDeep(target: Record<string, any>, dottedKey: string, value: any): void {
  const parts = dottedKey.split(".").map(part => part.trim()).filter(Boolean);
  if (parts.length === 0) return;
  let cursor: Record<string, any> = target;
  for (let index = 0; index < parts.length - 1; index++) {
    const part = parts[index];
    const current = cursor[part];
    if (!current || typeof current !== "object" || Array.isArray(current)) cursor[part] = {};
    cursor = cursor[part];
  }
  cursor[parts[parts.length - 1]] = value;
}

function parseArray(raw: string): string[] {
  const body = raw.trim().replace(/^\[/, "").replace(/\]$/, "").trim();
  if (!body) return [];
  return body
    .split(",")
    .map(item => item.trim())
    .filter(Boolean)
    .map(item => item.replace(/^"(.*)"$/, "$1").replace(/^'(.*)'$/, "$1"));
}

function parseValue(raw: string): any {
  const value = raw.trim();
  if (value.startsWith("[") && value.endsWith("]")) return parseArray(value);
  if (value === "true") return true;
  if (value === "false") return false;
  if (/^-?\d+$/.test(value)) return Number.parseInt(value, 10);
  if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
    return value.slice(1, -1);
  }
  return value;
}

export function parsePresetToml(content: string): PresetManifest {
  const root: Record<string, any> = {};
  let section: string[] = [];

  for (const rawLine of content.split(/\r?\n/)) {
    const noComment = rawLine.replace(/\s+#.*$/, "").trim();
    if (!noComment) continue;
    if (noComment.startsWith("#")) continue;
    if (noComment.startsWith("[") && noComment.endsWith("]")) {
      section = noComment.slice(1, -1).split(".").map(part => part.trim()).filter(Boolean);
      continue;
    }
    const eqIndex = noComment.indexOf("=");
    if (eqIndex < 0) continue;
    const key = noComment.slice(0, eqIndex).trim();
    const rawValue = noComment.slice(eqIndex + 1).trim();
    const dotted = section.length > 0 ? [...section, key].join(".") : key;
    setDeep(root, dotted, parseValue(rawValue));
  }

  return root as PresetManifest;
}

function validatePresetRecord(record: PresetRecord, projectRoot: string): string[] {
  const warnings = [...record.warnings];
  const { manifest, componentsDir, id } = record;
  if (manifest.id !== id) warnings.push(`manifest id '${manifest.id ?? ""}' does not match directory '${id}'`);
  if (!manifest.defaultMode) warnings.push("missing defaultMode");
  const modes = manifest.components?.modes ?? {};
  if (manifest.defaultMode && !modes[manifest.defaultMode]) warnings.push(`defaultMode '${manifest.defaultMode}' has no components.modes entry`);
  const allComponentGroups = new Map<string, string[]>();
  for (const [mode, names] of Object.entries(modes)) allComponentGroups.set(`mode:${mode}`, names ?? []);
  if (manifest.components?.default) allComponentGroups.set("default", manifest.components.default);
  if (manifest.components?.core) allComponentGroups.set("core", manifest.components.core);
  for (const [label, names] of allComponentGroups.entries()) {
    for (const name of names) {
      const path = join(componentsDir, `${name}.md`);
      if (!existsSync(path)) warnings.push(`component '${name}' referenced by ${label} is missing at ${path}`);
    }
  }
  if (manifest.layout) {
    const layoutPath = resolve(projectRoot, ".aoc", "layouts", `${manifest.layout}.kdl`);
    if (!existsSync(layoutPath)) warnings.push(`layout '${manifest.layout}' missing at ${layoutPath}`);
  }
  return warnings;
}

export function loadPresetRegistry(projectRoot: string): Map<string, PresetRecord> {
  const presetsDir = resolve(projectRoot, ".aoc", "presets");
  const registry = new Map<string, PresetRecord>();
  if (!existsSync(presetsDir)) return registry;

  for (const name of readdirSync(presetsDir)) {
    const dir = join(presetsDir, name);
    try {
      if (!statSync(dir).isDirectory()) continue;
    } catch {
      continue;
    }
    const manifestPath = join(dir, "preset.toml");
    if (!existsSync(manifestPath)) continue;
    const warnings: string[] = [];
    let manifest: PresetManifest;
    try {
      manifest = parsePresetToml(readFileSync(manifestPath, "utf8"));
    } catch (error: any) {
      manifest = { id: name, defaultMode: "default" };
      warnings.push(`failed to parse ${manifestPath}: ${error?.message ?? String(error)}`);
    }
    const record: PresetRecord = {
      id: name,
      dir,
      manifestPath,
      componentsDir: join(dir, "components"),
      manifest,
      warnings,
    };
    record.warnings = validatePresetRecord(record, projectRoot);
    registry.set(name, record);
  }

  return registry;
}
