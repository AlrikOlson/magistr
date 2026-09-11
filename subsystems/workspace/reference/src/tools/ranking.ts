/**
 * The six-stage ranking pipeline (SDD §7.4, law #2) — the product. Applied to
 * all search results before enveloping, in order: (1) kind classification,
 * (2) file-class weighting, (3) definition-first ordering within groups,
 * (4) dedup of identical lines across files into one entry + count,
 * (5) grouping by file ordered by best-hit score + path relevance, (6) trim
 * to budget (the truncation manifest itself is attached by the caller through
 * `truncateResults`, so it exists by construction).
 *
 * The combining model is first-party, always. Every tunable number lives in
 * {@link RANKING_WEIGHTS} — one table — and no weight change merges without a
 * before/after MRR number from the eval harness (scripts/eval-ranking.ts
 * enforces this by hashing the table into the recorded baseline).
 *
 * Pure module: no I/O, no ports. Stages are exported individually so the
 * golden-file tests cover each one in isolation.
 */

import type { TestRange } from "../ports";

export interface SearchHit {
  readonly line: number;
  readonly column: number;
  readonly excerpt: string;
  /**
   * Kind tag: the symbol kind when the warm index tagged this line (lexical),
   * the matched tree-sitter node kind (structural), or `definition`/`use`
   * (references).
   */
  readonly kind?: string;
  /** References mode: how much to trust this edge (SDD §14). */
  readonly confidence?: "high" | "medium" | "low";
  /** References mode: how the edge was derived (tier1-symbols/-imports/-text). */
  readonly provenance?: string;
  /** Stage 4: this line also matched in `duplicates` other files, elided there. */
  readonly duplicates?: number;
  readonly contextBefore?: readonly string[];
  readonly contextAfter?: readonly string[];
}

/** One result: a file group. Files mode emits groups with no hits. */
export interface SearchGroup {
  readonly file: string;
  readonly junk: boolean;
  /** Total matches in this file; > hits.length when the group elides. */
  readonly totalHits: number;
  readonly hits: readonly SearchHit[];
}

/** What the pipeline ranks: one candidate group per file, hits uncapped. */
export interface CandidateGroup {
  readonly file: string;
  readonly junk: boolean;
  readonly test: boolean;
  readonly hits: readonly SearchHit[];
  /**
   * Test-only line ranges INSIDE this file — Rust `#[cfg(test)]` modules
   * (specs/009 R4). Rust puts unit tests in the file under test, so no
   * per-file rule can see them; a hit inside one is test code even though the
   * file is source. Absent (the common case) leaves classification exactly as
   * it was.
   */
  readonly testRanges?: readonly TestRange[];
}

/** Stage-1 classes (SDD §7.4): declaration/call/import/comment/string. */
export type KindClass = "declaration" | "call" | "import" | "comment" | "string" | "other";

/** Stage-2 classes: source > tests > vendor/generated, strictly tiered. */
export type FileClass = "source" | "test" | "vendor";

/**
 * The ONE tunable table (SDD §7.4). Changing any number here changes the
 * weights hash the eval baseline records; CI refuses the change until
 * `bun run eval:rank --record` lands the before/after MRR alongside it.
 */
export const RANKING_WEIGHTS = {
  /** Stage 1/3: how much each kind class contributes to a hit's score. */
  kind: { declaration: 4.0, call: 2.0, import: 1.5, other: 1.0, string: 0.75, comment: 0.5 },
  /** Stage 2: strict tiers — a higher tier always outranks a lower one. */
  fileClass: { source: 2, test: 1, vendor: 0 },
  /** References mode: multiplier on the hit score per confidence level. */
  confidence: { high: 1.0, medium: 0.6, low: 0.3 },
  /** Stage 5 path relevance: query-token-in-path bonus, path-depth penalty. */
  path: { segmentMatch: 1.0, depthPenalty: 0.1 },
  /**
   * Stage 5 plugin features (chunk:m5-ranking-shadow): weight per namespaced
   * feature name (`<plugin>/<feature>`). The ONLY path by which a ranking
   * plugin's value reaches a score — a name not listed here has weight zero
   * even when the signal is promoted and its cache is warm. Being part of
   * this table puts every entry under the eval weights-hash gate.
   */
  plugin: { "ranking-visibility/exported": 0.5 } as Record<string, number>,
  /** Stage 4: collapse a line seen in at least `minFiles` files. */
  dedup: { minFiles: 3, minExcerptLength: 4 },
  /** Stage 5: hits kept per group; the rest elide via totalHits. */
  maxHitsPerGroup: 20,
  /** Stage 6: chars-per-token estimate for budget accounting. */
  charsPerToken: 4,
} as const;

/** Hits kept per file group before the group reports elision via totalHits. */
export const MAX_HITS_PER_GROUP: number = RANKING_WEIGHTS.maxHitsPerGroup;

/** Warm tags that mean "this line declares the symbol" (symbol-index kinds). */
const SYMBOL_KINDS = new Set([
  "function",
  "method",
  "class",
  "struct",
  "enum",
  "interface",
  "trait",
  "type",
  "type_alias",
  "module",
  "constant",
  "variable",
  "field",
  "property",
  "macro",
]);

const COMMENT_LINE = /^\s*(?:\/\/|\/\*|\*|#(?!\[|!)|--|;;)/;
const IMPORT_LINE =
  /^\s*(?:import\b|from\s+\S+\s+import\b|export\s+.*\bfrom\b|use\s+[\w:{]|require\s*\(|#include\b|using\s+\w)/;
const DECLARATION_KEYWORD =
  "function|def|fn|class|struct|enum|trait|interface|impl|type|mod|module";
const ASSIGNMENT_KEYWORD = "const|let|var|val|static|pub";
const DECLARATION_LINE = new RegExp(`\\b(?:${DECLARATION_KEYWORD})\\s+[\\w$]`);
const ASSIGNMENT_DECLARATION = new RegExp(`\\b(?:${ASSIGNMENT_KEYWORD})\\s+[\\w$]+\\s*[=:(]`);

/**
 * Does this line declare *the queried name*? For a symbolish query the
 * declared identifier must be the query itself — `const s = needle(x)` is a
 * call site, not a declaration, even though the line starts with `const`.
 */
function declaresQuery(excerpt: string, query: string): boolean {
  if (!/^[\w$]+$/.test(query)) {
    return DECLARATION_LINE.test(excerpt) || ASSIGNMENT_DECLARATION.test(excerpt);
  }
  const name = query.replace(/\$/g, "\\$&");
  return (
    new RegExp(`\\b(?:${DECLARATION_KEYWORD})\\s+${name}\\b`).test(excerpt) ||
    new RegExp(`\\b(?:${ASSIGNMENT_KEYWORD})\\s+${name}\\s*[=:(]`).test(excerpt)
  );
}

/** Is `column` (1-based) inside a quoted span on this line? Heuristic. */
function insideString(excerpt: string, column: number): boolean {
  let quote: string | null = null;
  for (let i = 0; i < Math.min(column - 1, excerpt.length); i += 1) {
    const ch = excerpt[i];
    if (quote === null) {
      if (ch === '"' || ch === "'" || ch === "`") quote = ch;
    } else if (ch === quote && excerpt[i - 1] !== "\\") {
      quote = null;
    }
  }
  return quote !== null;
}

/** Cold-path classification from the excerpt alone (tree-sitter not warm). */
function classifyExcerpt(hit: SearchHit, query: string): KindClass {
  const { excerpt, column } = hit;
  if (COMMENT_LINE.test(excerpt)) return "comment";
  if (IMPORT_LINE.test(excerpt)) return "import";
  if (declaresQuery(excerpt, query)) return "declaration";
  if (insideString(excerpt, column)) return "string";
  if (/^[\w$]+$/.test(query) && new RegExp(`\\b${query}\\s*\\(`).test(excerpt)) return "call";
  return "other";
}

/**
 * Stage 1 — kind classification. A warm tag (symbol kind, tree-sitter node
 * kind, or references refKind) decides; the cold path falls back to excerpt
 * heuristics. The wire `kind` field is untouched — the class is a ranking
 * input, not an output claim.
 */
export function classifyHit(hit: SearchHit, query: string): KindClass {
  const kind = hit.kind;
  if (kind !== undefined && kind !== "use") {
    if (/comment/.test(kind)) return "comment";
    if (/string|str_lit|template/.test(kind)) return "string";
    if (/import|include|require/.test(kind)) return "import";
    if (/call|invocation|new_expression/.test(kind)) return "call";
    if (/defin|declar/.test(kind)) return "declaration";
    if (SYMBOL_KINDS.has(kind)) return "declaration";
  }
  return classifyExcerpt(hit, query);
}

/** Is this 1-based line inside any of the file's test-only ranges? */
export function inTestRange(line: number, ranges: readonly TestRange[] | undefined): boolean {
  return ranges !== undefined && ranges.some((r) => line >= r.startLine && line <= r.endLine);
}

/**
 * Stage 2, hit-aware (specs/009 R4). A file whose every hit falls inside a
 * test-only region IS test code for ranking, even though the file itself is
 * source; a file with any production hit keeps its own class. Falls back to
 * [`fileClassOf`] whenever no ranges are known, so nothing outside Rust — and
 * no caller that cannot supply ranges — changes behavior.
 */
export function fileClassOfGroup(group: CandidateGroup): FileClass {
  const base = fileClassOf(group);
  if (base !== "source" || group.testRanges === undefined || group.hits.length === 0) return base;
  return group.hits.every((hit) => inTestRange(hit.line, group.testRanges)) ? "test" : base;
}

/** Stage 2 — file-class weighting: source > tests > vendor/generated. */
export function fileClassOf(group: { junk: boolean; test: boolean }): FileClass {
  if (group.junk) return "vendor";
  if (group.test) return "test";
  return "source";
}

/** A hit with its stage-1 class attached — the pipeline's working shape. */
export interface ClassifiedHit extends SearchHit {
  readonly kindClass: KindClass;
  /** The hit sits inside a test-only region of an otherwise-source file. */
  readonly inTestRange?: boolean;
}

/** One namespaced plugin feature value (chunk:m5-ranking-shadow). */
export interface PluginFeature {
  /** `<plugin>/<feature>`, namespaced by the host before it gets here. */
  readonly name: string;
  readonly value: number;
}

/**
 * Promoted plugin features by hit identity, resolved by the CALLER from the
 * feature cache before ranking — this module stays pure (no I/O, no ports).
 * Key: {@link pluginFeatureKey}. Absent map or absent key ⇒ zero contribution
 * (Constitution I: a cold cache degrades to the first-party baseline).
 */
export type PluginFeatureLookup = ReadonlyMap<string, readonly PluginFeature[]>;

/** Cache/lookup key for one hit's plugin features. */
export function pluginFeatureKey(
  file: string,
  line: number,
  column: number,
  excerpt: string,
): string {
  return `${file}\u0000${line}\u0000${column}\u0000${excerpt}`;
}

/** Combiner-weighted plugin boost for one hit: Σ weight(name)·value. */
function pluginBoost(features: readonly PluginFeature[] | undefined): number {
  if (features === undefined) return 0;
  let sum = 0;
  for (const feature of features) {
    if (!Number.isFinite(feature.value)) continue;
    sum += (RANKING_WEIGHTS.plugin[feature.name] ?? 0) * feature.value;
  }
  return sum;
}

function hitScore(hit: ClassifiedHit): number {
  const kind = RANKING_WEIGHTS.kind[hit.kindClass];
  const confidence = hit.confidence === undefined ? 1 : RANKING_WEIGHTS.confidence[hit.confidence];
  return kind * confidence;
}

/**
 * Stage 3 — definition-first ordering within a group: declarations first,
 * then higher confidence, then higher kind weight, then source order.
 */
export function orderHits(hits: readonly ClassifiedHit[]): ClassifiedHit[] {
  const confidenceOf = (h: ClassifiedHit): number =>
    h.confidence === undefined ? 1 : RANKING_WEIGHTS.confidence[h.confidence];
  return [...hits].sort((a, b) => {
    const aDecl = a.kindClass === "declaration";
    const bDecl = b.kindClass === "declaration";
    if (aDecl !== bDecl) return aDecl ? -1 : 1;
    const confidence = confidenceOf(b) - confidenceOf(a);
    if (confidence !== 0) return confidence;
    // Test-only regions are demoted before kind is even considered — the
    // same strict tiering file classes get (specs/009 R4).
    const region = Number(a.inTestRange ?? false) - Number(b.inTestRange ?? false);
    if (region !== 0) return region;
    const kind = RANKING_WEIGHTS.kind[b.kindClass] - RANKING_WEIGHTS.kind[a.kindClass];
    if (kind !== 0) return kind;
    return a.line - b.line;
  });
}

/** A group with its stage-2 class attached — stages 3–5 operate on this. */
export interface ClassifiedGroup {
  readonly file: string;
  readonly junk: boolean;
  readonly test: boolean;
  readonly fileClass: FileClass;
  readonly hits: readonly ClassifiedHit[];
  readonly totalHits: number;
}

/**
 * Stage 4 — dedup: an identical (trimmed) matching line present in at least
 * `dedup.minFiles` files collapses to one entry in the best file — highest
 * file-class tier, then lexicographic path — carrying `duplicates`, the count
 * of files it was elided from. `totalHits` stays honest per file: the match
 * exists there; only its display collapses.
 */
export function dedupAcrossFiles(groups: readonly ClassifiedGroup[]): ClassifiedGroup[] {
  const byLine = new Map<string, { files: number; canonical: string }>();
  for (const group of groups) {
    const seen = new Set<string>();
    for (const hit of group.hits) {
      const line = hit.excerpt.trim();
      if (line.length < RANKING_WEIGHTS.dedup.minExcerptLength || seen.has(line)) continue;
      seen.add(line);
      const entry = byLine.get(line);
      if (entry === undefined) {
        byLine.set(line, { files: 1, canonical: group.file });
        continue;
      }
      entry.files += 1;
      const canonical = groups.find((g) => g.file === entry.canonical) as ClassifiedGroup;
      const better =
        RANKING_WEIGHTS.fileClass[group.fileClass] > RANKING_WEIGHTS.fileClass[canonical.fileClass]
          ? group.file
          : RANKING_WEIGHTS.fileClass[group.fileClass] ===
                RANKING_WEIGHTS.fileClass[canonical.fileClass] && group.file < canonical.file
            ? group.file
            : canonical.file;
      entry.canonical = better;
    }
  }
  return groups.map((group) => {
    const hits: ClassifiedHit[] = [];
    for (const hit of group.hits) {
      const line = hit.excerpt.trim();
      const entry = byLine.get(line);
      if (entry === undefined || entry.files < RANKING_WEIGHTS.dedup.minFiles) {
        hits.push(hit);
        continue;
      }
      if (entry.canonical !== group.file) continue; // elided: counted on the canonical entry
      hits.push({ ...hit, duplicates: entry.files - 1 });
    }
    return { ...group, hits };
  });
}

/** Query tokens worth a path-relevance bonus: alphanumeric runs, length ≥ 3. */
function queryTokens(query: string): readonly string[] {
  return (query.toLowerCase().match(/[a-z0-9_]{3,}/g) ?? []).slice(0, 8);
}

/** Stage 5's path relevance: token-in-path bonus minus depth penalty. */
export function pathRelevance(file: string, tokens: readonly string[]): number {
  const path = file.toLowerCase();
  const matches = tokens.filter((t) => path.includes(t)).length;
  const depth = file.split("/").length;
  return RANKING_WEIGHTS.path.segmentMatch * matches - RANKING_WEIGHTS.path.depthPenalty * depth;
}

function bestHitScore(group: ClassifiedGroup, pluginFeatures?: PluginFeatureLookup): number {
  let best = 0;
  for (const hit of group.hits) {
    const boost =
      pluginFeatures === undefined
        ? 0
        : pluginBoost(
            pluginFeatures.get(pluginFeatureKey(group.file, hit.line, hit.column, hit.excerpt)),
          );
    best = Math.max(best, hitScore(hit) + boost);
  }
  return best;
}

function confidenceRank(group: ClassifiedGroup): number {
  let best = 0;
  for (const hit of group.hits) {
    if (hit.confidence !== undefined) {
      best = Math.max(best, RANKING_WEIGHTS.confidence[hit.confidence]);
    }
  }
  return best;
}

/**
 * Stage 5 — group ordering: definition-carrying groups first (law #2), then
 * references confidence, then the strict file-class tier, then the combining
 * model's score (best hit × confidence + path relevance), then hit count,
 * then path. Deterministic by construction.
 */
export function orderGroups(
  groups: readonly ClassifiedGroup[],
  tokens: readonly string[],
  pluginFeatures?: PluginFeatureLookup,
): ClassifiedGroup[] {
  const scored = groups.map((group) => ({
    group,
    hasDefinition: group.hits.some((h) => h.kindClass === "declaration"),
    confidence: confidenceRank(group),
    tier: RANKING_WEIGHTS.fileClass[group.fileClass],
    score: bestHitScore(group, pluginFeatures) + pathRelevance(group.file, tokens),
  }));
  scored.sort((a, b) => {
    if (a.hasDefinition !== b.hasDefinition) return a.hasDefinition ? -1 : 1;
    if (a.confidence !== b.confidence) return b.confidence - a.confidence;
    if (a.tier !== b.tier) return b.tier - a.tier;
    if (a.score !== b.score) return b.score - a.score;
    if (a.group.totalHits !== b.group.totalHits) return b.group.totalHits - a.group.totalHits;
    return a.group.file < b.group.file ? -1 : a.group.file > b.group.file ? 1 : 0;
  });
  return scored.map((s) => s.group);
}

/** Strip the ranking-only fields before a hit goes on the wire. */
function wireHit(hit: ClassifiedHit): SearchHit {
  const { kindClass: _kindClass, ...wire } = hit;
  return wire;
}

/**
 * Stages 1–5 composed: candidates in, wire-ready ranked groups out. Stage 6
 * ({@link trimToBudget}) runs separately so the caller owns the manifest.
 */
export function rankGroups(
  candidates: readonly CandidateGroup[],
  query: string,
  pluginFeatures?: PluginFeatureLookup,
): SearchGroup[] {
  const tokens = queryTokens(query);
  const classified: ClassifiedGroup[] = candidates.map((candidate) => ({
    file: candidate.file,
    junk: candidate.junk,
    test: candidate.test,
    fileClass: fileClassOfGroup(candidate),
    hits: orderHits(
      candidate.hits.map((hit) => ({
        ...hit,
        kindClass: classifyHit(hit, query),
        ...(inTestRange(hit.line, candidate.testRanges) ? { inTestRange: true } : {}),
      })),
    ),
    totalHits: candidate.hits.length,
  }));
  const deduped = dedupAcrossFiles(classified);
  return orderGroups(deduped, tokens, pluginFeatures).map((group) => ({
    file: group.file,
    junk: group.junk,
    totalHits: group.totalHits,
    hits: group.hits.slice(0, RANKING_WEIGHTS.maxHitsPerGroup).map(wireHit),
  }));
}

/** Estimated token cost of one group on the wire (Constitution II). */
export function groupTokens(group: SearchGroup): number {
  return Math.ceil(JSON.stringify(group).length / RANKING_WEIGHTS.charsPerToken);
}

/**
 * Stage 6 — trim to budget: keep whole groups until the estimate is spent.
 * `keep` is 0 when even the first group cannot fit (the caller answers
 * BUDGET_TOO_SMALL); the caller attaches the truncation manifest via
 * `truncateResults` so a cut is never silent.
 */
export function trimToBudget(
  groups: readonly SearchGroup[],
  budget: number,
): { keep: number; used: number } {
  let used = 0;
  let keep = 0;
  for (const group of groups) {
    const cost = groupTokens(group);
    if (keep > 0 && used + cost > budget) break;
    if (keep === 0 && cost > budget) break;
    used += cost;
    keep += 1;
  }
  return { keep, used };
}
