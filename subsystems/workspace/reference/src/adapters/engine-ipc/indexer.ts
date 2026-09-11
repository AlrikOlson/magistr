/**
 * The engine child's tiered indexer (SDD §7.3). Owns `<root>/.workbench/
 * index.db` — the child's single SQLite writer: every symbol, import and
 * FTS5 row commits in batched transactions on this one connection, in this
 * one process, where the single-writer invariant is actually enforceable
 * (SDD §6.1).
 *
 * Tier 0 is the absence of all of this: lexical answers straight off the
 * filesystem. A build is therefore a background promise — `index/build`
 * returns immediately and the frame loop keeps answering while workers parse
 * (`core.extract` runs on the napi worker pool, one parser arena per task).
 *
 * Junk (vendor/generated/lockfiles) is flagged, never excluded: junk files
 * get files rows, symbols and FTS content like everything else, and ranking
 * (§7.4) demotes them. Gitignored files are not walked at all — grep parity;
 * a path-targeted query still reaches them cold at Tier 0.
 */

import type { Database } from "bun:sqlite";
import { mkdirSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { openVersionedDatabase } from "../../data/versioned-db";
import type { CoreBinding, CoreExtraction, CoreScanEntry } from "../core";

export const INDEX_METHODS = [
  "index/build",
  "index/status",
  "index/symbols",
  "index/outline",
  "index/references",
  "index/importGraph",
  "index/upsert",
  "index/remove",
  "index/testRanges",
] as const;
export type IndexMethod = (typeof INDEX_METHODS)[number];

export function isIndexMethod(method: string): method is IndexMethod {
  return (INDEX_METHODS as readonly string[]).includes(method);
}

/**
 * Stamped as `PRAGMA user_version`; the migration runner (SDD §10.2) owns
 * evolution. Version 3 (specs/017): a file's `content_fts` row carries its
 * `files` row id, so a version-2 index (arbitrary FTS row ids) must be
 * discarded and rebuilt, never adopted — rowid-addressed deletes over it
 * would remove the wrong content rows.
 */
export const INDEX_SCHEMA_VERSION = 3;

/** Files above this size get a files row but no symbols/FTS content. */
export const MAX_CONTENT_BYTES = 2 * 1024 * 1024;

/** How many files commit per SQLite transaction during a build. */
export const COMMIT_BATCH_SIZE = 64;

/** How many extractions run concurrently on the napi worker pool. */
export const EXTRACT_CONCURRENCY = 8;

/**
 * How many mismatched files one query will synchronously re-parse before
 * falling back to dropping the file's rows and answering `cold` (SDD §7.3.3:
 * the re-parse is bounded so a query cannot degenerate into a rebuild).
 */
export const VERIFY_REPARSE_BOUND = 8;

/** Paths one outline query will answer (and mtime+size-verify) at a time. */
export const MAX_OUTLINE_PATHS = 512;

/** Candidate files one reference query will line-scan (keeps p95 bounded). */
export const MAX_REFERENCE_FILES = 200;

/** Reference edges kept per file before the rest are elided. */
export const MAX_REFERENCE_EDGES_PER_FILE = 50;

/** Total occurrence edges one reference answer carries (definitions always
 * ride on top; the tool's budget truncation cuts further for the wire). */
export const MAX_REFERENCE_EDGES_TOTAL = 1000;

/** Same failure contract as the child's other methods: a typed, answerable error. */
export class IndexMethodError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "IndexMethodError";
  }
}

/**
 * §7.7 cancellation token: `true` once the request's cancel frame arrived.
 * Long-running queries check it between parse units and inside scan
 * iteration so a cancelled request actually stops burning the engine.
 */
export type CancelToken = () => boolean;

function throwIfCancelled(isCancelled: CancelToken | undefined): void {
  if (isCancelled?.() === true) {
    throw new IndexMethodError("REQUEST_CANCELLED", "request cancelled mid-query (§7.7)");
  }
}

const EXT_LANGUAGE: Readonly<Record<string, string>> = {
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
};

/**
 * Registered lang-pack extensions (chunk:m5-lang-pack), maintained by the
 * child's sandbox/langPack handler on successful registration. Consulted
 * strictly AFTER the bundled table — the registry refuses bundled shadowing
 * (FR-005), and this lookup order makes that structural on the TS side too.
 */
const PACK_EXT_LANGUAGE = new Map<string, string>();

/** Record one registered pack language's extension claims. */
export function registerPackLanguage(id: string, extensions: readonly string[]): void {
  for (const ext of extensions) {
    const key = ext.toLowerCase();
    if (EXT_LANGUAGE[key] === undefined && !PACK_EXT_LANGUAGE.has(key)) {
      PACK_EXT_LANGUAGE.set(key, id);
    }
  }
}

/** Drop every pack extension claim (re-registration, tests). */
export function clearPackLanguages(): void {
  PACK_EXT_LANGUAGE.clear();
}

/** Language id for a path — bundled table first, then registered packs;
 * undefined for non-source files. */
export function languageForPath(path: string): string | undefined {
  const ext = path.slice(path.lastIndexOf(".") + 1).toLowerCase();
  return EXT_LANGUAGE[ext] ?? PACK_EXT_LANGUAGE.get(ext);
}

export interface ChildIndexStatus {
  readonly tier: 0 | 1;
  readonly generation: number;
  readonly building: boolean;
  readonly filesTotal: number;
  readonly filesIndexed: number;
  /** Present when the last build failed; the index stays at its prior tier. */
  readonly error?: string;
  /** Epoch ms the current/last build was kicked — the index/build task's createdAt. */
  readonly buildStartedAtMs?: number;
  /** Epoch ms of the last committed batch (or settle) of that build. */
  readonly buildUpdatedAtMs?: number;
}

/** Mirror of the envelope's freshness tiers — the child does not import the
 * envelope layer; the wire shape is what the server-side adapter types. */
export type IndexFreshness = "cold" | "warming" | "warm";

export interface VerifiedSymbols {
  readonly symbols: readonly SymbolRow[];
  readonly freshness: IndexFreshness;
}

/** One outlined file on the wire: its path and its symbol rows, in file order. */
export interface OutlineFileRows {
  readonly path: string;
  readonly symbols: readonly SymbolRow[];
}

export interface VerifiedOutline {
  readonly files: readonly OutlineFileRows[];
  readonly freshness: IndexFreshness;
}

interface SymbolRow {
  readonly name: string;
  readonly kind: string;
  readonly path: string;
  readonly start_line: number;
  readonly start_col: number;
  readonly end_line: number;
  /** Qualification the address dialect does not carry — the trait a Rust impl
   * member came from (specs/009 R2). NULL/absent for everything else. */
  readonly detail?: string | null;
  /** First line of the item's doc comment (specs/009 R6). NULL/absent when
   * the item has none; the port's upsert seam may omit it entirely. */
  readonly doc?: string | null;
}

/** One inclusive 1-based range of test-only code inside a source file — a
 * Rust `#[cfg(test)]` module (specs/009 R4). */
interface TestRangeRow {
  readonly path: string;
  readonly start_line: number;
  readonly end_line: number;
}

/**
 * One reference edge, with its confidence provenance (SDD §8.3, §14):
 * tree-sitter references are name-resolution-free, so every edge says how it
 * was derived and how much to trust it — the answer never overclaims.
 */
export interface ReferenceRow {
  readonly path: string;
  readonly line: number;
  readonly col: number;
  readonly excerpt: string;
  /** `definition` (a symbols row) or `use` (an occurrence). */
  readonly ref_kind: "definition" | "use";
  /** high = Tier-1 symbol row; medium = import-linked file; low = name match. */
  readonly confidence: "high" | "medium" | "low";
  readonly provenance: "tier1-symbols" | "tier1-imports" | "tier1-text";
}

export interface VerifiedReferences {
  readonly edges: readonly ReferenceRow[];
  readonly freshness: IndexFreshness;
}

/** One module in the import-graph summary: fan-out rows name an indexed file,
 * fan-in rows name an import target as written in source (a specifier). */
export interface ImportGraphRow {
  readonly module: string;
  readonly count: number;
}

/** One workspace member the crate-level graph is asked about (specs/014 R6). */
export interface CrateScopeRow {
  readonly name: string;
  readonly dir: string;
}

/** Importing files per (importer member dir, imported crate use-name). */
export interface CrateEdgeRow {
  readonly from: string;
  readonly to: string;
  readonly files: number;
}

export interface VerifiedImportGraph {
  readonly fanIn: readonly ImportGraphRow[];
  readonly fanOut: readonly ImportGraphRow[];
  /** Present iff the call asked with `crates`. */
  readonly crates?: readonly CrateEdgeRow[];
  readonly freshness: IndexFreshness;
}

/** Scopes answered per call — a workspace, not an inventory. */
export const CRATE_SCOPES_MAX = 256;

/** Modules kept per direction when `index/importGraph` gives no limit. */
export const IMPORT_GRAPH_DEFAULT_LIMIT = 10;
/** Hard bound on the per-direction limit — the summary is a summary. */
export const IMPORT_GRAPH_MAX_LIMIT = 50;

const JUNK_DIRS = new Set([
  "node_modules",
  "vendor",
  "third_party",
  "dist",
  "target",
  "__pycache__",
  ".venv",
  "venv",
  ".next",
  "coverage",
]);
const JUNK_FILES = new Set([
  "package-lock.json",
  "yarn.lock",
  "pnpm-lock.yaml",
  "bun.lock",
  "bun.lockb",
  "Cargo.lock",
  "composer.lock",
  "Gemfile.lock",
  "go.sum",
  "poetry.lock",
  "uv.lock",
]);
const JUNK_SUFFIXES = [".min.js", ".min.css", ".map", ".snap"];
const EXCLUDED_DIRS = new Set([".git", ".workbench"]);

function isJunkPath(relPath: string): boolean {
  const parts = relPath.split("/");
  const file = parts[parts.length - 1] ?? "";
  if (parts.slice(0, -1).some((dir) => JUNK_DIRS.has(dir))) return true;
  return JUNK_FILES.has(file) || JUNK_SUFFIXES.some((s) => file.endsWith(s));
}

interface GitignoreRule {
  /** Compiled matcher for the glob pattern (`*` → segment, `**` → any). */
  readonly regex: RegExp;
  /** Pattern contains a `/` → anchored to the .gitignore's directory. */
  readonly anchored: boolean;
  readonly dirOnly: boolean;
}

/** One directory's .gitignore rules, applied to paths relative to `baseRel`. */
interface GitignoreScope {
  /** Workspace-relative directory holding the .gitignore ("" = root). */
  readonly baseRel: string;
  readonly rules: readonly GitignoreRule[];
}

/** Compile one gitignore glob into a full-match RegExp. */
function compileGitignoreGlob(pattern: string): RegExp {
  let out = "";
  for (let i = 0; i < pattern.length; i++) {
    const ch = pattern[i];
    if (ch === "*") {
      if (pattern[i + 1] === "*") {
        out += ".*";
        i++;
      } else {
        out += "[^/]*";
      }
    } else if (ch === "?") {
      out += "[^/]";
    } else {
      out += (ch as string).replace(/[.+^${}()|[\]\\]/g, "\\$&");
    }
  }
  return new RegExp(`^${out}$`);
}

/**
 * Parse one .gitignore file: comments and blanks dropped, `dir/` suffixes,
 * `*`/`?`/`**` globs, and leading-`/` (or embedded-`/`) anchoring. Negation
 * (`!`) lines are deliberately not honored — the one remaining honest limit
 * of the fallback; the native path uses the `ignore` crate.
 */
function parseGitignoreFile(file: string): GitignoreRule[] {
  let text: string;
  try {
    text = readFileSync(file, "utf8");
  } catch {
    return [];
  }
  const rules: GitignoreRule[] = [];
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (line === "" || line.startsWith("#") || line.startsWith("!")) continue;
    const dirOnly = line.endsWith("/");
    const trimmed = line.replace(/\/+$/, "");
    const anchored = trimmed.includes("/");
    const pattern = trimmed.replace(/^\/+/, "");
    if (pattern === "") continue;
    rules.push({ regex: compileGitignoreGlob(pattern), anchored, dirOnly });
  }
  return rules;
}

/**
 * Does any scope ignore `relPath`? A rule sees the path relative to its own
 * .gitignore's directory: anchored rules (containing `/`) match that whole
 * relative path; bare rules match the basename at any depth below it.
 */
function isIgnored(scopes: readonly GitignoreScope[], relPath: string, isDir: boolean): boolean {
  const name = relPath.split("/").pop() ?? relPath;
  for (const scope of scopes) {
    let sub: string;
    if (scope.baseRel === "") {
      sub = relPath;
    } else if (relPath.startsWith(`${scope.baseRel}/`)) {
      sub = relPath.slice(scope.baseRel.length + 1);
    } else {
      continue;
    }
    for (const rule of scope.rules) {
      if (rule.dirOnly && !isDir) continue;
      if (rule.regex.test(rule.anchored ? sub : name)) return true;
    }
  }
  return false;
}

/**
 * Pure-TS workspace walk: the degraded path when the core cannot scan (the
 * wasm fallback without full fs access) and the stub core's implementation.
 * Honors gitignore semantics including nested .gitignore files; negation
 * lines are the one remaining honest limit of the fallback (the native path
 * uses the `ignore` crate with full gitignore semantics).
 */
export function fallbackScanWorkspace(root: string): CoreScanEntry[] {
  const entries: CoreScanEntry[] = [];
  const walk = (dir: string, rel: string, scopes: readonly GitignoreScope[]): void => {
    const rules = parseGitignoreFile(join(dir, ".gitignore"));
    const here = rules.length === 0 ? scopes : [...scopes, { baseRel: rel, rules }];
    for (const name of readdirSync(dir)) {
      const relPath = rel === "" ? name : `${rel}/${name}`;
      const absPath = join(dir, name);
      let stats: ReturnType<typeof statSync>;
      try {
        stats = statSync(absPath);
      } catch {
        continue;
      }
      const isDir = stats.isDirectory();
      if (isDir && EXCLUDED_DIRS.has(name)) continue;
      if (isIgnored(here, relPath, isDir)) continue;
      if (isDir) {
        walk(absPath, relPath, here);
      } else if (stats.isFile()) {
        entries.push({
          path: relPath,
          size: stats.size,
          mtimeMs: Math.trunc(stats.mtimeMs),
          junk: isJunkPath(relPath),
        });
      }
    }
  };
  walk(root, "", []);
  entries.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  return entries;
}

/** NUL in the first 8KB means binary — no FTS5 row, no parse. */
function looksBinary(bytes: Uint8Array): boolean {
  const window = bytes.subarray(0, 8192);
  return window.includes(0);
}

interface FileWork {
  readonly entry: CoreScanEntry;
  readonly language: string | undefined;
  readonly content: string | null;
  readonly extraction: CoreExtraction | null;
}

export class ChildIndexer {
  private readonly db: Database;
  private generation: number;
  /** Files the last build skipped as unchanged (diagnostics/tests). */
  reusedLastBuild = 0;
  private tier: 0 | 1;
  private building = false;
  private filesTotal = 0;
  private filesIndexed = 0;
  private lastError: string | undefined;
  private buildPromise: Promise<void> | null = null;
  // Build-lifecycle timestamps live with the build state they describe: in
  // memory, in the engine child. After a restart there is no build to report,
  // so the derived index/build task honestly disappears with them.
  private buildStartedAtMs: number | undefined;
  private buildUpdatedAtMs: number | undefined;

  constructor(
    readonly root: string,
    private readonly core: CoreBinding,
    journalMode: "WAL" | "TRUNCATE" = "WAL",
  ) {
    const stateDir = join(root, ".workbench");
    mkdirSync(stateDir, { recursive: true });
    // Cache-class (SDD §10.2): the index is a cache of the filesystem, so a
    // newer, unreadable or unmigratable database is discarded and rebuilt —
    // the build machinery below re-parses the repo from truth.
    this.db = openVersionedDatabase({
      path: join(stateDir, "index.db"),
      dataClass: "cache",
      version: INDEX_SCHEMA_VERSION,
      journalMode,
      // No migration chain (specs/009 R7). Schema 2 added columns and a table (and schema 3 aligns FTS row ids)
      // that a version-0/1 database does not have, and adopting such a
      // database would leave those absent — a silently half-shaped index.
      // Cache-class means the honest answer is to discard and re-parse the
      // repo, which `openVersionedDatabase` does when no chain reaches the
      // current version.
      migrations: [],
      bootstrap: (db) => {
        db.run("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
        db.run(
          "CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, language TEXT, " +
            "size INTEGER NOT NULL, mtime_ms INTEGER NOT NULL, junk INTEGER NOT NULL, " +
            "content_indexed INTEGER NOT NULL)",
        );
        db.run(
          "CREATE TABLE IF NOT EXISTS symbols (path TEXT NOT NULL, name TEXT NOT NULL, " +
            "kind TEXT NOT NULL, start_line INTEGER NOT NULL, start_col INTEGER NOT NULL, " +
            "end_line INTEGER NOT NULL, detail TEXT, doc TEXT)",
        );
        db.run("CREATE INDEX IF NOT EXISTS symbols_name ON symbols (name COLLATE NOCASE)");
        db.run("CREATE INDEX IF NOT EXISTS symbols_path ON symbols (path)");
        // Test-only regions inside otherwise-source files (specs/009 R4).
        db.run(
          "CREATE TABLE IF NOT EXISTS test_ranges (path TEXT NOT NULL, " +
            "start_line INTEGER NOT NULL, end_line INTEGER NOT NULL)",
        );
        db.run("CREATE INDEX IF NOT EXISTS test_ranges_path ON test_ranges (path)");
        db.run("CREATE TABLE IF NOT EXISTS imports (path TEXT NOT NULL, target TEXT NOT NULL)");
        db.run("CREATE INDEX IF NOT EXISTS imports_path ON imports (path)");
        db.run(
          "CREATE VIRTUAL TABLE IF NOT EXISTS content_fts USING fts5(path UNINDEXED, content)",
        );
      },
    }).db;
    this.db.run("PRAGMA synchronous = NORMAL");
    this.generation = Number(this.getMeta("generation") ?? "0");
    this.tier = this.getMeta("tier") === "1" ? 1 : 0;
  }

  private getMeta(key: string): string | undefined {
    const row = this.db.query("SELECT value FROM meta WHERE key = ?").get(key) as {
      value: string;
    } | null;
    return row?.value;
  }

  private setMeta(key: string, value: string): void {
    this.db.run(
      "INSERT INTO meta (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
      [key, value],
    );
  }

  private bumpGeneration(): void {
    this.generation += 1;
    this.setMeta("generation", String(this.generation));
  }

  status(): ChildIndexStatus {
    return {
      tier: this.tier,
      generation: this.generation,
      building: this.building,
      filesTotal: this.filesTotal,
      filesIndexed: this.filesIndexed,
      ...(this.lastError === undefined ? {} : { error: this.lastError }),
      ...(this.buildStartedAtMs === undefined ? {} : { buildStartedAtMs: this.buildStartedAtMs }),
      ...(this.buildUpdatedAtMs === undefined ? {} : { buildUpdatedAtMs: this.buildUpdatedAtMs }),
    };
  }

  /** Kick a Tier-1 build; returns immediately. A running build is not doubled. */
  build(): { started: boolean } & ChildIndexStatus {
    if (this.building) {
      return { started: false, ...this.status() };
    }
    this.building = true;
    this.lastError = undefined;
    this.buildStartedAtMs = Date.now();
    this.buildUpdatedAtMs = this.buildStartedAtMs;
    this.buildPromise = this.runBuild()
      .catch((error: unknown) => {
        this.lastError = error instanceof Error ? error.message : String(error);
      })
      .finally(() => {
        this.building = false;
        this.buildPromise = null;
        this.buildUpdatedAtMs = Date.now();
      });
    return { started: true, ...this.status() };
  }

  /** Test/synchronization hook: resolves when the in-flight build settles. */
  async settled(): Promise<void> {
    await this.buildPromise;
  }

  private async scan(): Promise<CoreScanEntry[]> {
    try {
      return await this.core.scanWorkspace(this.root);
    } catch {
      // The wasm fallback may lack fs access for the walker; degrade to the
      // TS walk (root .gitignore only) rather than refuse to index at all.
      return fallbackScanWorkspace(this.root);
    }
  }

  private async runBuild(): Promise<void> {
    const entries = await this.scan();
    this.filesTotal = entries.length;
    this.filesIndexed = 0;

    // Every row we already hold, keyed by path. Two jobs: find the rows whose
    // file is gone (deleted below), and let the worker skip a file that has
    // not moved since it was indexed.
    const stored = new Map(
      (
        this.db.query("SELECT path, size, mtime_ms, language FROM files").all() as {
          path: string;
          size: number;
          mtime_ms: number;
          language: string | null;
        }[]
      ).map((row) => [row.path, row] as const),
    );
    const present = new Set(entries.map((e) => e.path));
    for (const path of stored.keys()) {
      if (!present.has(path)) this.deleteFileRows(path);
    }

    /**
     * Unchanged since its row was written? Then its symbols, imports, test
     * ranges and FTS content are all still correct and re-parsing is pure
     * waste — the cost the whole Tier-1 build used to pay on EVERY start,
     * even with an untouched tree.
     *
     * Three conditions, all cheap: size and mtime are the same freshness
     * anchor query-time verification already trusts (§7.3.3), and the
     * language must still resolve the same way — a lang pack registered
     * since the last build changes `languageForPath`, and those files must
     * be re-extracted rather than skipped.
     *
     * A build interrupted part-way therefore also resumes instead of
     * restarting: the rows it did write are reused.
     */
    const unchanged = (entry: CoreScanEntry): boolean => {
      const prior = stored.get(entry.path);
      return (
        prior !== undefined &&
        prior.size === entry.size &&
        Math.trunc(prior.mtime_ms) === Math.trunc(entry.mtimeMs) &&
        (prior.language ?? undefined) === languageForPath(entry.path)
      );
    };
    let reused = 0;

    // A bounded worker pool: extraction parallelism lives on the napi worker
    // threads; all SQLite writes stay here, on the one writer.
    let cursor = 0;
    let batch: FileWork[] = [];
    const flush = (): void => {
      if (batch.length === 0) return;
      const work = batch;
      batch = [];
      this.db.transaction(() => {
        for (const item of work) {
          this.writeFile(item);
        }
      })();
      this.filesIndexed += work.length;
      this.buildUpdatedAtMs = Date.now();
    };
    const worker = async (): Promise<void> => {
      while (cursor < entries.length) {
        const index = cursor;
        cursor += 1;
        const entry = entries[index];
        if (entry === undefined) break;
        if (unchanged(entry)) {
          reused += 1;
          this.filesIndexed += 1;
          this.buildUpdatedAtMs = Date.now();
          continue;
        }
        const work = await this.loadFile(entry);
        // Push only after the await: `batch.push(await …)` would capture the
        // pre-await array, and a concurrent worker's flush swaps `batch` out —
        // the resumed push would land on the flushed array and lose the file.
        batch.push(work);
        if (batch.length >= COMMIT_BATCH_SIZE) flush();
      }
    };
    await Promise.all(Array.from({ length: EXTRACT_CONCURRENCY }, () => worker()));
    flush();

    this.reusedLastBuild = reused;
    this.tier = 1;
    this.setMeta("tier", "1");
    this.bumpGeneration();
  }

  private async loadFile(entry: CoreScanEntry): Promise<FileWork> {
    const language = languageForPath(entry.path);
    if (entry.size > MAX_CONTENT_BYTES) {
      return { entry, language, content: null, extraction: null };
    }
    let bytes: Uint8Array;
    try {
      bytes = readFileSync(join(this.root, entry.path));
    } catch {
      return { entry, language, content: null, extraction: null };
    }
    if (looksBinary(bytes)) {
      return { entry, language, content: null, extraction: null };
    }
    const content = new TextDecoder().decode(bytes);
    let extraction: CoreExtraction | null = null;
    if (language !== undefined && this.core.languages().includes(language)) {
      try {
        extraction = await this.core.extract(language, content);
      } catch {
        // A file the extractor rejects still gets its files row and FTS.
        extraction = null;
      }
    }
    return { entry, language, content, extraction };
  }

  /** All row writes for one file. Caller wraps batches in a transaction. */
  private writeFile(work: FileWork): void {
    const { entry, language, content, extraction } = work;
    this.deleteFileRows(entry.path);
    this.db.run(
      "INSERT INTO files (path, language, size, mtime_ms, junk, content_indexed) VALUES (?, ?, ?, ?, ?, ?)",
      [
        entry.path,
        language ?? null,
        entry.size,
        entry.mtimeMs,
        entry.junk ? 1 : 0,
        content === null ? 0 : 1,
      ],
    );
    if (content !== null) {
      // The FTS row takes the files row id (specs/017): `path` is UNINDEXED
      // in FTS5, so the row id is the only O(log n) way back to this row.
      this.db.run(
        "INSERT INTO content_fts (rowid, path, content) " +
          "VALUES ((SELECT rowid FROM files WHERE path = ?1), ?1, ?2)",
        [entry.path, content],
      );
    }
    if (extraction !== null) {
      for (const symbol of extraction.symbols) {
        this.db.run(
          "INSERT INTO symbols (path, name, kind, start_line, start_col, end_line, detail, doc) " +
            "VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
          [
            entry.path,
            symbol.name,
            symbol.kind,
            symbol.startLine,
            symbol.startCol,
            symbol.endLine,
            symbol.detail ?? null,
            symbol.doc ?? null,
          ],
        );
      }
      for (const target of extraction.imports) {
        this.db.run("INSERT INTO imports (path, target) VALUES (?, ?)", [entry.path, target]);
      }
      for (const range of extraction.testRanges ?? []) {
        this.db.run("INSERT INTO test_ranges (path, start_line, end_line) VALUES (?, ?, ?)", [
          entry.path,
          range.startLine,
          range.endLine,
        ]);
      }
    }
  }

  /**
   * Every row of one path, each delete O(log n). A path without a files row
   * has no rows anywhere (every table is written under the files row in one
   * transaction), so a fresh build issues no delete at all — the by-path
   * FTS delete this replaced scanned the whole table per file and made the
   * build O(n²) (specs/017). The FTS row goes first: it is addressed by the
   * files row id, and a row id SQLite later reuses must never inherit stale
   * content.
   */
  private deleteFileRows(path: string): void {
    const row = this.db.query("SELECT rowid FROM files WHERE path = ?").get(path) as {
      rowid: number;
    } | null;
    if (row === null) return;
    this.db.run("DELETE FROM content_fts WHERE rowid = ?", [row.rowid]);
    this.db.run("DELETE FROM symbols WHERE path = ?", [path]);
    this.db.run("DELETE FROM test_ranges WHERE path = ?", [path]);
    this.db.run("DELETE FROM imports WHERE path = ?", [path]);
    this.db.run("DELETE FROM files WHERE path = ?", [path]);
  }

  querySymbols(name: string, kind?: string): SymbolRow[] {
    const filter = `%${name}%`;
    const rows = (
      kind === undefined
        ? this.db
            .query(
              "SELECT name, kind, path, start_line, start_col, end_line, detail, doc FROM symbols WHERE name LIKE ? COLLATE NOCASE LIMIT 200",
            )
            .all(filter)
        : this.db
            .query(
              "SELECT name, kind, path, start_line, start_col, end_line, detail, doc FROM symbols WHERE name LIKE ? COLLATE NOCASE AND kind = ? LIMIT 200",
            )
            .all(filter, kind)
    ) as SymbolRow[];
    return rows;
  }

  /**
   * The bounded verification loop of SDD §7.3.3, shared by every verified
   * query: each distinct answer path is checked against the filesystem
   * (mtime+size vs the files row). A mismatch triggers a synchronous
   * re-parse — bounded by VERIFY_REPARSE_BOUND — a vanished file has its rows
   * removed, and a file that could not be re-parsed within the bound is
   * reported dropped so the caller can exclude it and answer `cold`.
   */
  private async verifyAnswerPaths(
    paths: Iterable<string>,
    isCancelled?: CancelToken,
  ): Promise<{ dropped: Set<string>; changed: boolean; cold: boolean }> {
    const dropped = new Set<string>();
    let changed = false;
    let cold = false;
    let reparses = 0;
    for (const path of paths) {
      throwIfCancelled(isCancelled);
      const stored = this.db.query("SELECT size, mtime_ms FROM files WHERE path = ?").get(path) as {
        size: number;
        mtime_ms: number;
      } | null;
      let stats: ReturnType<typeof statSync>;
      try {
        stats = statSync(join(this.root, path));
      } catch {
        // Vanished: the rows describe a file that no longer exists.
        this.remove(path);
        changed = true;
        continue;
      }
      if (
        stored !== null &&
        stored.size === stats.size &&
        stored.mtime_ms === Math.trunc(stats.mtimeMs)
      ) {
        continue;
      }
      if (reparses < VERIFY_REPARSE_BOUND) {
        reparses += 1;
        try {
          await this.upsert(path);
          changed = true;
          continue;
        } catch {
          // Fall through: unreadable mid-verification counts as unverifiable.
        }
      }
      dropped.add(path);
      cold = true;
    }
    return { dropped, changed, cold };
  }

  private liveFreshness(): IndexFreshness {
    return this.tier === 0 ? "cold" : this.building ? "warming" : "warm";
  }

  /**
   * The freshness-verified symbol query of SDD §7.3.3. Because watchers lie,
   * the answer is verified (and re-read after any re-parse) before return.
   * No stale line number leaves here.
   */
  async querySymbolsVerified(
    name: string,
    kind?: string,
    isCancelled?: CancelToken,
  ): Promise<VerifiedSymbols> {
    let rows = this.querySymbols(name, kind);
    const { dropped, changed, cold } = await this.verifyAnswerPaths(
      new Set(rows.map((row) => row.path)),
      isCancelled,
    );
    if (changed || dropped.size > 0) {
      rows = this.querySymbols(name, kind).filter((row) => !dropped.has(row.path));
    }
    return { symbols: rows, freshness: cold ? "cold" : this.liveFreshness() };
  }

  private outlineRows(paths: readonly string[]): OutlineFileRows[] {
    const files: OutlineFileRows[] = [];
    const query = this.db.query(
      "SELECT name, kind, path, start_line, start_col, end_line, detail, doc FROM symbols " +
        "WHERE path = ? ORDER BY start_line, start_col",
    );
    for (const path of paths) {
      files.push({ path, symbols: query.all(path) as SymbolRow[] });
    }
    return files;
  }

  /**
   * Per-file symbol outlines for map's structural overview (SDD §9.2),
   * verified like every other symbol answer (§7.3.3). The path list is
   * capped at MAX_OUTLINE_PATHS; a file the index has no rows for answers
   * with an empty outline rather than an error.
   */
  async queryOutlineVerified(
    paths: readonly string[],
    isCancelled?: CancelToken,
  ): Promise<VerifiedOutline> {
    const bounded = [...new Set(paths)].slice(0, MAX_OUTLINE_PATHS);
    const { dropped, cold } = await this.verifyAnswerPaths(bounded, isCancelled);
    const files = this.outlineRows(bounded.filter((path) => !dropped.has(path)));
    return { files, freshness: cold ? "cold" : this.liveFreshness() };
  }

  /**
   * Test-only line ranges for the given files (specs/009 R4) — Rust
   * `#[cfg(test)]` modules. A file with none simply has no rows, so ranking
   * treats it exactly as it does today.
   */
  queryTestRanges(paths: readonly string[]): Record<string, TestRangeRow[]> {
    const out: Record<string, TestRangeRow[]> = {};
    const query = this.db.query(
      "SELECT path, start_line, end_line FROM test_ranges WHERE path = ? ORDER BY start_line",
    );
    for (const path of [...new Set(paths)].slice(0, MAX_OUTLINE_PATHS)) {
      const rows = query.all(path) as TestRangeRow[];
      if (rows.length > 0) out[path] = rows;
    }
    return out;
  }

  private topFanOut(limit: number): ImportGraphRow[] {
    return this.db
      .query(
        "SELECT path AS module, COUNT(*) AS count FROM imports " +
          "GROUP BY path ORDER BY count DESC, path LIMIT ?",
      )
      .all(limit) as ImportGraphRow[];
  }

  /**
   * The import-graph summary behind map's overview (SDD §9.2): top fan-out
   * (files with the most outgoing imports) and top fan-in (targets imported
   * most often). Extraction is name-resolution-free, so fan-in is keyed by
   * the raw specifier — never a claim that a specifier is a workspace file.
   * Fan-out modules ARE files, so they run the §7.3.3 verification loop like
   * every other answer whose rows can go stale behind the index's back.
   */
  async queryImportGraphVerified(
    limit: number,
    isCancelled?: CancelToken,
    crates?: readonly CrateScopeRow[],
  ): Promise<VerifiedImportGraph> {
    const bounded = Math.min(Math.max(Math.trunc(limit), 1), IMPORT_GRAPH_MAX_LIMIT);
    let fanOut = this.topFanOut(bounded);
    const { dropped, changed, cold } = await this.verifyAnswerPaths(
      new Set(fanOut.map((row) => row.module)),
      isCancelled,
    );
    if (changed || dropped.size > 0) {
      fanOut = this.topFanOut(bounded).filter((row) => !dropped.has(row.module));
    }
    const fanIn = this.db
      .query(
        "SELECT target AS module, COUNT(*) AS count FROM imports " +
          "GROUP BY target ORDER BY count DESC, target LIMIT ?",
      )
      .all(bounded) as ImportGraphRow[];
    return {
      fanIn,
      fanOut,
      ...(crates === undefined ? {} : { crates: this.crateEdges(crates) }),
      freshness: cold ? "cold" : this.liveFreshness(),
    };
  }

  /**
   * Per-crate import edges (specs/014 R6): for each scope, the import rows
   * whose target IS the crate's use-name or starts with `<name>::`, each
   * attributed to the LONGEST scope directory prefixing the importing path
   * ("." catches the rest), counted as distinct files per (from, to). GLOB,
   * not LIKE: crate names carry underscores, which LIKE would read as
   * wildcards.
   */
  private crateEdges(scopes: readonly CrateScopeRow[]): CrateEdgeRow[] {
    const bounded = scopes.slice(0, CRATE_SCOPES_MAX);
    const dirs = [...new Set(bounded.map((s) => s.dir).filter((d) => d !== "."))].sort(
      (a, b) => b.length - a.length,
    );
    const attribute = (path: string): string => dirs.find((d) => path.startsWith(`${d}/`)) ?? ".";
    const query = this.db.query(
      "SELECT DISTINCT path FROM imports WHERE target = ?1 OR target GLOB ?2",
    );
    const counts = new Map<string, number>();
    for (const scope of bounded) {
      if (!/^[A-Za-z0-9_]+$/.test(scope.name)) continue;
      const rows = query.all(scope.name, `${scope.name}::*`) as { path: string }[];
      for (const row of rows) {
        const key = `${attribute(row.path)}\u0000${scope.name}`;
        counts.set(key, (counts.get(key) ?? 0) + 1);
      }
    }
    return [...counts]
      .map(([key, files]) => {
        const [from, to] = key.split("\u0000") as [string, string];
        return { from, to, files };
      })
      .sort((a, b) => (a.from < b.from ? -1 : a.from > b.from ? 1 : a.to < b.to ? -1 : 1));
  }

  /**
   * On-disk content for a path the index knows. `path` is UNINDEXED in the
   * FTS5 table, so a per-path SELECT there is a full-table scan; the answer
   * paths are mtime+size-verified against disk anyway (§7.3.3), so disk is
   * both the fast and the honest source for the stragglers the MATCH pass
   * did not already return.
   */
  private contentFromDisk(path: string): string | null {
    try {
      return readFileSync(join(this.root, path), "utf8");
    } catch {
      return null;
    }
  }

  /**
   * References for `name` from Tier-1 data (SDD §7.3.1/§8.3): definitions
   * from the symbols table, occurrences from FTS5-candidate files scanned
   * with identifier boundaries. Name-resolution-free, so every edge carries
   * confidence provenance: `high`/tier1-symbols for definition rows,
   * `medium`/tier1-imports for occurrences in files import-linked to a
   * defining module, `low`/tier1-text for bare name matches (§14).
   */
  queryReferences(name: string, isCancelled?: CancelToken): ReferenceRow[] {
    const defs = this.db
      .query(
        "SELECT name, kind, path, start_line, start_col, end_line, detail, doc FROM symbols " +
          "WHERE name = ? COLLATE NOCASE LIMIT 200",
      )
      .all(name) as SymbolRow[];
    const defPaths = new Set(defs.map((d) => d.path));
    const defLines = new Map<string, Set<number>>();
    for (const d of defs) {
      const lines = defLines.get(d.path) ?? new Set<number>();
      lines.add(d.start_line);
      defLines.set(d.path, lines);
    }

    // Files import-linked to a defining module: their `imports.target` names
    // a defining file's stem as a path/module segment. Defining files link
    // to themselves.
    const linked = new Set<string>(defPaths);
    const stems = new Set<string>();
    for (const p of defPaths) {
      const base = p.split("/").pop() ?? p;
      const stem = base.replace(/\.[^.]+$/, "");
      if (stem !== "") stems.add(stem);
    }
    for (const stem of [...stems].slice(0, 8)) {
      const rows = this.db
        .query(
          "SELECT DISTINCT path FROM imports WHERE target = ?1 " +
            "OR target LIKE '%/' || ?1 OR target LIKE '%/' || ?1 || '.%' " +
            "OR target LIKE ?1 || '.%' OR target LIKE '%.' || ?1 " +
            "OR target LIKE ?1 || '::%' OR target LIKE '%::' || ?1 " +
            "OR target LIKE '%::' || ?1 || '::%'",
        )
        .all(stem) as { path: string }[];
      for (const row of rows) linked.add(row.path);
    }

    // Occurrence candidates from FTS5, content retrieved in the same MATCH
    // pass (a per-path SELECT on the UNINDEXED path column would be a full
    // scan per file). The quoted phrase tokenizes the same way the indexed
    // content did, so snake_case identifiers stay findable.
    const term = `content:"${name.replaceAll('"', '""')}"`;
    const contents = new Map<string, string | null>();
    const candidates: string[] = [];
    try {
      const rows = this.db
        .query("SELECT path, content FROM content_fts WHERE content_fts MATCH ? LIMIT ?")
        .all(term, MAX_REFERENCE_FILES) as { path: string; content: string }[];
      for (const row of rows) {
        candidates.push(row.path);
        contents.set(row.path, row.content);
      }
    } catch {
      // A name FTS5 cannot parse even quoted: fall back to defining files only.
    }
    const scanPaths: string[] = [...defPaths];
    for (const path of candidates) {
      if (scanPaths.length >= MAX_REFERENCE_FILES) break;
      if (!defPaths.has(path)) scanPaths.push(path);
    }

    const lookup = (path: string): string | null => {
      let content = contents.get(path);
      if (content === undefined) {
        content = this.contentFromDisk(path);
        contents.set(path, content);
      }
      return content;
    };
    const isIdentChar = (ch: string | undefined): boolean => ch !== undefined && /[\w$]/.test(ch);

    const edges: ReferenceRow[] = [];
    for (const d of defs) {
      const excerpt = lookup(d.path)?.split("\n")[d.start_line - 1]?.trimEnd() ?? "";
      edges.push({
        path: d.path,
        line: d.start_line,
        col: d.start_col,
        excerpt,
        ref_kind: "definition",
        confidence: "high",
        provenance: "tier1-symbols",
      });
    }
    let occurrences = 0;
    for (const path of scanPaths) {
      // §7.7: the per-file scan is the search iteration a cancel must stop.
      throwIfCancelled(isCancelled);
      if (occurrences >= MAX_REFERENCE_EDGES_TOTAL) break;
      const content = lookup(path);
      if (content === null) continue;
      const importLinked = linked.has(path);
      let kept = 0;
      const lines = content.split("\n");
      for (let i = 0; i < lines.length && kept < MAX_REFERENCE_EDGES_PER_FILE; i += 1) {
        if (defLines.get(path)?.has(i + 1) === true) continue; // already a definition edge
        const line = lines[i] as string;
        let from = 0;
        while (kept < MAX_REFERENCE_EDGES_PER_FILE) {
          const at = line.indexOf(name, from);
          if (at < 0) break;
          from = at + name.length;
          if (isIdentChar(line[at - 1]) || isIdentChar(line[at + name.length])) continue;
          edges.push({
            path,
            line: i + 1,
            col: at + 1,
            excerpt: line.trimEnd(),
            ref_kind: "use",
            confidence: importLinked ? "medium" : "low",
            provenance: importLinked ? "tier1-imports" : "tier1-text",
          });
          kept += 1;
          occurrences += 1;
          if (occurrences >= MAX_REFERENCE_EDGES_TOTAL) break;
        }
      }
    }
    edges.sort((a, b) => {
      if (a.ref_kind !== b.ref_kind) return a.ref_kind === "definition" ? -1 : 1;
      if (a.path !== b.path) return a.path < b.path ? -1 : 1;
      return a.line - b.line || a.col - b.col;
    });
    return edges;
  }

  /** The verified counterpart of {@link queryReferences} (SDD §7.3.3). */
  async queryReferencesVerified(
    name: string,
    isCancelled?: CancelToken,
  ): Promise<VerifiedReferences> {
    let edges = this.queryReferences(name, isCancelled);
    const { dropped, changed, cold } = await this.verifyAnswerPaths(
      new Set(edges.map((edge) => edge.path)),
      isCancelled,
    );
    if (changed || dropped.size > 0) {
      edges = this.queryReferences(name, isCancelled).filter((edge) => !dropped.has(edge.path));
    }
    return { edges, freshness: cold ? "cold" : this.liveFreshness() };
  }

  /** FTS5 hit count for a term — proof the content index is live. */
  contentMatches(term: string): number {
    const row = this.db
      .query("SELECT count(*) AS n FROM content_fts WHERE content_fts MATCH ?")
      .get(`content: ${JSON.stringify(term)}`) as { n: number } | null;
    return row?.n ?? 0;
  }

  /**
   * Re-index one file from its current on-disk content, or store the given
   * symbols when the caller already extracted them (the port's upsert seam).
   */
  async upsert(path: string, symbols?: readonly SymbolRow[]): Promise<void> {
    let stats: ReturnType<typeof statSync>;
    try {
      stats = statSync(join(this.root, path));
    } catch {
      throw new IndexMethodError("MALFORMED_FRAME", `index/upsert: no such file: ${path}`);
    }
    const entry: CoreScanEntry = {
      path,
      size: stats.size,
      mtimeMs: Math.trunc(stats.mtimeMs),
      junk: isJunkPath(path),
    };
    const work = await this.loadFile(entry);
    const applied: FileWork =
      symbols === undefined
        ? work
        : {
            ...work,
            extraction: {
              symbols: symbols.map((s) => ({
                name: s.name,
                kind: s.kind,
                startLine: s.start_line,
                startCol: s.start_col,
                endLine: s.end_line,
                detail: s.detail ?? null,
                doc: s.doc ?? null,
              })),
              imports: work.extraction?.imports ?? [],
            },
          };
    this.db.transaction(() => {
      this.writeFile(applied);
    })();
    this.bumpGeneration();
  }

  remove(path: string): void {
    this.db.transaction(() => {
      this.deleteFileRows(path);
    })();
    this.bumpGeneration();
  }

  close(): void {
    this.db.close();
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireString(params: Record<string, unknown>, field: string): string {
  const value = params[field];
  if (typeof value !== "string") {
    throw new IndexMethodError("MALFORMED_FRAME", `params.${field} must be a string`);
  }
  return value;
}

/** One indexer per workspace root, created on first `index/build`. */
export class IndexerHost {
  private readonly indexers = new Map<string, ChildIndexer>();

  constructor(private readonly core: CoreBinding) {}

  /** Test hook: the indexer for a root, if one was built. */
  get(root: string): ChildIndexer | undefined {
    return this.indexers.get(root);
  }

  private forRoot(root: string, journalMode?: "WAL" | "TRUNCATE"): ChildIndexer {
    let indexer = this.indexers.get(root);
    if (indexer === undefined) {
      indexer = new ChildIndexer(root, this.core, journalMode ?? "WAL");
      this.indexers.set(root, indexer);
    }
    return indexer;
  }

  private only(): ChildIndexer {
    const first = this.indexers.values().next();
    if (first.done === true) {
      throw new IndexMethodError(
        "INDEX_UNAVAILABLE",
        "no index exists yet: call index/build first",
      );
    }
    return first.value;
  }

  async handle(method: IndexMethod, params: unknown, isCancelled?: CancelToken): Promise<unknown> {
    const record = isRecord(params) ? params : {};
    switch (method) {
      case "index/build": {
        const root = requireString(record, "root");
        const journalMode = record["journalMode"] === "TRUNCATE" ? "TRUNCATE" : "WAL";
        return this.forRoot(root, journalMode).build();
      }
      case "index/status":
        return this.only().status();
      case "index/symbols": {
        const name = requireString(record, "name");
        const kind = typeof record["kind"] === "string" ? record["kind"] : undefined;
        return await this.only().querySymbolsVerified(name, kind, isCancelled);
      }
      case "index/testRanges": {
        const raw = record["paths"];
        if (!Array.isArray(raw) || raw.some((p) => typeof p !== "string")) {
          throw new IndexMethodError("MALFORMED_FRAME", "index/testRanges: paths must be strings");
        }
        return this.only().queryTestRanges(raw as string[]);
      }
      case "index/outline": {
        const raw = record["paths"];
        if (!Array.isArray(raw) || raw.some((p) => typeof p !== "string")) {
          throw new IndexMethodError("MALFORMED_FRAME", "params.paths must be a string array");
        }
        return await this.only().queryOutlineVerified(raw as string[], isCancelled);
      }
      case "index/references": {
        const name = requireString(record, "name");
        return await this.only().queryReferencesVerified(name, isCancelled);
      }
      case "index/importGraph": {
        const raw = record["limit"];
        const limit = typeof raw === "number" ? raw : IMPORT_GRAPH_DEFAULT_LIMIT;
        const rawCrates = record["crates"];
        const crates = Array.isArray(rawCrates)
          ? rawCrates.flatMap((c: unknown): CrateScopeRow[] => {
              if (typeof c !== "object" || c === null) return [];
              const scope = c as Record<string, unknown>;
              return typeof scope["name"] === "string" && typeof scope["dir"] === "string"
                ? [{ name: scope["name"], dir: scope["dir"] }]
                : [];
            })
          : undefined;
        return await this.only().queryImportGraphVerified(limit, isCancelled, crates);
      }
      case "index/upsert": {
        const path = requireString(record, "path");
        const symbols = Array.isArray(record["symbols"])
          ? (record["symbols"] as SymbolRow[])
          : undefined;
        await this.only().upsert(path, symbols);
        return { ok: true, generation: this.only().status().generation };
      }
      case "index/remove": {
        const path = requireString(record, "path");
        this.only().remove(path);
        return { ok: true, generation: this.only().status().generation };
      }
    }
  }

  close(): void {
    for (const indexer of this.indexers.values()) {
      indexer.close();
    }
    this.indexers.clear();
  }
}
