/**
 * The resolver-plugin lane — ResolverPort v2 backed by the bundled LSP
 * resolver plugin (SDD §8.3 Resolvers, chunk:m5-lsp-resolver-plugin,
 * specs/001-lsp-resolver-plugin).
 *
 * Lives in the protocol layer deliberately: like `pluginHostDeps`, it needs
 * the tools-layer plugin host vocabulary AND the jail — the two things the
 * adapters layer is forbidden to see (scripts/check-import-graph.ts). The
 * engine-child invoke itself is injected by the composition root, typed
 * structurally so no adapters import is needed here.
 *
 * The upgrade lane, never the floor: every failure mode — plugin absent,
 * un-blessed, un-granted, circuit-open, slow, crashed, malformed — answers an
 * EMPTY edge list and the caller keeps its symbolic (tree-sitter) answer.
 * This class never throws and never waits past its per-request budget, which
 * is what "nothing ever waits on the resolver, and nothing dies with it"
 * (spec US2) means in code.
 *
 * Lazy boot (FR-003): the plugin is not consulted below Tier-1 index — the
 * first possible invoke is strictly after indexing, and no request path ever
 * blocks on plugin startup because startup IS an invoke.
 *
 * Circuit breaker (FR-005): rides the disk-backed plugin-health mechanism
 * (`.workbench/plugin-health.json`, stateless, componentHash-keyed) with one
 * resolver-specific refinement — a breach EXPIRES after
 * [`RESOLVER_RETRY_AFTER_MS`], giving the bounded, lazy retry the spec
 * requires while a changed component still clears immediately.
 *
 * Jail (FR-009): every location the guest reports is admitted through the
 * same `canonicalizeWithin` the write path uses; one out-of-jail or
 * malformed location marks the answer malformed, records a breach, and
 * drops the whole upgrade.
 */

import { existsSync, readdirSync, realpathSync } from "node:fs";
import { join, relative } from "node:path";
import { canonicalizeWithin } from "../jail";
import type { ResolvedEdge, ResolveRequest, ResolverPort } from "../ports";
import { PORT_VERSIONS } from "../ports/versions";
import {
  effectiveCapabilities,
  type InstalledPluginRecord,
  PLUGIN_FUEL_CAP,
  PLUGIN_MEMORY_CAP,
  type PluginComponentInfo,
  type PluginGuestPolicy,
  type PluginHealthRecord,
  type PluginManifest,
  parsePluginManifest,
  SUBPROCESS_IMPORT,
} from "../tools/plugins";

/** The bundled resolver's inventory name (`<publisher>.<pkg>`). */
export const RESOLVER_PLUGIN_NAME = "workbench.lsp-resolver";

/** Breach expiry: the bounded, lazy retry window (spec US2 scenario 3). */
export const RESOLVER_RETRY_AFTER_MS = 60_000;

/** Per-request ceiling: past this, the symbolic answer ships without us. */
export const RESOLVER_BUDGET_MS_DEFAULT = 2_000;

/** One host-curated language-server launch spec (specs/001 data-model.md). */
export interface LanguageServerSpec {
  readonly languageId: string;
  readonly bin: string;
  readonly args: readonly string[];
  /** Transport is always framed stdio (wit/subprocess.wit). */
  readonly transport: "stdio";
  readonly source: "detected" | "test-seam";
}

/** PATH probe, injectable for determinism in tests. */
export type BinaryLookup = (bin: string) => string | null;

const defaultLookup: BinaryLookup = (bin) => Bun.which(bin);

/**
 * Resolve a language's server binary to something that ACTUALLY RUNS, given
 * the workspace root, or null. Injectable so unit tests never shell out to a
 * real toolchain manager (specs/008 FR-006).
 */
export type LanguageServerResolver = (root: string) => string | null;

/** Does `bin --version` exit 0? Rejects a rustup proxy whose active
 * toolchain lacks the component (it prints "Unknown binary" and exits non-0).
 * specs/007 deviation 3, encoded as a detection rule (specs/008 FR-003). */
function runs(bin: string): boolean {
  try {
    return Bun.spawnSync([bin, "--version"], { stdout: "pipe", stderr: "pipe" }).exitCode === 0;
  } catch {
    return false;
  }
}

/**
 * Resolve the workspace's rust-analyzer (specs/008 R2/R3). Ask the toolchain
 * manager first — `rustup which rust-analyzer` with `cwd = root` returns the
 * absolute binary for the workspace's active/pinned toolchain and only
 * succeeds when the component is installed for it, so a `rust-toolchain.toml`
 * pin is honored for free. Fall back to a PATH `rust-analyzer` confirmed
 * runnable. Never returns the uninstalled-component proxy, so the pool never
 * spawns a doomed server.
 */
export function resolveRustAnalyzer(root: string): string | null {
  try {
    const result = Bun.spawnSync(["rustup", "which", "rust-analyzer"], {
      cwd: root,
      stdout: "pipe",
      stderr: "pipe",
    });
    if (result.exitCode === 0) {
      const path = new TextDecoder().decode(result.stdout).trim();
      if (path.length > 0) return path;
    }
  } catch {
    // rustup absent: fall through to the PATH probe.
  }
  const onPath = Bun.which("rust-analyzer");
  return onPath !== null && runs(onPath) ? onPath : null;
}

/**
 * Locate the directory holding the Cargo project a workspace root serves: the
 * root itself, else ONE level down — a `<repo>/rust/Cargo.toml` monorepo
 * layout is common and rust-analyzer discovers it from the parent. Bounded
 * to depth 1 and to a few dozen entries so a huge root never turns startup
 * detection into a walk (Constitution VI). Returns null when no manifest is
 * found.
 */
export function findCargoProjectDir(root: string): string | null {
  if (existsSync(join(root, "Cargo.toml"))) return root;
  let entries: string[];
  try {
    entries = readdirSync(root);
  } catch {
    return null;
  }
  // Deterministic: a sorted walk so two equal trees produce one answer.
  for (const name of entries.sort()) {
    if (name.startsWith(".") || name === "node_modules" || name === "target") continue;
    const candidate = join(root, name);
    if (existsSync(join(candidate, "Cargo.toml"))) return candidate;
  }
  return null;
}

/**
 * Probe the workspace for detectable language servers. Deliberately minimal
 * breadth (spec Assumptions): a TypeScript project with
 * `typescript-language-server` on PATH, and a Cargo project with a runnable
 * `rust-analyzer` (specs/008). Detection failure is an empty table, never an
 * error: the resolver then simply never upgrades. Only this module and a
 * test's injected resolvers mint specs — the guest addresses a language id;
 * the HOST decides which binary runs (Constitution IV). Called once at
 * startup, off every request path (src/main.ts).
 */
export function detectLanguageServers(
  root: string,
  lookup: BinaryLookup = defaultLookup,
  rustResolver: LanguageServerResolver = resolveRustAnalyzer,
): LanguageServerSpec[] {
  const specs: LanguageServerSpec[] = [];
  const tsProject =
    existsSync(join(root, "tsconfig.json")) || existsSync(join(root, "package.json"));
  if (tsProject) {
    const bin = lookup("typescript-language-server");
    if (bin !== null) {
      specs.push({
        languageId: "typescript",
        bin,
        args: ["--stdio"],
        transport: "stdio",
        source: "detected",
      });
    }
  }
  // Rust (specs/008): Cargo project + a rust-analyzer that runs. The resolved
  // absolute toolchain binary is spawned directly by the pool; rust-analyzer
  // finds its own sysroot. Args are its default stdio LSP mode. The manifest
  // may sit one level below the root (`<repo>/rust/Cargo.toml`): the resolver
  // is asked with THAT directory as cwd so a nested rust-toolchain.toml pin
  // is honored, while the server itself still runs at the workspace root
  // (it discovers the nested workspace from there — measured, not assumed).
  const cargoDir = findCargoProjectDir(root);
  if (cargoDir !== null) {
    const bin = rustResolver(cargoDir);
    if (bin !== null) {
      specs.push({
        languageId: "rust",
        bin,
        args: [],
        transport: "stdio",
        source: "detected",
      });
    }
  }
  return specs;
}

/** One guest-reported location, pre-jail (mirrors the FFI shape). */
export interface ResolverGuestLocation {
  readonly path: string;
  readonly line: number;
  readonly column: number;
}

/** Typed resolver-lane outcome (mirrors the engine child's answer). */
export interface ResolverGuestOutcome {
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
  readonly locations: readonly ResolverGuestLocation[];
  /** Language-server sessions the invoke spawned fresh / reused warm
   * (specs/007-resolver-server-pool FR-013; additive, absent on rigs that
   * script outcomes). */
  readonly sessionsOpened?: number;
  readonly sessionsReused?: number;
}

/** The spec shape crossing to the engine child (mirrors the FFI shape). */
export interface ResolverSubprocessSpec {
  readonly languageId: string;
  readonly bin: string;
  readonly args: readonly string[];
}

export interface ResolverPluginDeps {
  /** Workspace root (jail root and the guest's reported workspace-root). */
  readonly root: string;
  /** Host-curated detection table (detection + test seam). */
  readonly specs: readonly LanguageServerSpec[];
  /** Current index tier — the lazy-boot gate. */
  indexTier(): Promise<number>;
  /** Installed plugins with grant records, read per call (stateless). */
  inventory(): Promise<readonly InstalledPluginRecord[]>;
  /** plugin.toml text for a plugin dir; throws if unreadable. */
  readManifest(dir: string): Promise<string>;
  /** Resolve + hash the component, or null when missing/jailed. */
  componentInfo(dir: string, componentPath: string): Promise<PluginComponentInfo | null>;
  /** The typed resolver lane (engine child `sandbox/resolver`). */
  invokeResolver(
    absPath: string,
    symbol: string,
    capabilities: readonly string[],
    policy: PluginGuestPolicy,
    specs: readonly ResolverSubprocessSpec[],
    root: string,
  ): Promise<ResolverGuestOutcome>;
  /** The disk-backed circuit breaker (shared with the plugin-tool host). */
  readonly health: {
    read(): Promise<readonly PluginHealthRecord[]>;
    append(record: PluginHealthRecord): Promise<void>;
  };
  /** Wall-clock; injected for determinism in tests. */
  now(): string;
  readonly budgetMs?: number;
}

interface ResolvedPluginFiles {
  readonly record: InstalledPluginRecord;
  readonly manifest: PluginManifest;
  readonly info: PluginComponentInfo;
  readonly capabilities: readonly string[];
}

export class PluginResolver implements ResolverPort {
  readonly version = PORT_VERSIONS.resolver;

  constructor(private readonly deps: ResolverPluginDeps) {}

  async resolve(request: ResolveRequest): Promise<readonly ResolvedEdge[]> {
    // Upgrade lane only: definitions stay with the index's symbolic answer.
    if (request.kind !== "references") return [];
    if (this.deps.specs.length === 0) return [];
    try {
      return await this.upgrade(request.symbol);
    } catch {
      // Constitution I: a resolver problem degrades service, never ends it.
      return [];
    }
  }

  private async upgrade(symbol: string): Promise<readonly ResolvedEdge[]> {
    if ((await this.deps.indexTier()) < 1) return [];

    const plugin = await this.findPlugin();
    if (plugin === null) return [];

    if (await this.circuitOpen(plugin.info.sha256)) return [];

    const policy: PluginGuestPolicy = {
      fuel: Math.min(plugin.manifest.limits.fuel ?? PLUGIN_FUEL_CAP, PLUGIN_FUEL_CAP),
      memoryCapBytes: Math.min(
        plugin.manifest.limits.memoryCapBytes ?? PLUGIN_MEMORY_CAP,
        PLUGIN_MEMORY_CAP,
      ),
    };
    const specs: ResolverSubprocessSpec[] = this.deps.specs.map((s) => ({
      languageId: s.languageId,
      bin: s.bin,
      args: [...s.args],
    }));

    const outcome = await this.withBudget(
      this.deps.invokeResolver(
        plugin.info.absPath,
        symbol,
        plugin.capabilities,
        policy,
        specs,
        this.deps.root,
      ),
    );
    if (outcome === "budget-exceeded") {
      // Slowness is a health signal, not a wait (spec edge case).
      await this.breach(plugin.info.sha256, "resolver exceeded its per-request budget");
      return [];
    }
    if (outcome.outcome !== "ok") {
      if (outcome.outcome !== "cap-denied") {
        await this.breach(plugin.info.sha256, `${outcome.outcome}: ${outcome.detail}`);
      }
      return [];
    }

    const edges: ResolvedEdge[] = [];
    for (const location of outcome.locations) {
      const admitted = this.admit(location.path);
      if (admitted === null || location.line < 1 || location.column < 1) {
        // FR-009: one malformed/out-of-jail location poisons the answer.
        await this.breach(
          plugin.info.sha256,
          `malformed resolver location ${JSON.stringify(location.path)}`,
        );
        return [];
      }
      edges.push({
        location: { path: admitted, line: location.line, column: location.column },
        provenance: "resolver-lsp",
        confidence: "high",
      });
    }
    return edges;
  }

  /** Find the bundled resolver: installed, blessed, subprocess-granted. */
  private async findPlugin(): Promise<ResolvedPluginFiles | null> {
    const inventory = await this.deps.inventory();
    const record = inventory.find(
      (r) => r.name === RESOLVER_PLUGIN_NAME && r.dir !== undefined && r.tier === "blessed",
    );
    if (record?.dir === undefined) return null;
    let source: string;
    try {
      source = await this.deps.readManifest(record.dir);
    } catch {
      return null;
    }
    const parsed = parsePluginManifest(source);
    if (!parsed.ok) return null;
    const { effective } = effectiveCapabilities(record, parsed.manifest);
    if (!effective.includes(SUBPROCESS_IMPORT)) return null;
    const info = await this.deps.componentInfo(record.dir, parsed.manifest.componentPath);
    if (info === null) return null;
    return { record, manifest: parsed.manifest, info, capabilities: effective };
  }

  /** Breach active (not yet expired) under these exact component bytes? */
  private async circuitOpen(componentHash: string): Promise<boolean> {
    const breaches = await this.deps.health.read();
    const now = Date.parse(this.deps.now());
    return breaches.some(
      (b) =>
        b.plugin === RESOLVER_PLUGIN_NAME &&
        b.componentHash === componentHash &&
        Number.isFinite(Date.parse(b.at)) &&
        now - Date.parse(b.at) < RESOLVER_RETRY_AFTER_MS,
    );
  }

  private async breach(componentHash: string, reason: string): Promise<void> {
    try {
      await this.deps.health.append({
        plugin: RESOLVER_PLUGIN_NAME,
        componentHash,
        reason,
        at: this.deps.now(),
      });
    } catch {
      // Health-record IO must never fail a request.
    }
  }

  /** Admit one guest-reported path through the workspace jail (FR-009). */
  private admit(path: string): string | null {
    if (path.length === 0) return null;
    const decision = canonicalizeWithin(this.deps.root, path);
    if (!decision.ok) return null;
    // The jail answers a REAL path (symlinks resolved, §7.1). Relativize
    // against the root's real path too, or a root that is itself a symlink
    // (macOS: /var → /private/var, so every tmp workspace) makes a perfectly
    // in-jail location look like `../../private/var/...` and every resolver
    // answer is breached as "malformed".
    let rootReal: string;
    try {
      rootReal = realpathSync(this.deps.root);
    } catch {
      return null;
    }
    const rel = relative(rootReal, decision.path).replaceAll("\\", "/");
    if (rel.length === 0 || rel.startsWith("..")) return null;
    return rel;
  }

  private withBudget<T>(work: Promise<T>): Promise<T | "budget-exceeded"> {
    const budget = this.deps.budgetMs ?? RESOLVER_BUDGET_MS_DEFAULT;
    return Promise.race([
      work,
      new Promise<"budget-exceeded">((resolve) => {
        setTimeout(() => resolve("budget-exceeded"), budget).unref?.();
      }),
    ]);
  }
}
