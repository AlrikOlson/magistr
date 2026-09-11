/**
 * The port version registry — the "versioned signatures" half of SDD §6.4.
 *
 * Every port interface carries `readonly version` typed from this registry,
 * so an adapter cannot claim a port without naming the signature version it
 * implements. Widening a port's method set is a version bump here, which
 * makes signature evolution explicit in the diff rather than incidental.
 */

export const PORT_VERSIONS = {
  // v2: structural mode answered with kind-tagged matches (m1-search-structural).
  engine: 2,
  // v2: build() kick + build-progress fields on IndexStatus (m1-indexer-tiers).
  // v3: freshness-verified symbol queries + refreshFile (m1-watcher-freshness).
  // v4: queryReferences with confidence provenance (m1-search-structural).
  // v5: queryOutline — per-file symbol outlines for map (m1-map).
  // v6: SymbolEntry.endLine (additive) — the indexer has stored end_line since
  //     Tier 1 existed and the wire already carried it; exposing it lets read
  //     resolve exact symbol extents instead of next-row heuristics (m1-read).
  // v7: build-lifecycle fields on IndexStatus (additive) — buildStartedAtMs /
  //     buildUpdatedAtMs stamped by the engine child, and lastError no longer
  //     dropped at the IPC seam; the stateless index/build task of the MCP
  //     Tasks extension is derived from exactly these (m1-tasks-integration).
  // v8: queryImportGraph — top fan-in/fan-out modules from the imports table,
  //     the import-graph summary of map's overview (m1-map-enrichment).
  // v9: ImportGraphQuery.crates / ImportGraphResult.crates (additive) — per-
  //     crate importing-file counts aggregated by the engine child from the
  //     imports table, for map's Cargo workspace rows (specs/014 R6).
  index: 9,
  // v2: batch-level ops per SDD §10.1 — one op per write batch carrying
  //     {tool, args_hash, session_id, files[], undoOf}, per-file content by
  //     CAS hash with `null` for evicted blobs, append notes for §7.6's
  //     eviction-with-a-note, and undoCandidates for session-scoped undo
  //     (m2-journal-undo).
  // v3: recent() — the log a reader tails (newest first, bounded, each op's
  //     undo eligibility resolved) — and UndoCandidateQuery.seqs, so an undo
  //     can be aimed at named ops instead of only "the newest N"
  //     (chunk:journal-listing-verb).
  // v4: lossless FileContent snapshots for binary copy/move/delete and undo.
  journal: 4,
  watcher: 1,
  sandbox: 1,
  formatter: 1,
  // v2: resolve() answers ResolvedEdge[] — per-edge confidence provenance
  //     (resolver-lsp | tier1-*) instead of bare SourceLocations, so a
  //     symbolic answer can never be mistaken for a language-server-verified
  //     one (m5-lsp-resolver-plugin; the wit/resolver.wit provenance note).
  resolver: 2,
} as const;

export type PortName = keyof typeof PORT_VERSIONS;

export const PORT_NAMES = Object.keys(PORT_VERSIONS) as readonly PortName[];
