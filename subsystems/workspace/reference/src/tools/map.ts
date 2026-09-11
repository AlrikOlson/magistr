/**
 * The `map` tool (SDD §9.2) — replaces ls -R and orientation greps. One call
 * answers "what is this workspace": a pruned directory tree from the injected
 * workspace walk (Tier-0 capable — no index required), per-file symbol
 * outlines through IndexPort.queryOutline when the Tier-1 index is up, an
 * import-graph summary (top fan-in/fan-out) through queryImportGraph, detected
 * entry points (package.json main/bin, src/main.*, Cargo [[bin]] — pure
 * manifest/walk facts, so Tier-0 capable), index health, a language census,
 * and the installed-plugin inventory with trust labels (§8.4: sideloaded
 * plugins are always labeled untrusted).
 *
 * Of §9.2's overview, project-command detection is deliberately NOT here: it
 * is deferred to chunk m3-run-tool, because detected commands seed `run`'s
 * declared-command allowlist (§9.6, a human-amendable config table), and a
 * read-only overview must not be the source of an execution allowlist.
 *
 * The overview row carries a short `ttlMs` keyed to the index generation
 * (§5.1): a client may cache the answer briefly, and the generation beside it
 * says exactly which index state the answer described.
 *
 * This layer sees only ports and the envelope contract (import-graph rule);
 * the workspace walk and the plugin inventory are injected by the composing
 * layer, so the tool holds no filesystem access and no state.
 */

import {
  type Freshness,
  okEnvelope,
  type ResponseEnvelope,
  taxonomyError,
  truncateResults,
} from "../envelope";
import type { CrateImportEdge, IndexPort, IndexStatus, SymbolEntry } from "../ports";
import {
  type CargoWorkspaceModel,
  crateScopes,
  type MapCargoSummary,
  type MapCrate,
  readCargoWorkspace,
  renderCargo,
} from "./map-cargo";
import { RANKING_WEIGHTS } from "./ranking";

export type {
  MapCargoSummary,
  MapCrate,
  MapCrateDependency,
  MapCrateEntryPoint,
  MapCrateEntryPointKind,
  MapCrateRole,
} from "./map-cargo";
export {
  MAP_CRATE_ENTRY_POINTS_MAX,
  MAP_CRATE_FEATURES_MAX,
  MAP_CRATE_MANIFESTS_MAX,
  MAP_CRATE_ROWS_MAX,
} from "./map-cargo";

import type { SearchWorkspaceAccess, WorkspaceFile } from "./search";

/** Cacheable-answer TTLs (§5.1): short, and shorter while the index moves. */
export const MAP_TTL_WARM_MS = 30_000;
export const MAP_TTL_COLD_MS = 2_000;

/** Symbols kept per file at detail:"outline"; "full" lifts the cap. */
export const MAP_OUTLINE_SYMBOL_CAP = 20;

/** Modules kept per direction in the overview's import-graph summary. */
export const MAP_IMPORT_GRAPH_TOP = 5;

/** Entry points kept on the overview row — a summary, not an inventory. */
export const MAP_ENTRY_POINTS_MAX = 12;

/** Files one map call will ask the index to outline (mirrors the child's
 * MAX_OUTLINE_PATHS bound; the tools layer cannot import the adapter). */
export const MAP_OUTLINE_FILES_MAX = 512;

/** Presentational language census by extension. The indexer's grammar table
 * (adapters/engine-ipc) is the authority for what gets parsed; this only
 * names what the tree shows, and unknown extensions census as themselves. */
const LANGUAGE_BY_EXT: Readonly<Record<string, string>> = {
  sh: "bash",
  bash: "bash",
  c: "c",
  h: "c",
  cc: "cpp",
  cpp: "cpp",
  cxx: "cpp",
  hh: "cpp",
  hpp: "cpp",
  cs: "csharp",
  go: "go",
  java: "java",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  json: "json",
  py: "python",
  rb: "ruby",
  rs: "rust",
  ts: "typescript",
  mts: "typescript",
  cts: "typescript",
  tsx: "tsx",
  md: "markdown",
  toml: "toml",
  yml: "yaml",
  yaml: "yaml",
};

function languageOf(path: string): string | undefined {
  // Extension of the basename only: a dot in a DIRECTORY name must never
  // become a census key, and dotfiles/extensionless files census as nothing.
  const name = path.slice(path.lastIndexOf("/") + 1);
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) return undefined;
  const ext = name.slice(dot + 1).toLowerCase();
  return LANGUAGE_BY_EXT[ext] ?? ext;
}

/** One installed plugin as the inventory records it (§8.4). */
export interface InstalledPlugin {
  readonly name: string;
  readonly version?: string;
  /** How it got here: the signed registry, or sideloaded from disk. */
  readonly source: "registry" | "sideload";
  /** Registry trust tier, when signed. Sideloads never carry one. */
  readonly tier?: "blessed" | "verified";
}

export type PluginTrust = "blessed" | "verified" | "untrusted";

export interface MapPluginEntry {
  readonly name: string;
  readonly version?: string;
  readonly source: "registry" | "sideload";
  readonly trust: PluginTrust;
}

/**
 * §8.4: sideload always works, and is always labeled untrusted — a sideload
 * cannot smuggle a trust tier in. Registry plugins wear their signed tier;
 * a registry entry without one is "verified" (signed, not review-blessed).
 */
export function pluginTrust(plugin: InstalledPlugin): PluginTrust {
  if (plugin.source !== "registry") return "untrusted";
  return plugin.tier ?? "verified";
}

export interface MapDeps {
  readonly index: IndexPort;
  readonly workspace: SearchWorkspaceAccess;
  /** Installed-plugin inventory, read per call by the composing layer. */
  plugins(): Promise<readonly InstalledPlugin[]>;
}

/** Wire shape of one tool call, mirroring schemas/tools/map.input.schema.json. */
export interface MapInput {
  readonly path?: string;
  readonly depth?: number;
  readonly detail?: string;
  readonly budget?: number;
}

export const MAP_DETAILS = ["skeleton", "outline", "full"] as const;
export type MapDetail = (typeof MAP_DETAILS)[number];

export interface MapSymbol {
  readonly name: string;
  readonly kind: string;
  readonly line: number;
  /** First line of the item's doc comment, when it has one (specs/009 R6) —
   * an outline that says what each item is FOR saves opening the file. */
  readonly doc?: string;
}

/** One module in the overview's import-graph summary. */
export interface MapImportModule {
  /** fanOut: a workspace-relative file. fanIn: the import specifier as
   * written in source (extraction is name-resolution-free). */
  readonly module: string;
  readonly count: number;
}

/** Top fan-in/fan-out modules from the Tier-1 imports table (SDD §9.2). */
export interface MapImportGraph {
  readonly fanIn: readonly MapImportModule[];
  readonly fanOut: readonly MapImportModule[];
}

/** How one entry point was detected — manifest fact or walk convention. */
export type MapEntryPointKind = "package-main" | "package-bin" | "cargo-bin" | "convention";

export interface MapEntryPoint {
  /** Workspace-relative path as the manifest or walk names it. */
  readonly path: string;
  readonly kind: MapEntryPointKind;
  /** The bin name, when the manifest gives one. */
  readonly name?: string;
}

/** The first result row: workspace-level facts and the caching contract. */
export interface MapOverview {
  readonly kind: "overview";
  /** The mapped root, workspace-relative ("." is the workspace itself). */
  readonly path: string;
  /** Index health, or null when no index exists at all (pure Tier 0). */
  readonly index: IndexStatus | null;
  /** How long a client may cache this answer, keyed to `index.generation`. */
  readonly ttlMs: number;
  readonly files: number;
  readonly dirs: number;
  /** File count per detected language under the mapped root. */
  readonly languages: Readonly<Record<string, number>>;
  /** Installed plugins with trust labels; sideloads are always untrusted. */
  readonly plugins: readonly MapPluginEntry[];
  /** Import-graph summary; present only when the Tier-1 index answered with
   * at least one edge. Project commands are NOT here — see m3-run-tool. */
  readonly importGraph?: MapImportGraph;
  /** Detected entry points (workspace-level facts, like `plugins`). */
  readonly entryPoints?: readonly MapEntryPoint[];
  /** The Cargo workspace's shape (specs/014); absent outside Cargo workspaces.
   * Describes the WHOLE workspace whatever `path` scoped; the crate rows
   * that follow the overview are the scoped, ranked, capped detail. */
  readonly cargo?: MapCargoSummary;
}

/** One tree row. Rows are breadth-first: shallower entries always precede
 * deeper ones, so a budget cut folds the deep tail, never the skeleton. */
export interface MapEntry {
  readonly kind: "dir" | "file";
  readonly path: string;
  readonly junk?: boolean;
  readonly language?: string;
  /** dir: file count beneath it (all depths), so folded content stays counted. */
  readonly files?: number;
  /** dir: direct subdirectory count. */
  readonly dirs?: number;
  /** file: symbol outline, present when the index answered (detail ≥ outline). */
  readonly symbols?: readonly MapSymbol[];
  /** file: outline entries elided by the detail:"outline" cap. */
  readonly droppedSymbols?: number;
}

export type MapResult = MapOverview | MapCrate | MapEntry;

async function indexStatusOrNull(index: IndexPort): Promise<IndexStatus | null> {
  try {
    return await index.status();
  } catch {
    // No index yet: the honest Tier-0 answer.
    return null;
  }
}

function freshnessOf(status: IndexStatus | null): Freshness {
  if (status === null || status.tier === 0) return "cold";
  return status.building === true ? "warming" : "warm";
}

const FRESHNESS_RANK: Readonly<Record<Freshness, number>> = { cold: 0, warming: 1, warm: 2 };

/** The envelope may never claim fresher than its least-fresh ingredient. */
function worstFreshness(a: Freshness, b: Freshness): Freshness {
  return FRESHNESS_RANK[a] <= FRESHNESS_RANK[b] ? a : b;
}

/** The §5.1 short TTL: cache only while the index is not moving. */
export function mapTtlMs(status: IndexStatus | null): number {
  return status !== null && status.tier === 1 && status.building !== true
    ? MAP_TTL_WARM_MS
    : MAP_TTL_COLD_MS;
}

/** Normalize the input path to a workspace-relative prefix ("" = root). */
function normalizePrefix(path: string | undefined): string {
  const raw = (path ?? ".").replaceAll("\\", "/");
  const parts = raw.split("/").filter((p) => p !== "" && p !== ".");
  return parts.join("/");
}

interface TreeShape {
  readonly rows: MapEntry[];
  /** Paths of the visible file rows, in row order (outline candidates). */
  readonly filePaths: readonly string[];
  readonly files: number;
  readonly dirs: number;
  readonly languages: Record<string, number>;
}

/**
 * Fold the flat walk into breadth-first rows: every directory (any depth up
 * to `depth`) becomes a dir row with recursive file counts; files at or above
 * the depth line become file rows; deeper files exist only in their visible
 * ancestors' counts.
 */
function buildTree(files: readonly WorkspaceFile[], prefix: string, depth: number): TreeShape {
  const scoped: { rel: string; file: WorkspaceFile }[] = [];
  for (const file of files) {
    if (prefix === "") {
      scoped.push({ rel: file.path, file });
    } else if (file.path.startsWith(`${prefix}/`)) {
      scoped.push({ rel: file.path.slice(prefix.length + 1), file });
    }
  }

  const dirFiles = new Map<string, number>();
  const dirChildren = new Map<string, Set<string>>();
  const languages: Record<string, number> = {};
  for (const { rel } of scoped) {
    const segments = rel.split("/");
    for (let i = 1; i < segments.length; i++) {
      const dir = segments.slice(0, i).join("/");
      dirFiles.set(dir, (dirFiles.get(dir) ?? 0) + 1);
      if (i >= 2) {
        const parent = segments.slice(0, i - 1).join("/");
        const children = dirChildren.get(parent) ?? new Set<string>();
        children.add(dir);
        dirChildren.set(parent, children);
      }
    }
    const language = languageOf(rel);
    if (language !== undefined) {
      languages[language] = (languages[language] ?? 0) + 1;
    }
  }

  const topLevelDirs = new Set<string>();
  for (const dir of dirFiles.keys()) {
    const first = dir.split("/")[0];
    if (first !== undefined) topLevelDirs.add(first);
  }

  const rows: MapEntry[] = [];
  const filePaths: string[] = [];
  for (const [dir, count] of dirFiles) {
    if (dir.split("/").length > depth) continue;
    rows.push({
      kind: "dir",
      path: prefix === "" ? dir : `${prefix}/${dir}`,
      files: count,
      dirs: dirChildren.get(dir)?.size ?? 0,
    });
  }
  for (const { rel, file } of scoped) {
    if (rel.split("/").length > depth) continue;
    const language = languageOf(rel);
    rows.push({
      kind: "file",
      path: file.path,
      ...(file.junk ? { junk: true } : {}),
      ...(language === undefined ? {} : { language }),
    });
    filePaths.push(file.path);
  }
  // Breadth-first, dirs before files per level, then lexicographic — stable,
  // and a budget cut always drops the deepest tail first.
  rows.sort((a, b) => {
    const depthA = a.path.split("/").length;
    const depthB = b.path.split("/").length;
    if (depthA !== depthB) return depthA - depthB;
    if (a.kind !== b.kind) return a.kind === "dir" ? -1 : 1;
    return a.path < b.path ? -1 : a.path > b.path ? 1 : 0;
  });

  return { rows, filePaths, files: scoped.length, dirs: dirFiles.size, languages };
}

/** ≈tokens one wire row costs, same estimator the ranking trim uses. */
function rowTokens(row: MapResult): number {
  return Math.ceil(JSON.stringify(row).length / RANKING_WEIGHTS.charsPerToken);
}

interface OutlineAttachment {
  readonly symbols: readonly MapSymbol[];
  readonly droppedSymbols?: number;
}

interface Outlines {
  readonly byPath: ReadonlyMap<string, OutlineAttachment>;
  readonly freshness: Freshness;
}

/** Fetch verified outlines for the visible file rows, capped per detail. */
async function fetchOutlines(
  deps: MapDeps,
  filePaths: readonly string[],
  detail: MapDetail,
  notes: string[],
): Promise<Outlines> {
  const paths = filePaths.slice(0, MAP_OUTLINE_FILES_MAX);
  if (filePaths.length > MAP_OUTLINE_FILES_MAX) {
    notes.push(
      `outlines cover the first ${MAP_OUTLINE_FILES_MAX} of ${filePaths.length} visible files; ` +
        "narrow path or lower depth for full coverage",
    );
  }
  let outline: Awaited<ReturnType<IndexPort["queryOutline"]>>;
  try {
    outline = await deps.index.queryOutline({ paths });
  } catch {
    // The skeleton stands on its own (Constitution I); outlines are enrichment.
    notes.push("symbol outlines unavailable: the index could not answer queryOutline");
    return { byPath: new Map(), freshness: "cold" };
  }
  const cap = detail === "full" ? Number.POSITIVE_INFINITY : MAP_OUTLINE_SYMBOL_CAP;
  const byPath = new Map<string, OutlineAttachment>();
  for (const file of outline.files) {
    if (file.symbols.length === 0) continue;
    const kept = file.symbols.slice(0, cap);
    byPath.set(file.path, {
      symbols: kept.map(
        (symbol: SymbolEntry): MapSymbol => ({
          name: symbol.name,
          kind: symbol.kind,
          line: symbol.location.line,
          ...(symbol.doc === undefined ? {} : { doc: symbol.doc }),
        }),
      ),
      ...(file.symbols.length > kept.length
        ? { droppedSymbols: file.symbols.length - kept.length }
        : {}),
    });
  }
  return { byPath, freshness: outline.freshness };
}

interface ImportGraphFetch {
  readonly graph?: MapImportGraph;
  /** Per-crate edges when the index answered them (specs/014 R6). */
  readonly crates?: readonly CrateImportEdge[];
  readonly freshness: Freshness;
}

/** Fetch the overview's import-graph summary — and, for a Cargo workspace,
 * the per-crate edges in the SAME call; absence is degradation, never an
 * error (Constitution I — the skeleton stands on its own). */
async function fetchImportGraph(
  deps: MapDeps,
  notes: string[],
  cargo: CargoWorkspaceModel | undefined,
): Promise<ImportGraphFetch> {
  const scopes = cargo === undefined ? [] : crateScopes(cargo);
  let result: Awaited<ReturnType<IndexPort["queryImportGraph"]>>;
  try {
    result = await deps.index.queryImportGraph({
      limit: MAP_IMPORT_GRAPH_TOP,
      ...(scopes.length === 0 ? {} : { crates: scopes }),
    });
  } catch {
    notes.push("import-graph summary unavailable: the index could not answer queryImportGraph");
    if (scopes.length > 0) {
      notes.push("use counts unavailable: the index could not answer per-crate import edges");
    }
    return { freshness: "cold" };
  }
  if (scopes.length > 0 && result.crates === undefined) {
    notes.push("use counts unavailable: the index answered no per-crate import edges");
  }
  const crates = result.crates === undefined ? {} : { crates: result.crates };
  if (result.fanIn.length === 0 && result.fanOut.length === 0) {
    return { ...crates, freshness: result.freshness };
  }
  const entry = (row: { module: string; count: number }): MapImportModule => ({
    module: row.module,
    count: row.count,
  });
  return {
    graph: { fanIn: result.fanIn.map(entry), fanOut: result.fanOut.map(entry) },
    ...crates,
    freshness: result.freshness,
  };
}

/** `src/main.<ext>` — the cross-language convention row (covers Cargo's
 * default bin at src/main.rs without reading Cargo.toml). */
const ENTRY_CONVENTION = /^src\/main\.[^/.]+$/;

/** Manifest paths are written "./dist/cli.js"-style; the walk is bare. */
function normalizeManifestPath(path: string): string {
  return path.startsWith("./") ? path.slice(2) : path;
}

function packageEntryPoints(manifest: string): MapEntryPoint[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(manifest);
  } catch {
    return [];
  }
  if (typeof parsed !== "object" || parsed === null) return [];
  const record = parsed as Record<string, unknown>;
  const entries: MapEntryPoint[] = [];
  if (typeof record["main"] === "string") {
    entries.push({ path: normalizeManifestPath(record["main"]), kind: "package-main" });
  }
  const bin = record["bin"];
  const packageName = typeof record["name"] === "string" ? record["name"] : undefined;
  if (typeof bin === "string") {
    entries.push({
      path: normalizeManifestPath(bin),
      kind: "package-bin",
      ...(packageName === undefined ? {} : { name: packageName }),
    });
  } else if (typeof bin === "object" && bin !== null) {
    for (const [name, target] of Object.entries(bin)) {
      if (typeof target !== "string") continue;
      entries.push({ path: normalizeManifestPath(target), kind: "package-bin", name });
    }
  }
  return entries;
}

/** Minimal [[bin]] extraction — name/path keys per section, no TOML parser.
 * A pathless [[bin]] follows Cargo's src/bin/<name>.rs convention. */
function cargoEntryPoints(manifest: string): MapEntryPoint[] {
  const entries: MapEntryPoint[] = [];
  const sections = manifest.split(/^\s*\[/m);
  for (const section of sections) {
    if (!section.startsWith("[bin]]")) continue;
    const name = /^\s*name\s*=\s*"([^"]+)"/m.exec(section)?.[1];
    const path = /^\s*path\s*=\s*"([^"]+)"/m.exec(section)?.[1];
    if (name === undefined && path === undefined) continue;
    entries.push({
      path: normalizeManifestPath(path ?? `src/bin/${name}.rs`),
      kind: "cargo-bin",
      ...(name === undefined ? {} : { name }),
    });
  }
  return entries;
}

/**
 * Detect entry points from workspace-root manifests and the walk (SDD §9.2).
 * Pure Tier-0 facts: manifests are read only when the walk saw them, parse
 * failures detect nothing, and nothing here consults the index.
 */
async function detectEntryPoints(
  deps: MapDeps,
  files: readonly WorkspaceFile[],
): Promise<readonly MapEntryPoint[]> {
  const present = new Set(files.map((file) => file.path));
  const readManifest = async (path: string): Promise<string | null> => {
    if (!present.has(path)) return null;
    try {
      return await deps.workspace.read(path);
    } catch {
      return null;
    }
  };
  const entries: MapEntryPoint[] = [];
  const packageManifest = await readManifest("package.json");
  if (packageManifest !== null) entries.push(...packageEntryPoints(packageManifest));
  const cargoManifest = await readManifest("Cargo.toml");
  if (cargoManifest !== null) entries.push(...cargoEntryPoints(cargoManifest));
  for (const file of files) {
    if (ENTRY_CONVENTION.test(file.path)) {
      entries.push({ path: file.path, kind: "convention" });
    }
  }
  const seen = new Set<string>();
  const deduped = entries.filter((entry) => {
    if (seen.has(entry.path)) return false;
    seen.add(entry.path);
    return true;
  });
  return deduped.slice(0, MAP_ENTRY_POINTS_MAX);
}

/**
 * Answer one map call with a ResponseEnvelope. The caller negotiates the
 * envelope version and seals (redacts) the result before it reaches a wire.
 */
export async function runMap(
  deps: MapDeps,
  input: MapInput,
  v: number,
): Promise<ResponseEnvelope<MapResult>> {
  const budget = input.budget ?? 6000;
  const depth = Math.min(Math.max(input.depth ?? 3, 1), 16);
  const detail: MapDetail = (MAP_DETAILS as readonly string[]).includes(input.detail ?? "")
    ? (input.detail as MapDetail)
    : "outline";
  const prefix = normalizePrefix(input.path);
  const notes: string[] = [];

  const status = await indexStatusOrNull(deps.index);
  let freshness = freshnessOf(status);

  const allFiles = await deps.workspace.list();
  const tree = buildTree(allFiles, prefix, depth);
  if (tree.files === 0 && prefix !== "") {
    notes.push(`no files under ${JSON.stringify(prefix)}; is the path workspace-relative?`);
  }

  const entryPoints = await detectEntryPoints(deps, allFiles);

  // The Cargo workspace shape (specs/014): Tier-0 manifest facts through the
  // walk; the index adds importing-file counts when it is up.
  const cargoModel = await readCargoWorkspace(
    allFiles.map((file) => file.path),
    (path) => deps.workspace.read(path),
  );

  let importGraph: MapImportGraph | undefined;
  let crateEdges: readonly CrateImportEdge[] | undefined;
  if (status !== null && status.tier === 1) {
    const fetched = await fetchImportGraph(deps, notes, cargoModel);
    freshness = worstFreshness(freshness, fetched.freshness);
    importGraph = fetched.graph;
    crateEdges = fetched.crates;
  }
  const cargo = cargoModel === undefined ? undefined : renderCargo(cargoModel, prefix, crateEdges);
  if (cargo !== undefined) notes.push(...cargo.notes);

  let treeRows: readonly MapEntry[] = tree.rows;
  if (detail !== "skeleton" && status !== null && status.tier === 1) {
    const outlines = await fetchOutlines(deps, tree.filePaths, detail, notes);
    freshness = worstFreshness(freshness, outlines.freshness);
    treeRows = tree.rows.map((row) => {
      const attachment = row.kind === "file" ? outlines.byPath.get(row.path) : undefined;
      return attachment === undefined ? row : { ...row, ...attachment };
    });
  } else if (detail !== "skeleton") {
    notes.push(
      "skeleton from the live filesystem; symbol outlines appear once the Tier-1 index is up",
    );
  }

  let plugins: readonly InstalledPlugin[];
  try {
    plugins = await deps.plugins();
  } catch {
    plugins = [];
    notes.push("plugin inventory unreadable; no plugins are shown");
  }
  const pluginEntries = plugins.map(
    (plugin): MapPluginEntry => ({
      name: plugin.name,
      ...(plugin.version === undefined ? {} : { version: plugin.version }),
      source: plugin.source,
      trust: pluginTrust(plugin),
    }),
  );
  const untrusted = pluginEntries.filter((p) => p.trust === "untrusted");
  if (untrusted.length > 0) {
    notes.push(
      `${untrusted.length} sideloaded plugin(s) are untrusted: ` +
        `${untrusted.map((p) => p.name).join(", ")} (SDD §8.4 — tightest capability defaults)`,
    );
  }

  // What the symbol index CANNOT see, said where the agent reads it
  // (specs/009 R9). Derive and `macro_rules!` output exists only after
  // expansion, which no tree-sitter parse performs, so an agent that treats
  // the index as complete would conclude a generated item does not exist.
  // Gated on the census: silence in workspaces the note does not describe.
  if ((tree.languages["rust"] ?? 0) > 0 && status !== null) {
    notes.push(
      "macro-generated items (derives, macro_rules!) are not in the symbol index: " +
        "tree-sitter sees source, not expansion — use search mode:lexical or " +
        "mode:references for them",
    );
  }

  const overview: MapOverview = {
    kind: "overview",
    path: prefix === "" ? "." : prefix,
    index: status,
    ttlMs: mapTtlMs(status),
    files: tree.files,
    dirs: tree.dirs,
    languages: tree.languages,
    plugins: pluginEntries,
    ...(importGraph === undefined ? {} : { importGraph }),
    ...(entryPoints.length === 0 ? {} : { entryPoints }),
    ...(cargo === undefined ? {} : { cargo: cargo.summary }),
  };

  // Crate rows sit between the overview and the tree: workspace-level facts,
  // ranked by dependents, so a budget cut keeps the load-bearing crates.
  const rows: MapResult[] = [overview, ...(cargo?.rows ?? []), ...treeRows];
  let used = 0;
  let keep = 0;
  for (const row of rows) {
    const cost = rowTokens(row);
    if (used + cost > budget) break;
    used += cost;
    keep += 1;
  }
  if (keep === 0) {
    return taxonomyError({
      v,
      code: "BUDGET_TOO_SMALL",
      message:
        `budget ${budget} cannot fit even the overview row ` +
        `(≈${rowTokens(overview)} tokens); raise the budget`,
    });
  }
  const cursorRow = rows[keep];
  const cut = truncateResults(rows, keep, {
    reason: "budget",
    narrow: ["lower depth", "narrow path", 'detail: "skeleton"', "raise budget"],
    cursor:
      cursorRow === undefined || cursorRow.kind === "overview" ? "-" : `path:${cursorRow.path}`,
  });

  return okEnvelope<MapResult>({
    v,
    freshness,
    budget: { requested: budget, used },
    results: cut,
    ...(notes.length === 0 ? {} : { notes }),
  });
}
