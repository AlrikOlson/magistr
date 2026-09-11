/**
 * Plugin capability manifests and the plugin-tool host (SDD §8.1, §8.3,
 * chunk:m4-capability-manifests).
 *
 * A plugin declares what it needs in `plugin.toml` — identity, its component,
 * its tools, the WIT world import names it wants, fs-read scopes, resource
 * limits. The human grants at install time (the grant record lives on the
 * `.workbench/plugins.json` inventory entry), and the host enforces at
 * runtime: the effective capability set for a run is the manifest's declared
 * imports intersected with the recorded grants — default deny, an absent
 * grant record grants nothing. The engine child's component lane refuses any
 * un-covered import BEFORE instantiation, so a plugin exceeding its grants
 * answers `PLUGIN_CAP_DENIED` without a single guest instruction running.
 *
 * A plugin that traps, exhausts fuel, breaches its memory cap, or fails to
 * instantiate answers `PLUGIN_CRASHED` and is circuit-breakered unhealthy:
 * the breach is recorded (keyed by plugin name + component content hash, on
 * disk — the server holds no in-memory session state) and later invokes
 * short-circuit without touching the engine, until the component's bytes
 * change. Core tools never consult any of this — they continue at full
 * fidelity by construction (Constitution IV).
 *
 * No plugin can shadow, wrap, intercept or rename a core tool: wire names are
 * DERIVED as `<publisher>.<pkg>/<toolname>` from validated identifier parts —
 * a manifest cannot state a wire name, every derived name contains `/`, and
 * the core dispatch is checked first. Belt-and-braces, the validator also
 * refuses a tool named exactly like a reserved surface name.
 *
 * The fs-read scopes and non-import capabilities are recorded and granted but
 * not yet servable: no component host-call bridge exists in this chunk (the
 * guest is a pure `(string) -> string` invoke). The bridge lands with the
 * first first-party consumer (M5); grants recorded now are what it will honor.
 */

import { okEnvelope, type ResponseEnvelope, taxonomyError } from "../envelope";

/** The closed core tool surface (SDD §1.1): seven tools plus undo, plus the
 * status tool — no plugin may occupy or mimic any of these bare names. */
export const RESERVED_TOOL_NAMES: readonly string[] = [
  "search",
  "read",
  "edit",
  "map",
  "files",
  "script",
  "run",
  "undo",
  "workbench_status",
];

/** publisher, pkg and tool names: lowercase kebab, 1–32 chars. */
const NAME_RE = /^[a-z][a-z0-9-]{0,31}$/;

/** Host ceilings for plugin guests — the §8.1 "one fuel/memory policy":
 * identical to the script lane's defaults; manifest limits clamp DOWNWARD. */
export const PLUGIN_FUEL_CAP = 1_000_000_000;
export const PLUGIN_MEMORY_CAP = 256 * 1024 * 1024;

const PLUGIN_BUDGET_DEFAULT = 4000;

/** Default export address for the plugin's invoke function. */
export const PLUGIN_INVOKE_EXPORT_DEFAULT = "invoke";

/** The subprocess capability's CURRENT grant name — the WIT world import
 * name of `workbench:subprocess/subprocess@0.2.0` (wit/subprocess.wit). The
 * one capability reserved to blessed plugins (SDD §8.3 Resolvers). */
export const SUBPROCESS_IMPORT = "workbench:subprocess/subprocess@0.2.0";

/**
 * True for any stamp of the subprocess capability within major 0
 * (`workbench:subprocess/subprocess@0.<minor>.<patch>`): the blessed-only
 * rule applies to every version equally (specs/007-resolver-server-pool
 * research R3).
 */
export function isSubprocessImport(name: string): boolean {
  return /^workbench:subprocess\/subprocess@0\.\d+\.\d+$/.test(name);
}

/**
 * Does a recorded grant cover a manifest import? Exact match, or the same
 * `package/interface@major` — the project's additive-within-major rule read
 * on the grant side, so a record granted under `…@0.1.0` still authorizes the
 * rebuilt plugin's `…@0.2.0` import. Nothing crosses a major.
 */
export function grantCovers(grant: string, imp: string): boolean {
  if (grant === imp) return true;
  const split = (name: string): readonly [string, number] | null => {
    const at = name.lastIndexOf("@");
    if (at < 0) return null;
    const major = /^(\d+)\.\d+\.\d+$/.exec(name.slice(at + 1));
    return major?.[1] === undefined ? null : [name.slice(0, at), Number(major[1])];
  };
  const g = split(grant);
  const i = split(imp);
  return g !== null && i !== null && g[0] === i[0] && g[1] === i[1];
}

export interface PluginToolDecl {
  readonly name: string;
  readonly description: string;
}

export interface PluginManifest {
  readonly publisher: string;
  readonly pkg: string;
  readonly version: string;
  /** Component wasm, relative to the plugin's own directory. */
  readonly componentPath: string;
  /** Export address of the invoke function: a root export name, or
   * `"pkg:iface/name@ver#func"` for a WIT world's exported interface. */
  readonly invokeExport: string;
  readonly tools: readonly PluginToolDecl[];
  /** WIT world import names the plugin declares it needs (SDD §8.2: world
   * import names are the grant vocabulary). */
  readonly imports: readonly string[];
  /** fs-read scope globs, recorded for the future host-call bridge. */
  readonly fsRead: readonly string[];
  readonly limits: {
    readonly fuel?: number;
    readonly memoryCapBytes?: number;
  };
}

export type ManifestParse =
  | { readonly ok: true; readonly manifest: PluginManifest; readonly warnings: readonly string[] }
  | { readonly ok: false; readonly errors: readonly string[] };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringList(value: unknown, field: string, errors: string[]): string[] {
  if (value === undefined) return [];
  if (!Array.isArray(value) || value.some((v) => typeof v !== "string" || v.length === 0)) {
    errors.push(`${field} must be an array of non-empty strings`);
    return [];
  }
  return value as string[];
}

function positiveInt(value: unknown, field: string, errors: string[]): number | undefined {
  if (value === undefined) return undefined;
  if (typeof value !== "number" || !Number.isInteger(value) || value < 1) {
    errors.push(`${field} must be a positive integer`);
    return undefined;
  }
  return value;
}

/**
 * Parse and validate one `plugin.toml`. Validation is fail-closed on
 * identity, tools and the component path (a bad manifest cannot produce an
 * invocable plugin), warn-tolerant on unknown keys (config precedent, §7.2).
 */
export function parsePluginManifest(source: string): ManifestParse {
  const errors: string[] = [];
  const warnings: string[] = [];
  let doc: unknown;
  try {
    doc = Bun.TOML.parse(source);
  } catch (error) {
    return {
      ok: false,
      errors: [
        `plugin.toml is not valid TOML: ${error instanceof Error ? error.message : String(error)}`,
      ],
    };
  }
  const root = isRecord(doc) ? doc : {};
  for (const key of Object.keys(root)) {
    if (!["plugin", "component", "tools", "capabilities", "limits"].includes(key)) {
      warnings.push(`unknown plugin.toml table [${key}] ignored`);
    }
  }

  const plugin = isRecord(root["plugin"]) ? root["plugin"] : {};
  const publisher = plugin["publisher"];
  const pkg = plugin["pkg"];
  const version = plugin["version"];
  if (typeof publisher !== "string" || !NAME_RE.test(publisher)) {
    errors.push("plugin.publisher must match ^[a-z][a-z0-9-]{0,31}$");
  }
  if (typeof pkg !== "string" || !NAME_RE.test(pkg)) {
    errors.push("plugin.pkg must match ^[a-z][a-z0-9-]{0,31}$");
  }
  if (typeof version !== "string" || version.length === 0) {
    errors.push("plugin.version must be a non-empty string");
  }

  const component = isRecord(root["component"]) ? root["component"] : {};
  const componentPath = component["path"];
  if (
    typeof componentPath !== "string" ||
    componentPath.length === 0 ||
    componentPath.includes("..") ||
    componentPath.startsWith("/") ||
    /^[a-zA-Z]:/.test(componentPath)
  ) {
    errors.push("component.path must be a relative path inside the plugin directory");
  }
  const invokeExport = component["invoke"];
  if (
    invokeExport !== undefined &&
    (typeof invokeExport !== "string" || invokeExport.length === 0)
  ) {
    errors.push("component.invoke must be a non-empty string when present");
  }

  const toolsRaw = root["tools"];
  const tools: PluginToolDecl[] = [];
  if (!Array.isArray(toolsRaw) || toolsRaw.length === 0) {
    errors.push("at least one [[tools]] entry is required");
  } else {
    const seen = new Set<string>();
    for (const entry of toolsRaw) {
      const record = isRecord(entry) ? entry : {};
      const name = record["name"];
      const description = record["description"];
      if (typeof name !== "string" || !NAME_RE.test(name)) {
        errors.push("tools[].name must match ^[a-z][a-z0-9-]{0,31}$");
        continue;
      }
      if (RESERVED_TOOL_NAMES.includes(name)) {
        errors.push(
          `tools[].name ${JSON.stringify(name)} is a reserved core tool name ` +
            "(SDD §1.1: the seven core tools plus undo are immutable)",
        );
        continue;
      }
      if (seen.has(name)) {
        errors.push(`duplicate tool name ${JSON.stringify(name)}`);
        continue;
      }
      seen.add(name);
      tools.push({
        name,
        description: typeof description === "string" ? description : "",
      });
    }
  }

  const capabilities = isRecord(root["capabilities"]) ? root["capabilities"] : {};
  const imports = stringList(capabilities["imports"], "capabilities.imports", errors);
  const fsRead = stringList(capabilities["fs-read"], "capabilities.fs-read", errors);

  const limitsRaw = isRecord(root["limits"]) ? root["limits"] : {};
  const fuel = positiveInt(limitsRaw["fuel"], "limits.fuel", errors);
  const memoryCapBytes = positiveInt(limitsRaw["memory_bytes"], "limits.memory_bytes", errors);

  if (errors.length > 0) return { ok: false, errors };
  return {
    ok: true,
    warnings,
    manifest: {
      publisher: publisher as string,
      pkg: pkg as string,
      version: version as string,
      componentPath: componentPath as string,
      invokeExport: (invokeExport as string | undefined) ?? PLUGIN_INVOKE_EXPORT_DEFAULT,
      tools,
      imports,
      fsRead,
      limits: {
        ...(fuel === undefined ? {} : { fuel }),
        ...(memoryCapBytes === undefined ? {} : { memoryCapBytes }),
      },
    },
  };
}

/**
 * The one place wire names come from: derived, never declared. Every wire
 * name contains `/`, so it can never collide with a core tool name.
 */
export function wireToolName(manifest: PluginManifest, tool: string): string {
  return `${manifest.publisher}.${manifest.pkg}/${tool}`;
}

// ---- Inventory, grants and health -----------------------------------------

/** One `.workbench/plugins.json` entry as the host reads it. Extends the map
 * inventory shape additively: `dir` and `grants` make an entry invocable. */
export interface InstalledPluginRecord {
  readonly name: string;
  readonly version?: string;
  readonly source: "registry" | "sideload";
  readonly tier?: "blessed" | "verified";
  /** Workspace-relative directory holding plugin.toml and the component.
   * Absent = inventory-only (listed in map, nothing to invoke). */
  readonly dir?: string;
  /** Install-time granted import names. Default deny: absent grants nothing. */
  readonly grants?: readonly string[];
}

/**
 * Effective capability set for one plugin run: declared ∩ granted (default
 * deny), with the subprocess import additionally reserved to tier "blessed"
 * (SDD §8.3: "the one capability reserved to blessed plugins"). A grant a
 * human was talked into giving a sideload is ignored with a note here, so
 * the guest is refused BEFORE instantiation (`PLUGIN_CAP_DENIED`) rather
 * than ever linking the interface.
 */
export function effectiveCapabilities(
  record: InstalledPluginRecord,
  manifest: PluginManifest,
): { effective: string[]; notes: string[] } {
  const grants = record.grants ?? [];
  const intersected = manifest.imports.filter((imp) => grants.some((g) => grantCovers(g, imp)));
  const notes: string[] = [];
  const effective = intersected.filter((imp) => {
    if (isSubprocessImport(imp) && record.tier !== "blessed") {
      notes.push(
        `plugin ${record.name}: the subprocess capability is reserved to blessed ` +
          `plugins — grant ignored (tier: ${record.tier ?? "untiered"}, SDD §8.3)`,
      );
      return false;
    }
    return true;
  });
  return { effective, notes };
}

/** One circuit-breaker record (disk-backed; quarantine precedent §6.2). */
export interface PluginHealthRecord {
  readonly plugin: string;
  /** sha256 of the component bytes the breach happened under; a changed
   * component clears the breaker (same recovery story as INPUT_QUARANTINED). */
  readonly componentHash: string;
  readonly reason: string;
  readonly at: string;
}

export interface PluginGuestPolicy {
  readonly fuel: number;
  readonly memoryCapBytes: number;
}

/** Component-lane invoke result — mirrors the engine child's outcome. */
export interface PluginInvokeOutcome {
  readonly outcome:
    | "ok"
    | "out-of-fuel"
    | "memory-cap"
    | "cap-denied"
    | "guest-trap"
    | "guest-exit"
    | "invalid-guest";
  readonly detail: string;
  readonly fuelUsed: number;
  readonly result: string | null;
}

export interface PluginComponentInfo {
  /** Absolute path of the component wasm (jail-checked by the composer). */
  readonly absPath: string;
  /** sha256 of the component bytes — the breaker key. */
  readonly sha256: string;
}

export interface PluginHostDeps {
  /** Installed plugins with grant records, read per call (stateless). */
  inventory(): Promise<readonly InstalledPluginRecord[]>;
  /** plugin.toml text for a plugin dir; throws if unreadable. */
  readManifest(dir: string): Promise<string>;
  /** Resolve + hash the component, or null when missing/jailed. */
  componentInfo(dir: string, componentPath: string): Promise<PluginComponentInfo | null>;
  /** One guest invoke in the engine child (SDD §6.1/§8.1). */
  invokeGuest(
    absPath: string,
    exportName: string,
    payload: string,
    capabilities: readonly string[],
    policy: PluginGuestPolicy,
  ): Promise<PluginInvokeOutcome>;
  /** The disk-backed circuit breaker (.workbench/plugin-health.json). */
  health: {
    read(): Promise<readonly PluginHealthRecord[]>;
    append(record: PluginHealthRecord): Promise<void>;
  };
  /** Wall-clock for health records; injected for determinism in tests. */
  now(): string;
}

/** A listable plugin tool: wire name plus display metadata. */
export interface PluginToolListing {
  readonly name: string;
  readonly title: string;
  readonly description: string;
  readonly plugin: string;
}

interface ResolvedPlugin {
  readonly record: InstalledPluginRecord;
  readonly manifest: PluginManifest;
}

async function resolvePlugins(
  deps: PluginHostDeps,
): Promise<{ plugins: ResolvedPlugin[]; notes: string[] }> {
  const plugins: ResolvedPlugin[] = [];
  const notes: string[] = [];
  let inventory: readonly InstalledPluginRecord[];
  try {
    inventory = await deps.inventory();
  } catch (error) {
    return {
      plugins,
      notes: [
        `plugin inventory unreadable: ${error instanceof Error ? error.message : String(error)}`,
      ],
    };
  }
  for (const record of inventory) {
    if (record.dir === undefined) continue;
    let source: string;
    try {
      source = await deps.readManifest(record.dir);
    } catch {
      notes.push(`plugin ${record.name}: plugin.toml unreadable — not offered`);
      continue;
    }
    const parsed = parsePluginManifest(source);
    if (!parsed.ok) {
      notes.push(`plugin ${record.name}: invalid manifest (${parsed.errors[0]}) — not offered`);
      continue;
    }
    notes.push(...parsed.warnings.map((w) => `plugin ${record.name}: ${w}`));
    plugins.push({ record, manifest: parsed.manifest });
  }
  return { plugins, notes };
}

/** Every invocable plugin tool under its derived wire name. */
export async function listPluginTools(
  deps: PluginHostDeps,
): Promise<{ tools: PluginToolListing[]; notes: string[] }> {
  const { plugins, notes } = await resolvePlugins(deps);
  const tools: PluginToolListing[] = [];
  for (const { record, manifest } of plugins) {
    for (const tool of manifest.tools) {
      tools.push({
        name: wireToolName(manifest, tool.name),
        title: `${tool.name} (plugin ${manifest.publisher}.${manifest.pkg})`,
        description:
          `${tool.description} [Plugin tool, sandboxed (SDD §8.1); ` +
          `source: ${record.source}${record.source === "sideload" ? ", untrusted" : ""}. ` +
          "Output is host-wrapped in the standard envelope.]",
        plugin: record.name,
      });
    }
  }
  return { tools, notes };
}

/** True iff `name` addresses a plugin tool (contains the namespace slash). */
export function isPluginToolName(name: string): boolean {
  return name.includes("/");
}

/**
 * Invoke one plugin tool by wire name. Returns null when no installed plugin
 * derives that name — the caller answers its transport's unknown-tool error,
 * exactly as it would for any unknown name.
 */
export async function runPluginTool(
  deps: PluginHostDeps,
  wireName: string,
  args: Record<string, unknown>,
  v: number,
): Promise<ResponseEnvelope<unknown> | null> {
  const { plugins, notes } = await resolvePlugins(deps);
  let match: { plugin: ResolvedPlugin; tool: PluginToolDecl } | null = null;
  for (const plugin of plugins) {
    for (const tool of plugin.manifest.tools) {
      if (wireToolName(plugin.manifest, tool.name) === wireName) {
        match = { plugin, tool };
        break;
      }
    }
    if (match !== null) break;
  }
  if (match === null) return null;

  const { record, manifest } = match.plugin;
  const pluginLabel = `${manifest.publisher}.${manifest.pkg}`;

  const info = await deps.componentInfo(record.dir ?? "", manifest.componentPath);
  if (info === null) {
    return taxonomyError({
      v,
      code: "PLUGIN_CRASHED",
      plugin: pluginLabel,
      message:
        `plugin ${pluginLabel}: component ${manifest.componentPath} is missing or ` +
        "outside the plugin directory; reinstall the plugin",
      notes,
    });
  }

  // Circuit breaker (SDD §8.1): a recorded breach under these exact component
  // bytes short-circuits without touching the engine. Changing the component
  // clears it — the same recovery story as INPUT_QUARANTINED.
  const breaches = await deps.health.read();
  const breach = breaches.find((b) => b.plugin === pluginLabel && b.componentHash === info.sha256);
  if (breach !== undefined) {
    return taxonomyError({
      v,
      code: "PLUGIN_CRASHED",
      plugin: pluginLabel,
      message:
        `plugin ${pluginLabel} is circuit-breakered unhealthy since ${breach.at} ` +
        `(${breach.reason}); core tools continue at full fidelity. ` +
        "A changed component clears the breaker.",
      notes,
    });
  }

  // Effective capabilities: declared ∩ granted. Default deny — an absent
  // grant record grants nothing, and a grant for an undeclared import is
  // inert (the manifest is the outer bound, the grant record the inner).
  const { effective, notes: capNotes } = effectiveCapabilities(record, manifest);
  notes.push(...capNotes);

  // Manifest limits clamp downward against the host ceiling (§7.2 lattice
  // precedent: a plugin can ask for less, never more).
  const policy: PluginGuestPolicy = {
    fuel: Math.min(manifest.limits.fuel ?? PLUGIN_FUEL_CAP, PLUGIN_FUEL_CAP),
    memoryCapBytes: Math.min(
      manifest.limits.memoryCapBytes ?? PLUGIN_MEMORY_CAP,
      PLUGIN_MEMORY_CAP,
    ),
  };

  const payload = JSON.stringify({ v, tool: match.tool.name, args });
  let outcome: PluginInvokeOutcome;
  try {
    outcome = await deps.invokeGuest(
      info.absPath,
      manifest.invokeExport,
      payload,
      effective,
      policy,
    );
  } catch (error) {
    // Child down or the sandbox lane absent (§6.2) — retryable, not a breach.
    return taxonomyError({
      v,
      code: "ENGINE_UNAVAILABLE",
      message:
        `plugin ${pluginLabel} could not run: ` +
        (error instanceof Error ? error.message : String(error)),
      notes,
    });
  }

  if (outcome.outcome === "cap-denied") {
    return taxonomyError({
      v,
      code: "PLUGIN_CAP_DENIED",
      message:
        `plugin ${pluginLabel} requires the import ${JSON.stringify(outcome.detail)} ` +
        "which is not granted; widening a grant is a human install-time decision (SDD §8.4)",
      notes,
    });
  }
  if (outcome.outcome !== "ok") {
    const reason = `${outcome.outcome}${outcome.detail === "" ? "" : `: ${outcome.detail}`}`;
    await deps.health.append({
      plugin: pluginLabel,
      componentHash: info.sha256,
      reason,
      at: deps.now(),
    });
    return taxonomyError({
      v,
      code: "PLUGIN_CRASHED",
      plugin: pluginLabel,
      message:
        `plugin ${pluginLabel} ${outcome.outcome === "out-of-fuel" ? "exhausted its fuel" : "trapped"} ` +
        `(${reason}) and is now circuit-breakered unhealthy; ` +
        "core tools continue at full fidelity",
      notes,
    });
  }

  let result: unknown;
  try {
    result = JSON.parse(outcome.result ?? "null");
  } catch {
    result = { raw: outcome.result };
  }
  return okEnvelope<unknown>({
    v,
    freshness: "warm",
    budget: { requested: PLUGIN_BUDGET_DEFAULT, used: 1 },
    results: [result],
    notes: [
      `plugin tool ${wireName} (source: ${record.source}); output host-wrapped (SDD §8.3)`,
      ...notes,
    ],
  });
}
