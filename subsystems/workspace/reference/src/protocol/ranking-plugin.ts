/**
 * The ranking-signal boot lane (chunk:m5-ranking-shadow, feature
 * 003-ranking-shadow): discovery of `ranking-` plugins, the disk-backed
 * breaker, the persisted per-signal promotion store, the size-capped local
 * shadow log, the in-memory feature cache, and the post-response shadow
 * scorer. The sandbox is NEVER on the search response path (research R2 /
 * SC-005): shadow scoring is fire-and-forget after ranking, and promoted
 * signals apply through the cache's synchronous lookup only — a miss
 * contributes zero (Constitution I).
 *
 * Promotion is a HUMAN decision (SDD §8.3: "until the user promotes it"), so
 * it is deliberately NOT a tool argument an agent could issue: the persisted
 * store (`.workbench/ranking-promotions.json`) is the user surface, and the
 * runtime's promote/demote verbs exist for that surface and for tests.
 * Promotion binds to the exact component hash — new bytes re-earn promotion
 * in shadow (research R5).
 *
 * Contribution hygiene (contracts/ranking.md): every guest feature name is
 * namespaced `<pkg>/<name>` HERE, before the cache, the log, or any scorer
 * sees it — a guest cannot spoof another signal's namespace — and non-finite
 * or absurd values are dropped at this boundary.
 */

import { createHash } from "node:crypto";
import {
  effectiveCapabilities,
  type InstalledPluginRecord,
  PLUGIN_FUEL_CAP,
  PLUGIN_MEMORY_CAP,
  type PluginGuestPolicy,
  type PluginHealthRecord,
  parsePluginManifest,
} from "../tools/plugins";
import {
  type CandidateGroup,
  classifyHit,
  fileClassOf,
  type PluginFeature,
  type PluginFeatureLookup,
  pluginFeatureKey,
} from "../tools/ranking";

/** Inventory pkg-name convention marking a plugin as a ranking signal. */
export const RANKING_SIGNAL_PKG_PREFIX = "ranking-";

/** The bundled first-party signal's wire name. */
export const VISIBILITY_SIGNAL_NAME = "workbench.ranking-visibility";

/** Reject |value| beyond this at the host boundary (spec edge case). */
const MAX_FEATURE_MAGNITUDE = 1e6;

/** Most hit-contexts scored per search — bounds fuel per shadow pass. */
const MAX_SHADOW_HITS = 200;

/** Feature-cache entries kept per signal (data-model.md FeatureCacheEntry). */
const CACHE_CAPACITY = 50_000;

/** Shadow-log size cap (research R4); oldest entries evicted on breach. */
export const SHADOW_LOG_CAP_BYTES = 5 * 1024 * 1024;

/** One raw (un-namespaced) feature as lifted from the guest. */
export interface RawFeature {
  readonly name: string;
  readonly value: number;
}

/**
 * Namespace and validate one guest's contributions for one hit. Malformed
 * entries are dropped per-contribution, silently to any search caller; the
 * caller counts a fully-failed batch toward the plugin's health.
 */
export function sanitizeContributions(
  plugin: string,
  features: readonly RawFeature[],
): PluginFeature[] {
  const out: PluginFeature[] = [];
  for (const feature of features) {
    if (feature.name === "" || feature.name.includes("/")) continue;
    if (!Number.isFinite(feature.value)) continue;
    if (Math.abs(feature.value) > MAX_FEATURE_MAGNITUDE) continue;
    out.push({ name: `${plugin}/${feature.name}`, value: feature.value });
  }
  return out;
}

/** One hit-context crossing the seam into the child's ranking lane — the
 * protocol-layer mirror of the binding's CoreRankingHit (the layer rule
 * forbids importing adapter types here). */
export interface RankingHitContext {
  readonly file: string;
  readonly line: number;
  readonly column: number;
  readonly excerpt: string;
  readonly kindClass: string;
  readonly fileClass: string;
}

/** Batched contribute outcome crossing the seam from the child. */
export interface RankingGuestOutcome {
  readonly outcome: string;
  readonly detail: string;
  readonly fuelUsed: number;
  /** Parallel to the input batch; shorter on a mid-batch failure. */
  readonly contributions: readonly { readonly features: readonly RawFeature[] }[];
}

/** The engine-child call: SANDBOX_RANKING_METHOD behind a seam. */
export type RankingContributeInvoke = (
  absPath: string,
  hits: readonly RankingHitContext[],
  capabilities: readonly string[],
  policy: PluginGuestPolicy,
) => Promise<RankingGuestOutcome>;

/** One signal's observable state (US2/T014 observability). */
export interface RankingSignalStatus {
  /** Wire name, e.g. "workbench.ranking-visibility". */
  readonly plugin: string;
  readonly tier: string;
  readonly promoted: boolean;
  readonly healthy: boolean;
  /** Boot/last outcome kind ("ok", "breakered", "manifest-unreadable", …). */
  readonly outcome: string;
  readonly detail: string;
  readonly cachedHits: number;
}

/** One persisted promotion record (data-model.md PromotionRecord). */
interface PromotionRecord {
  readonly name: string;
  readonly componentHash: string;
  readonly promoted: boolean;
  readonly at: string;
}

export interface RankingSignalsDeps {
  inventory(): Promise<readonly InstalledPluginRecord[]>;
  readManifest(dir: string): Promise<string>;
  componentInfo(
    dir: string,
    componentPath: string,
  ): Promise<{ readonly absPath: string; readonly sha256: string } | null>;
  contribute: RankingContributeInvoke;
  health: {
    read(): Promise<readonly PluginHealthRecord[]>;
    append(record: PluginHealthRecord): Promise<void>;
  };
  /** `.workbench/ranking-promotions.json` (null = absent = all shadow). */
  promotions: {
    read(): Promise<string | null>;
    write(text: string): Promise<void>;
  };
  /** `.workbench/ranking-shadow.jsonl`, size-capped by the runtime. */
  shadowLog: {
    size(): Promise<number>;
    read(): Promise<string>;
    write(text: string): Promise<void>;
    append(line: string): Promise<void>;
  };
  now(): string;
}

interface BootedSignal {
  /** Full wire name ("workbench.ranking-visibility"). */
  readonly plugin: string;
  /** The pkg half — the feature namespace root ("ranking-visibility"). */
  readonly pkg: string;
  readonly tier: string;
  readonly absPath: string;
  readonly sha256: string;
  readonly capabilities: readonly string[];
  readonly policy: PluginGuestPolicy;
  healthy: boolean;
  outcome: string;
  detail: string;
}

function pkgOf(record: { readonly name: string }): string {
  return record.name.split(".")[1] ?? record.name;
}

function isRankingSignal(record: InstalledPluginRecord): boolean {
  return pkgOf(record).startsWith(RANKING_SIGNAL_PKG_PREFIX);
}

/** Derive the child-lane hit contexts for one search's candidates. */
export function hitContextsOf(
  candidates: readonly CandidateGroup[],
  query: string,
  cap: number = MAX_SHADOW_HITS,
): RankingHitContext[] {
  const out: RankingHitContext[] = [];
  for (const group of candidates) {
    const fileClass = fileClassOf(group);
    for (const hit of group.hits) {
      out.push({
        file: group.file,
        line: hit.line,
        column: hit.column,
        excerpt: hit.excerpt,
        kindClass: classifyHit(hit, query),
        fileClass,
      });
      if (out.length >= cap) return out;
    }
  }
  return out;
}

/**
 * The per-workspace ranking-signal runtime. Constructed synchronously so the
 * search path can hold a reference from the start; `boot()` fills it in the
 * background — until boot settles, promotedFeatures() is undefined and
 * afterSearch() is a no-op, which IS the Constitution-I baseline.
 */
export class RankingSignals {
  private readonly deps: RankingSignalsDeps;
  private signals: BootedSignal[] = [];
  private bootStatuses: RankingSignalStatus[] = [];
  private promotions = new Map<string, PromotionRecord>();
  /** Per-signal feature cache: pkg → hitKey → namespaced features. */
  private readonly caches = new Map<string, Map<string, PluginFeature[]>>();
  /** Merged promoted∩healthy view the scorer reads synchronously. */
  private merged = new Map<string, PluginFeature[]>();

  constructor(deps: RankingSignalsDeps) {
    this.deps = deps;
  }

  /** Discover, breaker-check, and load promotion state. Never throws. */
  async boot(): Promise<RankingSignalStatus[]> {
    let inventory: readonly InstalledPluginRecord[];
    try {
      inventory = await this.deps.inventory();
    } catch {
      return [];
    }
    await this.loadPromotions();
    const statuses: RankingSignalStatus[] = [];
    for (const record of inventory.filter(isRankingSignal)) {
      statuses.push(await this.bootOne(record));
    }
    this.bootStatuses = statuses;
    this.rebuildMerged();
    return statuses;
  }

  private async bootOne(record: InstalledPluginRecord): Promise<RankingSignalStatus> {
    // An inventory record with no tier is a sideload (always untrusted).
    const tier = record.tier ?? "sideload";
    const base = {
      plugin: record.name,
      tier,
      promoted: false,
      healthy: false,
      cachedHits: 0,
    };
    if (record.dir === undefined) {
      return {
        ...base,
        outcome: "missing-component",
        detail: "inventory-only record: no plugin directory",
      };
    }
    let source: string;
    try {
      source = await this.deps.readManifest(record.dir);
    } catch (error) {
      return {
        ...base,
        outcome: "manifest-unreadable",
        detail: error instanceof Error ? error.message : String(error),
      };
    }
    const parsed = parsePluginManifest(source);
    if (!parsed.ok) {
      return { ...base, outcome: "manifest-unreadable", detail: parsed.errors.join("; ") };
    }
    const info = await this.deps.componentInfo(record.dir, parsed.manifest.componentPath);
    if (info === null) {
      return {
        ...base,
        outcome: "missing-component",
        detail: `component ${parsed.manifest.componentPath} is missing or outside the workspace`,
      };
    }
    // The disk-backed breaker: a signal that failed under this exact
    // component stays quiet until the bytes change (lang-pack precedent).
    try {
      const breaches = await this.deps.health.read();
      const breach = breaches.find(
        (b) => b.plugin === record.name && b.componentHash === info.sha256,
      );
      if (breach !== undefined) {
        return {
          ...base,
          outcome: "breakered",
          detail: `unhealthy since ${breach.at} (${breach.reason}); a changed component clears the breaker`,
        };
      }
    } catch {
      // Health-record IO must never fail a boot.
    }
    const { effective } = effectiveCapabilities(record, parsed.manifest);
    const policy: PluginGuestPolicy = {
      fuel: Math.min(parsed.manifest.limits.fuel ?? PLUGIN_FUEL_CAP, PLUGIN_FUEL_CAP),
      memoryCapBytes: Math.min(
        parsed.manifest.limits.memoryCapBytes ?? PLUGIN_MEMORY_CAP,
        PLUGIN_MEMORY_CAP,
      ),
    };
    this.signals.push({
      plugin: record.name,
      pkg: pkgOf(record),
      tier,
      absPath: info.absPath,
      sha256: info.sha256,
      capabilities: effective,
      policy,
      healthy: true,
      outcome: "ok",
      detail: "",
    });
    return {
      ...base,
      promoted: this.isPromoted(record.name, info.sha256),
      healthy: true,
      outcome: "ok",
      detail: "",
    };
  }

  private async loadPromotions(): Promise<void> {
    try {
      const raw = await this.deps.promotions.read();
      if (raw === null) return;
      const parsed: unknown = JSON.parse(raw);
      const list = (parsed as { promotions?: unknown }).promotions;
      if (!Array.isArray(list)) return;
      for (const entry of list) {
        const record = entry as PromotionRecord;
        if (typeof record.name !== "string" || typeof record.componentHash !== "string") continue;
        this.promotions.set(record.name, {
          name: record.name,
          componentHash: record.componentHash,
          promoted: record.promoted === true,
          at: typeof record.at === "string" ? record.at : "",
        });
      }
    } catch {
      // A corrupt store means every signal is shadow — the safe default.
    }
  }

  private async persistPromotions(): Promise<void> {
    const promotions = [...this.promotions.values()];
    await this.deps.promotions.write(`${JSON.stringify({ promotions }, null, 2)}\n`);
  }

  /** Promotion binds to the exact artifact: hash mismatch reads as shadow. */
  private isPromoted(plugin: string, sha256: string): boolean {
    const record = this.promotions.get(plugin);
    return record?.promoted === true && record.componentHash === sha256;
  }

  /**
   * Promote one installed signal (explicit user/test action, FR-005).
   * Unknown signals are refused — persisted state never changes for them.
   */
  async promote(name: string): Promise<{ readonly ok: boolean; readonly detail: string }> {
    return this.setPromotion(name, true);
  }

  /** Demote one signal back to shadow (FR-005). */
  async demote(name: string): Promise<{ readonly ok: boolean; readonly detail: string }> {
    return this.setPromotion(name, false);
  }

  private async setPromotion(
    name: string,
    promoted: boolean,
  ): Promise<{ readonly ok: boolean; readonly detail: string }> {
    const signal = this.signals.find((s) => s.plugin === name || s.pkg === name);
    if (signal === undefined) {
      return { ok: false, detail: `unknown ranking signal ${JSON.stringify(name)}` };
    }
    this.promotions.set(signal.plugin, {
      name: signal.plugin,
      componentHash: signal.sha256,
      promoted,
      at: this.deps.now(),
    });
    try {
      await this.persistPromotions();
    } catch (error) {
      return {
        ok: false,
        detail: `promotion not persisted: ${error instanceof Error ? error.message : String(error)}`,
      };
    }
    this.rebuildMerged();
    return { ok: true, detail: signal.plugin };
  }

  /** Rebuild the merged promoted∩healthy view (rare: boot, promote, breach). */
  private rebuildMerged(): void {
    const merged = new Map<string, PluginFeature[]>();
    for (const signal of this.signals) {
      if (!signal.healthy || !this.isPromoted(signal.plugin, signal.sha256)) continue;
      const cache = this.caches.get(signal.pkg);
      if (cache === undefined) continue;
      for (const [key, features] of cache) {
        const existing = merged.get(key);
        if (existing === undefined) merged.set(key, [...features]);
        else existing.push(...features);
      }
    }
    this.merged = merged;
  }

  /**
   * The promoted-lane lookup (SC-005): a synchronous map read, never an
   * await. Undefined when nothing is promoted, healthy, and warm — the
   * scorer then runs the exact baseline path.
   */
  promotedFeatures(): PluginFeatureLookup | undefined {
    return this.merged.size === 0 ? undefined : this.merged;
  }

  /**
   * The shadow-lane hook: schedules batched scoring + logging and returns
   * immediately. Never throws; nothing awaits it (SC-005 pins that the
   * search response resolves while this work is still pending).
   */
  afterSearch(
    query: string,
    candidates: readonly CandidateGroup[],
    servedOrder: readonly string[],
  ): void {
    if (!this.signals.some((s) => s.healthy)) return;
    const hits = hitContextsOf(candidates, query);
    if (hits.length === 0) return;
    void this.scoreAndLog(query, hits, servedOrder).catch(() => {});
  }

  private async scoreAndLog(
    query: string,
    hits: readonly RankingHitContext[],
    servedOrder: readonly string[],
  ): Promise<void> {
    const contributions: Record<string, Record<string, PluginFeature[]>> = {};
    let mergedDirty = false;
    for (const signal of this.signals) {
      if (!signal.healthy) continue;
      const cache = this.cacheOf(signal.pkg);
      const missing = hits.filter((h) => !cache.has(this.keyOf(h)));
      if (missing.length > 0) {
        let outcome: RankingGuestOutcome;
        try {
          outcome = await this.deps.contribute(
            signal.absPath,
            missing,
            signal.capabilities,
            signal.policy,
          );
        } catch (error) {
          await this.breach(signal, error instanceof Error ? error.message : String(error));
          continue;
        }
        // Partial lifts are kept (parallel prefix); a non-ok end breaches.
        outcome.contributions.forEach((c, i) => {
          const hit = missing[i];
          if (hit === undefined) return;
          this.store(cache, this.keyOf(hit), sanitizeContributions(signal.pkg, c.features));
        });
        if (outcome.outcome !== "ok") {
          await this.breach(signal, outcome.detail === "" ? outcome.outcome : outcome.detail);
        } else if (this.isPromoted(signal.plugin, signal.sha256)) {
          mergedDirty = true;
        }
      }
      const rows: Record<string, PluginFeature[]> = {};
      for (const hit of hits) {
        const features = cache.get(this.keyOf(hit));
        if (features !== undefined && features.length > 0) rows[this.keyOf(hit)] = features;
      }
      contributions[signal.pkg] = rows;
    }
    if (mergedDirty) this.rebuildMerged();
    await this.appendShadowLog({
      at: this.deps.now(),
      queryHash: createHash("sha256").update(query).digest("hex").slice(0, 16),
      servedOrder,
      applied: this.signals
        .filter((s) => s.healthy && this.isPromoted(s.plugin, s.sha256))
        .map((s) => s.pkg),
      contributions,
    });
  }

  private keyOf(hit: RankingHitContext): string {
    return pluginFeatureKey(hit.file, hit.line, hit.column, hit.excerpt);
  }

  private cacheOf(pkg: string): Map<string, PluginFeature[]> {
    let cache = this.caches.get(pkg);
    if (cache === undefined) {
      cache = new Map();
      this.caches.set(pkg, cache);
    }
    return cache;
  }

  /** LRU by insertion order: over capacity, the oldest key falls out. */
  private store(cache: Map<string, PluginFeature[]>, key: string, features: PluginFeature[]): void {
    cache.set(key, features);
    if (cache.size > CACHE_CAPACITY) {
      const oldest = cache.keys().next().value;
      if (oldest !== undefined) cache.delete(oldest);
    }
  }

  private async breach(signal: BootedSignal, reason: string): Promise<void> {
    signal.healthy = false;
    signal.outcome = "breakered";
    signal.detail = reason;
    this.rebuildMerged();
    try {
      await this.deps.health.append({
        plugin: signal.plugin,
        componentHash: signal.sha256,
        reason,
        at: this.deps.now(),
      });
    } catch {
      // Health-record IO must never fail scoring.
    }
  }

  /** Append one entry, evicting oldest lines past the size cap (R4). */
  private async appendShadowLog(entry: unknown): Promise<void> {
    const line = JSON.stringify(entry);
    try {
      const size = await this.deps.shadowLog.size();
      if (size + line.length + 1 > SHADOW_LOG_CAP_BYTES) {
        const existing = await this.deps.shadowLog.read();
        const lines = existing.split("\n").filter((l) => l.length > 0);
        const kept = lines.slice(Math.ceil(lines.length / 2));
        await this.deps.shadowLog.write(`${[...kept, line].join("\n")}\n`);
      } else {
        await this.deps.shadowLog.append(`${line}\n`);
      }
    } catch {
      // Logging must never fail a search (FR-004).
    }
  }

  /** Per-signal state rows (T014 observability). */
  statuses(): RankingSignalStatus[] {
    if (this.signals.length === 0) return this.bootStatuses;
    return [
      ...this.bootStatuses.filter((s) => !this.signals.some((x) => x.plugin === s.plugin)),
      ...this.signals.map((s) => ({
        plugin: s.plugin,
        tier: s.tier,
        promoted: this.isPromoted(s.plugin, s.sha256),
        healthy: s.healthy,
        outcome: s.outcome,
        detail: s.detail,
        cachedHits: this.caches.get(s.pkg)?.size ?? 0,
      })),
    ];
  }

  /** Status rows as workbench_status notes (SDD §5.2's schema-free channel). */
  statusNotes(): string[] {
    return this.statuses().map(
      (s) =>
        `ranking signal ${s.plugin}: tier ${s.tier}, ${s.promoted ? "PROMOTED" : "shadow"}, ` +
        `${s.healthy ? "healthy" : `unhealthy (${s.outcome}: ${s.detail})`}, ` +
        `${s.cachedHits} cached hit(s)`,
    );
  }
}
