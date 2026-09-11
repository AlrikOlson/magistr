/**
 * IndexPort — the tiered index behind search and map (SDD §6.4).
 *
 * Tier 0 is cold-FS (no index at all); Tier 1 is tree-sitter symbols. The
 * port never blocks on a warm index — that refusal shape (`INDEX_UNAVAILABLE`,
 * recovery `retry`) belongs to the tool layer per Constitution I. `build` is
 * a non-blocking kick: it starts (or reports) a Tier-1 build and returns the
 * current status immediately, so Tier-0 answers stay available while the
 * index warms (SDD §7.3.2).
 */

import type { Freshness } from "../envelope";
import type { SourceLocation } from "./types";
import type { PORT_VERSIONS } from "./versions";

export type IndexTier = 0 | 1;

export interface IndexStatus {
  readonly tier: IndexTier;
  /** Monotonic index generation; bumps on every applied re-index. */
  readonly generation: number;
  /** True while a Tier-1 build is in flight. */
  readonly building?: boolean;
  /** Files the current/last build discovered. */
  readonly filesTotal?: number;
  /** Files the current/last build has committed. */
  readonly filesIndexed?: number;
  /** Epoch ms the current/last build was kicked; absent when no build ran this process. */
  readonly buildStartedAtMs?: number;
  /** Epoch ms that build last made progress (batch commit or settle). */
  readonly buildUpdatedAtMs?: number;
  /** The last build's failure, verbatim; the index stays at its prior tier. */
  readonly lastError?: string;
}

export interface SymbolEntry {
  readonly name: string;
  readonly kind: string;
  readonly location: SourceLocation;
  /** 1-based inclusive last line of the declaration, when the index knows it. */
  readonly endLine?: number;
  /**
   * Qualification the address dialect does not carry — the trait a Rust impl
   * member came from, so `Type::member` stays the typeable address while an
   * ambiguity can still say `<Type as Trait>::member` (specs/009 R2).
   */
  readonly detail?: string;
  /** First line of the item's doc comment, when it has one (specs/009 R6). */
  readonly doc?: string;
}

/** One inclusive 1-based range of test-only code inside a file — a Rust
 * `#[cfg(test)]` module. Hits inside one rank as test code even though the
 * file itself is source (specs/009 R4). */
export interface TestRange {
  readonly startLine: number;
  readonly endLine: number;
}

export interface SymbolQuery {
  /** Case-insensitive symbol-name query. */
  readonly name: string;
  readonly kind?: string;
}

export interface IndexedFile {
  readonly path: string;
  readonly symbols: readonly SymbolEntry[];
}

/**
 * A symbol query answer plus the freshness tier it was verified at
 * (SDD §7.3.3): `warm` only when every result file's mtime+size matched the
 * index (or was synchronously re-parsed), `warming` while a build is in
 * flight, `cold` when the index is absent or a file could not be verified.
 */
export interface SymbolQueryResult {
  readonly symbols: readonly SymbolEntry[];
  readonly freshness: Freshness;
}

/** How a reference edge was derived, and how much to trust it (SDD §14). */
export type ReferenceConfidence = "high" | "medium" | "low";

/**
 * Where a reference edge came from (SDD §14, §8.3). The `tier1-*` values are
 * name-resolution-free tree-sitter derivations; `resolver-lsp` marks an edge
 * verified by the resolver plugin's language server (Tier-2,
 * chunk:m5-lsp-resolver-plugin) — the one value a symbolic lane must never
 * claim.
 */
export type ReferenceProvenance = "resolver-lsp" | "tier1-symbols" | "tier1-imports" | "tier1-text";

/**
 * One reference edge. Tree-sitter references are name-resolution-free, so
 * every edge carries its confidence provenance: `high`/tier1-symbols is a
 * definition row, `medium`/tier1-imports an occurrence in a file
 * import-linked to a defining module, `low`/tier1-text a bare name match.
 */
export interface ReferenceEdge {
  readonly location: SourceLocation;
  readonly excerpt: string;
  readonly refKind: "definition" | "use";
  readonly confidence: ReferenceConfidence;
  readonly provenance: ReferenceProvenance;
}

export interface ReferenceQuery {
  /** Exact identifier to trace (case-insensitive for definitions). */
  readonly name: string;
}

/** A reference answer plus the freshness tier it was verified at (§7.3.3). */
export interface ReferenceQueryResult {
  readonly edges: readonly ReferenceEdge[];
  readonly freshness: Freshness;
}

export interface OutlineQuery {
  /** Workspace-relative paths to outline; files without symbols come back empty. */
  readonly paths: readonly string[];
}

/** Per-file symbol outlines plus the freshness tier they were verified at. */
export interface OutlineQueryResult {
  readonly files: readonly IndexedFile[];
  readonly freshness: Freshness;
}

export interface ImportGraphQuery {
  /** Modules kept per direction (top-N by edge count). */
  readonly limit?: number;
  /**
   * v9 (specs/014 R6): ask for per-crate import edges as well. One scope per
   * workspace member — the name `use` paths spell and the member directory.
   * Adapters that record no imports simply omit `crates` from the answer.
   */
  readonly crates?: readonly CrateScope[];
}

/** One workspace member the crate-level import graph is asked about. */
export interface CrateScope {
  /** The crate's `use` name (hyphens already underscored). */
  readonly name: string;
  /** Member directory, workspace-relative; "." for a root package. */
  readonly dir: string;
}

/**
 * Importing files per (importer member, imported crate): the importer is the
 * longest scope directory prefixing the importing path ("." catches the
 * rest). A count has no mtime to verify, so these rows carry the index's
 * live freshness rather than a per-path verification (specs/014 R7).
 */
export interface CrateImportEdge {
  readonly from: string;
  readonly to: string;
  readonly files: number;
}

/** One module in the import-graph summary, with its edge count. */
export interface ImportGraphEntry {
  /** Fan-out rows name a workspace-relative file; fan-in rows name an import
   * target as written in source (a specifier, not necessarily a file). */
  readonly module: string;
  readonly count: number;
}

/**
 * The import-graph summary behind map's overview (SDD §9.2): the modules
 * with the most outgoing imports (fan-out, keyed by indexed file) and the
 * targets imported most often (fan-in, keyed by the raw import specifier —
 * tree-sitter extraction is name-resolution-free, so specifiers are the
 * honest unit). Fan-out paths are files and are freshness-verified;
 * fan-in specifiers have no mtime to check.
 */
export interface ImportGraphResult {
  readonly fanIn: readonly ImportGraphEntry[];
  readonly fanOut: readonly ImportGraphEntry[];
  /** Present iff the query asked and the adapter records imports (v9). */
  readonly crates?: readonly CrateImportEdge[];
  readonly freshness: Freshness;
}

export interface IndexPort {
  readonly version: typeof PORT_VERSIONS.index;
  status(): Promise<IndexStatus>;
  /** Kick a Tier-1 build of `root`; never blocks until warm. */
  build(root: string): Promise<IndexStatus>;
  /** Answer verified against the filesystem before return — never stale. */
  querySymbols(query: SymbolQuery): Promise<SymbolQueryResult>;
  /** Reference edges with confidence provenance, verified like querySymbols. */
  queryReferences(query: ReferenceQuery): Promise<ReferenceQueryResult>;
  /** Per-file symbol outlines (map's structural overview), verified likewise. */
  queryOutline(query: OutlineQuery): Promise<OutlineQueryResult>;
  /** Top fan-in/fan-out modules from the imports table (map's overview). */
  queryImportGraph(query: ImportGraphQuery): Promise<ImportGraphResult>;
  /**
   * Test-only line ranges per file (Rust `#[cfg(test)]` modules), for files
   * that have any. Ranking uses them to class a hit inside one as test code
   * even when its file is source (specs/009 R4). Optional: an adapter that
   * cannot answer simply omits it, and ranking falls back to per-file rules.
   */
  queryTestRanges?(
    paths: readonly string[],
  ): Promise<Readonly<Record<string, readonly TestRange[]>>>;
  upsertFile(file: IndexedFile): Promise<void>;
  /** Re-index one file from its current on-disk content (the watcher seam). */
  refreshFile(path: string): Promise<void>;
  removeFile(path: string): Promise<void>;
}
