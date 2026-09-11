/**
 * The five-layer config resolver (SDD §7.2): built-in defaults → org policy
 * → user config → workspace `workbench.toml` → per-session flags, lowest
 * precedence first. Non-security keys merge nearest-wins; security keys are
 * tighten-only downward — a later layer attempting to loosen what an earlier
 * layer explicitly tightened is refused, the tightened value stands, and the
 * attempt is recorded as a `CONFIG_POLICY_DENIED` violation naming the
 * pinning layer. Unknown keys, wrong-typed values, and unreadable layer
 * files warn and are skipped; they never abort startup.
 *
 * Resolution is a pure function of (workspace root, env, session flags): no
 * lock, no database, no clock. `workbench doctor config` can therefore run
 * beside a live server, and the statelessness gate can compare the derived
 * notes byte-for-byte across server processes (Constitution V).
 */

import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { UnsupportedVersionError } from "../data/versioned-db";
import {
  CONFIG_KEYS,
  CONFIG_VERSION,
  type ConfigLayerName,
  isSecuritySpec,
  type KeySpec,
  LAYER_ORDER,
} from "./schema";

/** Env override for the org policy file path (operator/test hook). */
export const ORG_POLICY_ENV = "WORKBENCH_ORG_POLICY";
/** Env override for the user config file path (operator/test hook). */
export const USER_CONFIG_ENV = "WORKBENCH_USER_CONFIG";

export interface ResolvedKey {
  readonly key: string;
  readonly value: unknown;
  /** The layer whose setting is the effective value. */
  readonly decidedBy: ConfigLayerName;
  /**
   * For security keys explicitly decided above defaults: the layer whose
   * setting now binds every later layer. Absent when only defaults spoke.
   */
  readonly pinnedBy?: ConfigLayerName;
}

export interface PolicyViolation {
  readonly code: "CONFIG_POLICY_DENIED";
  readonly key: string;
  readonly attemptedBy: ConfigLayerName;
  readonly attempted: unknown;
  readonly pinnedBy: ConfigLayerName;
  readonly message: string;
}

export interface LayerReport {
  readonly layer: ConfigLayerName;
  /** File path for file layers; absent for defaults and session. */
  readonly source?: string;
  /** Whether the layer contributed anything (file present / flags given). */
  readonly present: boolean;
  /** Parse failure, when the layer was skipped as unreadable. */
  readonly error?: string;
}

export interface ResolvedConfig {
  readonly configVersion: typeof CONFIG_VERSION;
  /** Effective value per known dotted key, registry order. */
  readonly effective: Readonly<Record<string, unknown>>;
  /** The merged `[commands]` run allowlist (§9.6), name → command line. */
  readonly commands: Readonly<Record<string, string>>;
  /** Per-key provenance, registry order then command names sorted. */
  readonly trace: readonly ResolvedKey[];
  readonly warnings: readonly string[];
  readonly violations: readonly PolicyViolation[];
  readonly layers: readonly LayerReport[];
}

export interface ResolveConfigOptions {
  /** Canonical absolute workspace root. */
  readonly root: string;
  readonly env?: Record<string, string | undefined>;
  /** Per-session ephemeral flags, dotted key → value (from `--config k=v`). */
  readonly sessionFlags?: Readonly<Record<string, unknown>>;
}

/** Platform-fixed org policy path (admin-distributed, read-only at runtime). */
function orgPolicyPath(env: Record<string, string | undefined>): string {
  const override = env[ORG_POLICY_ENV];
  if (override !== undefined && override !== "") return override;
  if (process.platform === "win32") {
    return join(env.ProgramData ?? "C:\\ProgramData", "workbench", "policy.toml");
  }
  return "/etc/workbench/policy.toml";
}

/** `~/.config/workbench/config.toml`, honoring XDG_CONFIG_HOME. */
function userConfigPath(env: Record<string, string | undefined>): string {
  const override = env[USER_CONFIG_ENV];
  if (override !== undefined && override !== "") return override;
  const base =
    env.XDG_CONFIG_HOME !== undefined && env.XDG_CONFIG_HOME !== ""
      ? env.XDG_CONFIG_HOME
      : join(homedir(), ".config");
  return join(base, "workbench", "config.toml");
}

interface LayerSettings {
  /** Dotted known-key candidates plus unknown keys, insertion order. */
  readonly entries: ReadonlyMap<string, unknown>;
  /** The `[commands]` table, when the layer declared one. */
  readonly commands?: ReadonlyMap<string, unknown>;
}

/**
 * Flatten a parsed TOML document to dotted keys. The `commands` table keeps
 * one level of dynamic children; everything else flattens fully so unknown
 * nesting surfaces as an unknown dotted key, not a silent object.
 */
function flattenDocument(
  doc: Record<string, unknown>,
  layer: ConfigLayerName,
  source: string,
  warnings: string[],
): LayerSettings {
  const entries = new Map<string, unknown>();
  let commands: Map<string, unknown> | undefined;
  const walk = (prefix: string, value: unknown): void => {
    if (
      typeof value === "object" &&
      value !== null &&
      !Array.isArray(value) &&
      !(value instanceof Date)
    ) {
      for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
        walk(prefix === "" ? k : `${prefix}.${k}`, v);
      }
      return;
    }
    entries.set(prefix, value);
  };
  for (const [key, value] of Object.entries(doc)) {
    if (key === "config_version") {
      if (typeof value === "number" && Number.isInteger(value)) {
        if (value > CONFIG_VERSION) {
          // Durable-class fail-closed (§10.2): written by a newer binary.
          throw new UnsupportedVersionError(
            `config layer ${layer} (${source}) declares config_version ${value}; ` +
              `this binary supports ${CONFIG_VERSION}`,
          );
        }
      } else {
        warnings.push(`config_version in ${layer} config (${source}) must be an integer — ignored`);
      }
      continue;
    }
    if (key === "commands") {
      if (typeof value === "object" && value !== null && !Array.isArray(value)) {
        commands = new Map(Object.entries(value as Record<string, unknown>));
      } else {
        warnings.push(`[commands] in ${layer} config (${source}) must be a table — ignored`);
      }
      continue;
    }
    walk(key, value);
  }
  return commands === undefined ? { entries } : { entries, commands };
}

/** A well-typed candidate value for `spec`, or undefined plus a warning. */
function admitValue(
  spec: KeySpec,
  key: string,
  raw: unknown,
  layer: ConfigLayerName,
  source: string,
  warnings: string[],
): unknown {
  const reject = (expected: string): undefined => {
    warnings.push(
      `config key '${key}' in ${layer} config (${source}) must be ${expected} — ignored`,
    );
    return undefined;
  };
  switch (spec.kind) {
    case "boolean":
      return typeof raw === "boolean" ? raw : reject("a boolean");
    case "number":
      return typeof raw === "number" && Number.isFinite(raw) ? raw : reject("a number");
    case "enum":
      return typeof raw === "string" && spec.levels.includes(raw)
        ? raw
        : reject(`one of ${spec.levels.join(" | ")}`);
    case "string-list":
      return Array.isArray(raw) && raw.every((e) => typeof e === "string")
        ? (raw as readonly string[])
        : reject("an array of strings");
  }
}

/** Whether `candidate` loosens `effective` under the spec's lattice. */
function loosens(spec: KeySpec, effective: unknown, candidate: unknown): boolean {
  if (spec.kind === "boolean" && spec.tighter !== undefined) {
    return effective === spec.tighter && candidate !== spec.tighter;
  }
  if (spec.kind === "enum" && spec.capped === true) {
    return spec.levels.indexOf(candidate as string) > spec.levels.indexOf(effective as string);
  }
  return false;
}

/** Coerce one `--config key=value` flag value: booleans, numbers, JSON arrays, else string. */
function coerceFlagValue(raw: string): unknown {
  if (raw === "true") return true;
  if (raw === "false") return false;
  if (/^-?\d+(\.\d+)?$/.test(raw)) return Number(raw);
  if (raw.startsWith("[")) {
    try {
      return JSON.parse(raw) as unknown;
    } catch {
      return raw;
    }
  }
  return raw;
}

/**
 * Parse repeatable `--config key=value` CLI flags into the session layer.
 * Malformed pairs are returned as errors for the caller's usage failure —
 * a typo on the command line is the one place abort beats warn.
 */
export function parseSessionFlags(pairs: readonly string[]): {
  readonly flags: Readonly<Record<string, unknown>>;
  readonly errors: readonly string[];
} {
  const flags: Record<string, unknown> = {};
  const errors: string[] = [];
  for (const pair of pairs) {
    const eq = pair.indexOf("=");
    if (eq <= 0) {
      errors.push(`--config expects key=value, got '${pair}'`);
      continue;
    }
    flags[pair.slice(0, eq)] = coerceFlagValue(pair.slice(eq + 1));
  }
  return { flags, errors };
}

/** The registry's defaults as a resolved config — no layers consulted. */
export function defaultResolvedConfig(): ResolvedConfig {
  const effective: Record<string, unknown> = {};
  const trace: ResolvedKey[] = [];
  for (const [key, spec] of Object.entries(CONFIG_KEYS)) {
    effective[key] = spec.default;
    trace.push({ key, value: spec.default, decidedBy: "defaults" });
  }
  return {
    configVersion: CONFIG_VERSION,
    effective,
    commands: {},
    trace,
    warnings: [],
    violations: [],
    layers: LAYER_ORDER.map((layer) => ({ layer, present: layer === "defaults" })),
  };
}

/** Resolve the five layers for `root` (SDD §7.2). Never throws for content
 * problems — only for a layer written by a newer binary (§10.2). */
export function resolveConfig(options: ResolveConfigOptions): ResolvedConfig {
  const env = options.env ?? process.env;
  const warnings: string[] = [];
  const violations: PolicyViolation[] = [];
  const layerReports: LayerReport[] = [];

  // ---- Load each layer's raw settings -----------------------------------
  const loadFile = (layer: ConfigLayerName, source: string): LayerSettings | undefined => {
    if (!existsSync(source)) {
      layerReports.push({ layer, source, present: false });
      return undefined;
    }
    let doc: unknown;
    try {
      doc = Bun.TOML.parse(readFileSync(source, "utf8"));
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      warnings.push(`${layer} config (${source}) is not valid TOML — layer skipped: ${reason}`);
      layerReports.push({ layer, source, present: false, error: reason });
      return undefined;
    }
    layerReports.push({ layer, source, present: true });
    return flattenDocument(doc as Record<string, unknown>, layer, source, warnings);
  };

  layerReports.push({ layer: "defaults", present: true });
  const fileLayers: readonly (readonly [ConfigLayerName, LayerSettings | undefined])[] = [
    ["org-policy", loadFile("org-policy", orgPolicyPath(env))],
    ["user", loadFile("user", userConfigPath(env))],
    ["workspace", loadFile("workspace", join(options.root, "workbench.toml"))],
  ];
  const sessionEntries = new Map<string, unknown>();
  let sessionCommands: Map<string, unknown> | undefined;
  for (const [key, value] of Object.entries(options.sessionFlags ?? {})) {
    if (key.startsWith("commands.")) {
      sessionCommands ??= new Map<string, unknown>();
      sessionCommands.set(key.slice("commands.".length), value);
    } else {
      sessionEntries.set(key, value);
    }
  }
  layerReports.push({
    layer: "session",
    present: sessionEntries.size > 0 || sessionCommands !== undefined,
  });
  const layers: readonly (readonly [ConfigLayerName, LayerSettings])[] = [
    ...fileLayers.filter((l): l is [ConfigLayerName, LayerSettings] => l[1] !== undefined),
    ...(sessionEntries.size > 0 || sessionCommands !== undefined
      ? ([
          [
            "session",
            sessionCommands === undefined
              ? { entries: sessionEntries }
              : { entries: sessionEntries, commands: sessionCommands },
          ],
        ] as const)
      : []),
  ];

  // ---- Merge known keys under the lattice -------------------------------
  interface KeyState {
    value: unknown;
    decidedBy: ConfigLayerName;
    explicit: boolean;
  }
  const state = new Map<string, KeyState>();
  for (const [key, spec] of Object.entries(CONFIG_KEYS)) {
    state.set(key, { value: spec.default, decidedBy: "defaults", explicit: false });
  }

  for (const [layer, settings] of layers) {
    const source = layerReports.find((r) => r.layer === layer)?.source ?? "flags";
    for (const [key, raw] of settings.entries) {
      const spec = CONFIG_KEYS[key];
      if (spec === undefined) {
        warnings.push(`unknown config key '${key}' in ${layer} config (${source}) — ignored`);
        continue;
      }
      const value = admitValue(spec, key, raw, layer, source, warnings);
      if (value === undefined) continue;
      const current = state.get(key) as KeyState;
      if (spec.kind === "string-list" && spec.additive === true) {
        // Additive-only: union, so no layer can remove an earlier entry.
        const merged = [
          ...new Set([...(current.value as readonly string[]), ...(value as readonly string[])]),
        ];
        state.set(key, { value: merged, decidedBy: layer, explicit: true });
        continue;
      }
      if (isSecuritySpec(spec) && current.explicit && loosens(spec, current.value, value)) {
        violations.push({
          code: "CONFIG_POLICY_DENIED",
          key,
          attemptedBy: layer,
          attempted: value,
          pinnedBy: current.decidedBy,
          message:
            `${layer} config may not loosen '${key}' — ` +
            `${current.decidedBy} pinned it to ${JSON.stringify(current.value)} (SDD §7.2)`,
        });
        continue;
      }
      state.set(key, { value, decidedBy: layer, explicit: true });
    }
  }

  // ---- Merge [commands] (§9.6): org policy pins the allowlist -----------
  interface CommandState {
    value: string;
    decidedBy: ConfigLayerName;
  }
  const commandState = new Map<string, CommandState>();
  let commandsPinnedBy: ConfigLayerName | undefined;
  for (const [layer, settings] of layers) {
    if (settings.commands === undefined) continue;
    const source = layerReports.find((r) => r.layer === layer)?.source ?? "flags";
    for (const [name, raw] of settings.commands) {
      if (typeof raw !== "string") {
        warnings.push(
          `command '${name}' in ${layer} config (${source}) must be a string — ignored`,
        );
        continue;
      }
      if (commandsPinnedBy !== undefined) {
        const pinned = commandState.get(name);
        if (pinned === undefined || pinned.value !== raw) {
          violations.push({
            code: "CONFIG_POLICY_DENIED",
            key: `commands.${name}`,
            attemptedBy: layer,
            attempted: raw,
            pinnedBy: commandsPinnedBy,
            message:
              `${layer} config may not ${pinned === undefined ? "add" : "change"} ` +
              `command '${name}' — ${commandsPinnedBy} pins the run allowlist (SDD §7.2/§9.6)`,
          });
          continue;
        }
        continue;
      }
      commandState.set(name, { value: raw, decidedBy: layer });
    }
    if (layer === "org-policy") commandsPinnedBy = "org-policy";
  }

  // ---- Assemble deterministic outputs -----------------------------------
  const effective: Record<string, unknown> = {};
  const trace: ResolvedKey[] = [];
  for (const [key, spec] of Object.entries(CONFIG_KEYS)) {
    const s = state.get(key) as KeyState;
    effective[key] = s.value;
    trace.push(
      s.explicit && isSecuritySpec(spec)
        ? { key, value: s.value, decidedBy: s.decidedBy, pinnedBy: s.decidedBy }
        : { key, value: s.value, decidedBy: s.decidedBy },
    );
  }
  const commands: Record<string, string> = {};
  for (const name of [...commandState.keys()].sort()) {
    const s = commandState.get(name) as CommandState;
    commands[name] = s.value;
    trace.push(
      commandsPinnedBy === undefined
        ? { key: `commands.${name}`, value: s.value, decidedBy: s.decidedBy }
        : {
            key: `commands.${name}`,
            value: s.value,
            decidedBy: s.decidedBy,
            pinnedBy: commandsPinnedBy,
          },
    );
  }

  return {
    configVersion: CONFIG_VERSION,
    effective,
    commands,
    trace,
    warnings,
    violations,
    layers: layerReports,
  };
}

/**
 * Deterministic envelope notes for a resolution (Constitution V): unknown
 * keys and policy denials ride every envelope the server emits, in the
 * order the layers were read.
 */
export function configNotes(config: ResolvedConfig): readonly string[] {
  return [
    ...config.warnings.map((w) => `config: ${w}`),
    ...config.violations.map((v) => `config: CONFIG_POLICY_DENIED — ${v.message}`),
  ];
}
