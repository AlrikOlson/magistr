/**
 * The `search` tool (SDD §9.1) — replaces grep, glob and find. Lexical,
 * structural, references and files modes are live; `auto` runs lexical
 * (Tier-0 capable, always answers); `semantic` is reserved and rejected with
 * a typed taxonomy error, never a schema failure (§1.1).
 *
 * mode:structural is ast-grep pattern matching at tree-sitter fidelity —
 * hits carry the matched node kind. mode:references answers from Tier-1
 * symbols and imports; the edges are name-resolution-free, so every hit
 * carries confidence provenance and the envelope says so rather than
 * overclaiming (§14). references requires a warm index and refuses with
 * `INDEX_UNAVAILABLE` (recovery retry, build progress in notes) until then.
 *
 * Results are always grouped by file and ranked — never a flat dump. This
 * layer builds one candidate group per file and hands every mode to the
 * six-stage ranking pipeline in ./ranking (SDD §7.4): kind classification,
 * file-class weighting, definition-first ordering, cross-file dedup,
 * score-ordered grouping, and the budget trim whose manifest is attached
 * here through `truncateResults`.
 *
 * This layer sees only ports and the envelope contract (import-graph rule).
 * The workspace walk is injected by the composing layer so Tier-0 answers
 * need no index and no adapter import.
 */

import {
  type Freshness,
  okEnvelope,
  type ResponseEnvelope,
  taxonomyError,
  truncateResults,
} from "../envelope";
import type {
  EngineMatch,
  EnginePort,
  IndexPort,
  IndexStatus,
  ReferenceEdge,
  ReferenceQueryResult,
  ResolvedEdge,
  ResolverPort,
  SymbolEntry,
  TestRange,
} from "../ports";
import {
  type CandidateGroup,
  groupTokens,
  type PluginFeatureLookup,
  rankGroups,
  type SearchGroup,
  type SearchHit,
  trimToBudget,
} from "./ranking";

export { MAX_HITS_PER_GROUP, type SearchGroup, type SearchHit } from "./ranking";

/** Modes this build answers; the schema's enum additionally carries the rest. */
export const LIVE_SEARCH_MODES = ["auto", "lexical", "structural", "references", "files"] as const;

export interface WorkspaceFile {
  /** Workspace-relative posix path. */
  readonly path: string;
  /** Vendored/generated/lockfile content — demoted by ranking, never hidden. */
  readonly junk: boolean;
}

/** The walk and read capability the composing layer injects (Tier-0 capable). */
export interface SearchWorkspaceAccess {
  list(): Promise<readonly WorkspaceFile[]>;
  read(path: string): Promise<string>;
}

/**
 * The ranking-signal seam (chunk:m5-ranking-shadow): a synchronous cache
 * lookup for promoted signals and a fire-and-forget post-ranking hook for
 * shadow scoring. NOTHING here may await a sandbox — the search response
 * path stays sandbox-free by construction (SC-005, Constitution I).
 */
export interface RankingSignalsPort {
  /** Warm promoted-signal features, or undefined ⇒ exact baseline path. */
  promotedFeatures(): PluginFeatureLookup | undefined;
  /** Schedules shadow scoring + logging; returns immediately, never throws. */
  afterSearch(
    query: string,
    candidates: readonly CandidateGroup[],
    servedOrder: readonly string[],
  ): void;
  /** Per-signal state lines for workbench_status notes. */
  statusNotes(): string[];
}

export interface SearchDeps {
  readonly engine: EnginePort;
  readonly index: IndexPort;
  readonly workspace: SearchWorkspaceAccess;
  /**
   * Optional Tier-2 upgrade lane (chunk:m5-lsp-resolver-plugin): a
   * ResolverPort whose `resolver-lsp` edges upgrade matching index edges'
   * provenance. Absent or failing, references stay at tree-sitter fidelity —
   * the resolver is never a requirement (SDD §1.1).
   */
  readonly resolver?: ResolverPort;
  /**
   * Optional ranking-signal lane (chunk:m5-ranking-shadow). Absent, booting,
   * cold, or unhealthy all mean the same thing: first-party baseline.
   */
  readonly rankingSignals?: RankingSignalsPort;
}

/** Wire shape of one tool call, mirroring schemas/tools/search.input.schema.json. */
export interface SearchInput {
  readonly query: string;
  readonly mode?: string;
  readonly kind?: readonly string[];
  readonly path?: string;
  readonly include?: { readonly tests?: boolean; readonly vendor?: boolean };
  readonly context?: number;
  readonly budget?: number;
}

const GLOB_CHARS = /[*?[\]{}]/;

function matchesGlob(pattern: string, path: string): boolean {
  return new Bun.Glob(pattern).match(path);
}

// `benches`/`examples` are Rust's non-shipping directories (specs/009 R5);
// the rest are the JS/Python/Go conventions this already covered.
const TEST_SEGMENTS = new Set([
  "test",
  "tests",
  "__tests__",
  "spec",
  "specs",
  "benches",
  "examples",
]);
const TEST_FILE = /\.(test|spec)\.[^.]+$|_test\.[^.]+$/;

function isTestPath(path: string): boolean {
  const segments = path.split("/");
  const file = segments[segments.length - 1] ?? path;
  return segments.slice(0, -1).some((s) => TEST_SEGMENTS.has(s)) || TEST_FILE.test(file);
}

interface ScopedFile extends WorkspaceFile {
  readonly test: boolean;
}

/** Apply the path glob and include{tests,vendor} excludes (absent = demote). */
function scopeFiles(files: readonly WorkspaceFile[], input: SearchInput): readonly ScopedFile[] {
  const scoped: ScopedFile[] = [];
  for (const file of files) {
    if (input.path !== undefined && !matchesGlob(input.path, file.path)) continue;
    const test = isTestPath(file.path);
    if (input.include?.tests === false && test) continue;
    if (input.include?.vendor === false && file.junk) continue;
    scoped.push({ ...file, test });
  }
  return scoped;
}

/** Identifier-ish queries are worth a symbol-index lookup; regexes are not. */
function isSymbolishQuery(query: string): boolean {
  return /^[\w$.:-]{1,128}$/.test(query);
}

function engineFailure(v: number, error: unknown): ResponseEnvelope<SearchGroup> {
  const message = error instanceof Error ? error.message : String(error);
  const code = (error as { code?: unknown }).code;
  if (code === "INDEX_UNAVAILABLE") {
    return taxonomyError({ v, code: "INDEX_UNAVAILABLE", message });
  }
  if (code === "INPUT_QUARANTINED") {
    return taxonomyError({ v, code: "INPUT_QUARANTINED", message });
  }
  if (code === "ENGINE_BUSY") {
    const backoffMs = (error as { backoffMs?: unknown }).backoffMs;
    return taxonomyError({
      v,
      code: "ENGINE_BUSY",
      message,
      backoffMs: typeof backoffMs === "number" ? backoffMs : 250,
    });
  }
  if (/invalid search pattern|invalid structural pattern|regex parse error/i.test(message)) {
    return taxonomyError({ v, code: "PATTERN_INVALID", message });
  }
  return taxonomyError({ v, code: "ENGINE_UNAVAILABLE", message });
}

/** The typed rejection for modes this build does not answer (SDD §1.1). */
function modeRejection(v: number, mode: string): ResponseEnvelope<SearchGroup> {
  const message =
    mode === "semantic"
      ? 'search.mode "semantic" is reserved and rejected in v1 (SDD §1.1); ' +
        "use lexical, files, structural or references"
      : `search.mode ${JSON.stringify(mode)} is not a mode this server version knows; ` +
        "supported: auto, lexical, structural, references, files (semantic is reserved)";
  return taxonomyError({ v, code: "UNSUPPORTED_VERSION", message });
}

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

/** Symbols in `file` whose declaration starts at `line`, if the index has any. */
async function symbolTags(
  index: IndexPort,
  input: SearchInput,
): Promise<ReadonlyMap<string, readonly SymbolEntry[]>> {
  if (!isSymbolishQuery(input.query)) return new Map();
  let symbols: readonly SymbolEntry[];
  try {
    ({ symbols } = await index.querySymbols({ name: input.query }));
  } catch {
    // Index unavailable — lexical answers stand on their own (Constitution I).
    return new Map();
  }
  const wanted = input.kind === undefined ? null : new Set(input.kind);
  const byFile = new Map<string, SymbolEntry[]>();
  for (const symbol of symbols) {
    if (wanted !== null && !wanted.has(symbol.kind)) continue;
    const list = byFile.get(symbol.location.path) ?? [];
    list.push(symbol);
    byFile.set(symbol.location.path, list);
  }
  return byFile;
}

function engineCandidates(
  matches: readonly EngineMatch[],
  files: readonly ScopedFile[],
  tags: ReadonlyMap<string, readonly SymbolEntry[]>,
  structural: boolean,
  wantedKinds: readonly string[] | undefined,
): CandidateGroup[] {
  const meta = new Map(files.map((f) => [f.path, f]));
  const byFile = new Map<string, EngineMatch[]>();
  for (const match of matches) {
    // Structural kind filter: exact node kind, or a schema-style shorthand
    // ("call" matches "call_expression") — substring, never a guess upward.
    if (
      structural &&
      wantedKinds !== undefined &&
      !wantedKinds.some((k) => match.kind === k || match.kind?.includes(k) === true)
    ) {
      continue;
    }
    const list = byFile.get(match.location.path) ?? [];
    list.push(match);
    byFile.set(match.location.path, list);
  }
  const candidates: CandidateGroup[] = [];
  for (const [file, fileMatches] of byFile) {
    const fileTags = tags.get(file) ?? [];
    const hits = fileMatches.map((match): SearchHit => {
      const tag = fileTags.find((s) => s.location.line === match.location.line);
      const kind = match.kind ?? tag?.kind;
      return {
        line: match.location.line,
        column: match.location.column ?? 1,
        excerpt: match.excerpt,
        ...(kind === undefined ? {} : { kind }),
      };
    });
    const info = meta.get(file);
    candidates.push({
      file,
      junk: info?.junk ?? false,
      test: info?.test ?? isTestPath(file),
      hits,
    });
  }
  return candidates;
}

/**
 * Fold resolver-verified edges into the index's symbolic answer: an index
 * edge the resolver confirms (same file + line) is upgraded to
 * `resolver-lsp`/`high`; a resolver-only edge is appended with an empty
 * excerpt and `refKind: "use"`. Per-edge, never per-response (spec US3) —
 * unconfirmed symbolic edges keep their honest `tier1-*` labels.
 */
function mergeResolverEdges(
  indexEdges: readonly ReferenceEdge[],
  resolved: readonly ResolvedEdge[],
): readonly ReferenceEdge[] {
  const keyOf = (path: string, line: number): string => `${path} ${line}`;
  const verified = new Set(resolved.map((e) => keyOf(e.location.path, e.location.line)));
  const merged: ReferenceEdge[] = indexEdges.map((edge) =>
    verified.has(keyOf(edge.location.path, edge.location.line))
      ? { ...edge, provenance: "resolver-lsp", confidence: "high" }
      : edge,
  );
  const known = new Set(indexEdges.map((e) => keyOf(e.location.path, e.location.line)));
  for (const edge of resolved) {
    if (known.has(keyOf(edge.location.path, edge.location.line))) continue;
    merged.push({
      location: edge.location,
      excerpt: "",
      refKind: "use",
      confidence: edge.confidence,
      provenance: edge.provenance,
    });
  }
  return merged;
}

function referenceCandidates(
  edges: readonly ReferenceEdge[],
  files: readonly ScopedFile[],
): CandidateGroup[] {
  const meta = new Map(files.map((f) => [f.path, f]));
  const byFile = new Map<string, ReferenceEdge[]>();
  for (const edge of edges) {
    // Scope discipline: an edge outside the path glob / include excludes is
    // not part of this answer.
    if (!meta.has(edge.location.path)) continue;
    const list = byFile.get(edge.location.path) ?? [];
    list.push(edge);
    byFile.set(edge.location.path, list);
  }
  const candidates: CandidateGroup[] = [];
  for (const [file, fileEdges] of byFile) {
    const info = meta.get(file);
    candidates.push({
      file,
      junk: info?.junk ?? false,
      test: info?.test ?? isTestPath(file),
      hits: fileEdges.map(
        (edge): SearchHit => ({
          line: edge.location.line,
          column: edge.location.column ?? 1,
          excerpt: edge.excerpt,
          kind: edge.refKind,
          confidence: edge.confidence,
          provenance: edge.provenance,
        }),
      ),
    });
  }
  return candidates;
}

/**
 * Attach each candidate file's test-only line ranges, when the index can
 * answer them. Best-effort by design: a failure here must never fail a
 * search, it only costs the hit-level demotion (Constitution I).
 */
async function withTestRanges(
  deps: SearchDeps,
  candidates: readonly CandidateGroup[],
): Promise<CandidateGroup[]> {
  const query = deps.index.queryTestRanges?.bind(deps.index);
  if (query === undefined || candidates.length === 0) return [...candidates];
  let ranges: Readonly<Record<string, readonly TestRange[]>>;
  try {
    ranges = await query(candidates.map((c) => c.file));
  } catch {
    return [...candidates];
  }
  return candidates.map((candidate) => {
    const found = ranges[candidate.file];
    return found === undefined || found.length === 0
      ? candidate
      : { ...candidate, testRanges: found };
  });
}

function fileCandidates(files: readonly ScopedFile[], query: string): CandidateGroup[] {
  const glob = GLOB_CHARS.test(query);
  const needle = query.toLowerCase();
  const candidates: CandidateGroup[] = [];
  for (const file of files) {
    const matched = glob ? matchesGlob(query, file.path) : file.path.toLowerCase().includes(needle);
    if (!matched) continue;
    candidates.push({ file: file.path, junk: file.junk, test: file.test, hits: [] });
  }
  return candidates;
}

/** Attach `context` lines around each kept hit, reading each file once. */
async function attachContext(
  deps: SearchDeps,
  groups: readonly SearchGroup[],
  context: number,
): Promise<readonly SearchGroup[]> {
  if (context === 0) return groups;
  const out: SearchGroup[] = [];
  for (const group of groups) {
    if (group.hits.length === 0) {
      out.push(group);
      continue;
    }
    let lines: string[];
    try {
      lines = (await deps.workspace.read(group.file)).split("\n");
    } catch {
      out.push(group);
      continue;
    }
    out.push({
      ...group,
      hits: group.hits.map((hit) => ({
        ...hit,
        contextBefore: lines.slice(Math.max(0, hit.line - 1 - context), hit.line - 1),
        contextAfter: lines.slice(hit.line, hit.line + context),
      })),
    });
  }
  return out;
}

/** §9.1: references strictly requires a warm index — refuse with progress. */
function indexUnavailable(v: number, status: IndexStatus | null): ResponseEnvelope<SearchGroup> {
  const progress =
    status === null
      ? "no index exists yet for this workspace"
      : status.building === true
        ? `index build in progress: ${status.filesIndexed ?? 0}/${status.filesTotal ?? 0} files`
        : "the Tier-1 index build has not completed";
  return taxonomyError({
    v,
    code: "INDEX_UNAVAILABLE",
    message:
      'search.mode "references" requires a warm index and it is not warm yet ' +
      "(SDD §9.1, Constitution I); retry once the Tier-1 build completes",
    notes: [progress],
  });
}

/** §14: the honest fidelity note every references answer carries. */
const REFERENCES_NOTE =
  "references are tree-sitter fidelity (name-resolution-free): high confidence is a " +
  "Tier-1 definition row, medium an occurrence in an import-linked file, low a bare " +
  "name match that may be a false positive";

/**
 * Answer one search call with a ResponseEnvelope. The caller negotiates the
 * envelope version and seals (redacts) the result before it reaches a wire.
 */
export async function runSearch(
  deps: SearchDeps,
  input: SearchInput,
  v: number,
  opts: { readonly signal?: AbortSignal } = {},
): Promise<ResponseEnvelope<SearchGroup>> {
  const mode = input.mode ?? "auto";
  if (!(LIVE_SEARCH_MODES as readonly string[]).includes(mode)) {
    return modeRejection(v, mode);
  }

  const budget = input.budget ?? 4000;
  const context = input.context ?? 2;
  const status = await indexStatusOrNull(deps.index);
  let freshness = freshnessOf(status);
  const scoped = scopeFiles(await deps.workspace.list(), input);
  const notes: string[] = [];
  if (input.path !== undefined && scoped.length === 0) {
    notes.push(`path scope ${JSON.stringify(input.path)} matched no files; nothing was searched`);
  }

  let candidates: CandidateGroup[];
  if (mode === "files") {
    candidates = fileCandidates(scoped, input.query);
  } else if (mode === "references") {
    if (status === null || status.tier === 0) {
      return indexUnavailable(v, status);
    }
    let result: ReferenceQueryResult;
    try {
      result = await deps.index.queryReferences({ name: input.query });
    } catch (error) {
      return engineFailure(v, error);
    }
    // The reference answer was itself verified (§7.3.3); its freshness is the
    // envelope's.
    freshness = result.freshness;
    let edges: readonly ReferenceEdge[] = result.edges;
    if (deps.resolver !== undefined) {
      try {
        const upgraded = await deps.resolver.resolve({
          kind: "references",
          symbol: input.query,
        });
        if (upgraded.length > 0) {
          edges = mergeResolverEdges(result.edges, upgraded);
          notes.push(
            `${upgraded.length} edge(s) verified by the resolver plugin ` +
              "(Tier-2, provenance resolver-lsp)",
          );
        }
      } catch {
        // The resolver is an upgrade, never a requirement (SDD §1.1): any
        // failure leaves the symbolic answer untouched.
      }
    }
    candidates = referenceCandidates(edges, scoped);
    notes.push(REFERENCES_NOTE);
  } else {
    const structural = mode === "structural";
    // An empty scope never reaches the engine: EngineSearchRequest reads an
    // empty `paths` as "the whole workspace" (ports/engine.ts), which would
    // flip a scope that matched nothing into an unscoped walk — and let a
    // quarantined file OUTSIDE the requested scope refuse the whole search
    // (chunk:backlog-quarantine-scope-leak). INPUT_QUARANTINED may only
    // answer a search that would actually touch the quarantined file.
    let matches: readonly EngineMatch[] = [];
    if (scoped.length > 0) {
      try {
        matches = await deps.engine.search({
          mode: structural ? "structural" : "lexical",
          pattern: input.query,
          paths: scoped.map((f) => f.path),
          ...(opts.signal === undefined ? {} : { signal: opts.signal }),
        });
      } catch (error) {
        return engineFailure(v, error);
      }
    }
    const tags = structural ? new Map() : await symbolTags(deps.index, input);
    candidates = engineCandidates(matches, scoped, tags, structural, input.kind);
    if (mode === "auto") {
      notes.push(
        "mode:auto ran lexical; use mode:structural for AST patterns or mode:references " +
          "for symbol edges",
      );
    }
  }

  // Rust puts unit tests INSIDE the file under test, so hits in a
  // `#[cfg(test)]` module must rank as test code even though the file is
  // source (specs/009 R4). Ranges come from the index; an adapter that cannot
  // answer simply leaves every candidate as it was.
  candidates = await withTestRanges(deps, candidates);

  // Stages 1–5 (SDD §7.4): classify, weight, order, dedup, group and score.
  // Promoted plugin features enter as a synchronous warm-cache lookup only —
  // a cold cache is the baseline, and no sandbox runs on this path (SC-005).
  const groups = rankGroups(candidates, input.query, deps.rankingSignals?.promotedFeatures());
  // Shadow lane (chunk:m5-ranking-shadow): fire-and-forget scoring + logging
  // for installed signals; the response never waits on it.
  deps.rankingSignals?.afterSearch(
    input.query,
    candidates,
    groups.map((g) => g.file),
  );

  // Stage 6: keep whole groups until the budget estimate is spent.
  const { keep, used } = trimToBudget(groups, budget);
  if (groups.length > 0 && keep === 0) {
    return taxonomyError({
      v,
      code: "BUDGET_TOO_SMALL",
      message:
        `budget ${budget} cannot fit even one result group ` +
        `(first group ≈${groupTokens(groups[0] as SearchGroup)} tokens); ` +
        "raise the budget or narrow the query",
    });
  }
  const cursorGroup = groups[keep];
  const cut = truncateResults(groups, keep, {
    reason: "budget",
    narrow: ["scope with path (e.g. src/**)", "raise budget", "make the query more specific"],
    cursor: cursorGroup === undefined ? "-" : `file:${cursorGroup.file}`,
  });

  const kept = await attachContext(deps, cut.kept, mode === "files" ? 0 : context);

  return okEnvelope<SearchGroup>({
    v,
    freshness,
    budget: { requested: budget, used },
    results: cut.truncation === undefined ? kept : { kept, truncation: cut.truncation },
    ...(notes.length === 0 ? {} : { notes }),
  });
}
