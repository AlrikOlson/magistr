/**
 * Lang-pack boot lane (SDD §8.3 Language packs, chunk:m5-lang-pack,
 * specs/002-wasm-lang-pack).
 *
 * Discovers installed lang packs in the plugin inventory (the
 * `lang-pack-` pkg-name convention), reads each pack's carrier exports in
 * the engine child's sandbox lane, and registers the language into the
 * child's dynamic registry. The trust tier decides the execution lane there
 * (research R5): tier "blessed" may promote to native execution; everything
 * else runs the grammar in the WasmStore.
 *
 * Degradation is Constitution I throughout: a pack that fails to load is
 * breakered (disk-backed, same mechanism as tool plugins) and its language
 * simply stays lexical-only — never an error surface for unrelated queries.
 * The child owns the registry, so a child restart forgets it; callers may
 * re-run boot at any time (registration is idempotent per language).
 */

import {
  effectiveCapabilities,
  type InstalledPluginRecord,
  PLUGIN_FUEL_CAP,
  PLUGIN_MEMORY_CAP,
  type PluginGuestPolicy,
  type PluginHealthRecord,
  parsePluginManifest,
} from "../tools/plugins";

/** Inventory pkg-name convention marking a plugin as a lang pack. */
export const LANG_PACK_PKG_PREFIX = "lang-pack-";

/** The bundled first-party pack's wire name. */
export const GLEAM_PACK_NAME = "workbench.lang-pack-gleam";

/** One pack's boot verdict — the host-side half of FR-007's observability
 * (the engine-side truth is the binding's langPackSnapshot). */
export interface LangPackStatus {
  /** Wire name, e.g. "workbench.lang-pack-gleam". */
  readonly plugin: string;
  readonly ok: boolean;
  /** Sandbox outcome kind or a boot-stage refusal ("missing-component",
   * "manifest-unreadable", "breakered", "registration-refused"). */
  readonly outcome: string;
  readonly detail: string;
  /** Registered language id, present iff ok. */
  readonly language?: string;
  readonly extensions?: readonly string[];
  /** "native" (blessed promotion) | "wasm" — the lane the registry chose. */
  readonly lane?: string;
  readonly queriesOk?: boolean;
  /** Every shadowed claim or degradation, verbatim (FR-012). */
  readonly notes: readonly string[];
}

/** One registered pack language, as the engine child reports it — the
 * protocol-layer mirror of the binding's LangPackRegistration (the layer
 * rule forbids importing adapter types here; the shapes are pinned
 * structurally where main.ts wires the seam). */
export interface LangPackGuestRegistration {
  readonly id: string;
  readonly extensions: readonly string[];
  readonly lane: string;
  readonly queriesOk: boolean;
  readonly notes: readonly string[];
}

/** Carrier read + registration outcome crossing the seam from the child. */
export interface LangPackGuestOutcome {
  readonly outcome: string;
  readonly detail: string;
  readonly fuelUsed: number;
  /** Absent/null both mean "not registered". */
  readonly registered?: LangPackGuestRegistration | null;
}

/** The engine-child call: SANDBOX_LANG_PACK_METHOD behind a seam. */
export type LangPackLoadInvoke = (
  absPath: string,
  blessed: boolean,
  capabilities: readonly string[],
  policy: PluginGuestPolicy,
) => Promise<LangPackGuestOutcome>;

export interface LangPackBootDeps {
  inventory(): Promise<readonly InstalledPluginRecord[]>;
  readManifest(dir: string): Promise<string>;
  componentInfo(
    dir: string,
    componentPath: string,
  ): Promise<{ readonly absPath: string; readonly sha256: string } | null>;
  loadPack: LangPackLoadInvoke;
  health: {
    read(): Promise<readonly PluginHealthRecord[]>;
    append(record: PluginHealthRecord): Promise<void>;
  };
  now(): string;
}

function isLangPack(record: InstalledPluginRecord): boolean {
  const pkg = record.name.split(".")[1] ?? record.name;
  return pkg.startsWith(LANG_PACK_PKG_PREFIX);
}

/**
 * Boot every installed lang pack. Never throws: each pack answers a status
 * row, and a failed pack breaches the breaker so repeated boots stay quiet
 * until the component changes.
 */
export async function bootLangPacks(deps: LangPackBootDeps): Promise<LangPackStatus[]> {
  let inventory: readonly InstalledPluginRecord[];
  try {
    inventory = await deps.inventory();
  } catch {
    return [];
  }
  const statuses: LangPackStatus[] = [];
  for (const record of inventory.filter(isLangPack)) {
    statuses.push(await bootOne(deps, record));
  }
  return statuses;
}

async function bootOne(
  deps: LangPackBootDeps,
  record: InstalledPluginRecord,
): Promise<LangPackStatus> {
  const plugin = record.name;
  if (record.dir === undefined) {
    return {
      plugin,
      ok: false,
      outcome: "missing-component",
      detail: "inventory-only record: no plugin directory",
      notes: [],
    };
  }
  let source: string;
  try {
    source = await deps.readManifest(record.dir);
  } catch (error) {
    return {
      plugin,
      ok: false,
      outcome: "manifest-unreadable",
      detail: error instanceof Error ? error.message : String(error),
      notes: [],
    };
  }
  const parsed = parsePluginManifest(source);
  if (!parsed.ok) {
    return {
      plugin,
      ok: false,
      outcome: "manifest-unreadable",
      detail: parsed.errors.join("; "),
      notes: [],
    };
  }
  const info = await deps.componentInfo(record.dir, parsed.manifest.componentPath);
  if (info === null) {
    return {
      plugin,
      ok: false,
      outcome: "missing-component",
      detail: `component ${parsed.manifest.componentPath} is missing or outside the workspace`,
      notes: [],
    };
  }

  // The disk-backed breaker: a pack that failed under this exact component
  // stays quiet until the bytes change (same recovery story as tool plugins).
  try {
    const breaches = await deps.health.read();
    const breach = breaches.find((b) => b.plugin === plugin && b.componentHash === info.sha256);
    if (breach !== undefined) {
      return {
        plugin,
        ok: false,
        outcome: "breakered",
        detail: `unhealthy since ${breach.at} (${breach.reason}); a changed component clears the breaker`,
        notes: [],
      };
    }
  } catch {
    // Health-record IO must never fail a boot.
  }

  const { effective, notes: capNotes } = effectiveCapabilities(record, parsed.manifest);
  // Manifest limits clamp downward against the host ceiling (§7.2 lattice
  // precedent: a plugin can ask for less, never more).
  const policy: PluginGuestPolicy = {
    fuel: Math.min(parsed.manifest.limits.fuel ?? PLUGIN_FUEL_CAP, PLUGIN_FUEL_CAP),
    memoryCapBytes: Math.min(
      parsed.manifest.limits.memoryCapBytes ?? PLUGIN_MEMORY_CAP,
      PLUGIN_MEMORY_CAP,
    ),
  };
  const blessed = record.tier === "blessed";

  let outcome: LangPackGuestOutcome;
  try {
    outcome = await deps.loadPack(info.absPath, blessed, effective, policy);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    await breach(deps, plugin, info.sha256, detail);
    return { plugin, ok: false, outcome: "registration-refused", detail, notes: capNotes };
  }
  const registered = outcome.registered;
  if (outcome.outcome !== "ok" || registered === undefined || registered === null) {
    const detail = outcome.detail === "" ? outcome.outcome : outcome.detail;
    await breach(deps, plugin, info.sha256, detail);
    return { plugin, ok: false, outcome: outcome.outcome, detail, notes: capNotes };
  }
  return {
    plugin,
    ok: true,
    outcome: "ok",
    detail: "",
    language: registered.id,
    extensions: registered.extensions,
    lane: registered.lane,
    queriesOk: registered.queriesOk,
    notes: [...capNotes, ...registered.notes],
  };
}

async function breach(
  deps: LangPackBootDeps,
  plugin: string,
  componentHash: string,
  reason: string,
): Promise<void> {
  try {
    await deps.health.append({ plugin, componentHash, reason, at: deps.now() });
  } catch {
    // Health-record IO must never fail a boot.
  }
}
