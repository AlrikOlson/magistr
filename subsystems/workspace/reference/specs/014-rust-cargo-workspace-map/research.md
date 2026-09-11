# Phase 0 Research: Cargo Workspace Map

Grounded in `src/tools/map.ts` (overview row + breadth-first tree rows; root-manifest entry
points read only when the walk listed them; `queryImportGraph({limit})` summary; per-row budget
cut with `truncateResults`, cursor `path:<row.path>`, narrowing hints; `MAP_ENTRY_POINTS_MAX`
as the "summary, not inventory" precedent), `test/map.test.ts` (`fakeDeps` builds a full
`IndexPort` literal and a walk with optional content), `src/ports/index-port.ts` and
`versions.ts` (IndexPort v8; additive evolution log), `src/adapters/engine-ipc/indexer.ts`
(imports table `(path, target)` with the raw specifier — `use alpha::add;` stores
`alpha::add`; `index/importGraph` IPC call; fan-out verified, fan-in a GROUP BY),
`src/adapters/inprocess/inprocess.ts` (records no imports; answers empty), twelve test files
that construct fake `IndexPort`s, `src/tools/rust-move.ts` (`crateDirOf`,
`manifestTargetPaths`, the AUTO_ROOT convention regex), `scripts/check-import-graph.ts` (tools
may import ports and envelope only — not config), and this run's `src/config/detect.ts`
`cargoLayout` (the textual manifest idiom, unreachable from tools). Pinned in think:176/177.

## R1 — One `crate` row per member, between the overview and the tree; a summary on the overview

**Decision**: a new result row kind `crate` with `path` = the member directory (so the
existing `path:` cursor and narrowing work unchanged), emitted after the overview and before
the tree rows; the overview gains an optional `cargo` summary `{ kind, members, edges,
omitted?, unusedDeps?, top }`.

**Rationale**: rows are what the budget cut trims, so "budget trim still honest on a large
workspace" is the existing manifest, not a second mechanism. Crate rows are workspace-level
facts like the overview, so they precede the tree; they are ranked (R4) so a cut keeps the
load-bearing ones.

**Alternatives considered**: everything on the overview row (rejected: one row no budget can
trim); crate rows after the tree (rejected: on a monorepo the crate list IS the orientation).

## R2 — Tier-0 sources: manifests through the walk, no filesystem access, no TOML parser

**Decision**: root `Cargo.toml` `[workspace] members`/`exclude` read textually (section split
+ quoted-string arrays); a `<dir>/*` entry expands to the walk's `<dir>/<child>/Cargo.toml`
paths; other glob forms are reported in a note; a root `[package]` is a member with directory
`.`; each member manifest is read through `workspace.read` only when the walk listed it, at
most 256 per call (the overview notes the rest). Per member: `[package] name` and `edition`,
`[lib]` (`name`, `proc-macro`, `path`), `[[bin]]`/`[[example]]`/`[[test]]`/`[[bench]]`
(`name`, `path`), `[features]` keys, and the dependency tables `[dependencies]`,
`[dev-dependencies]`, `[build-dependencies]` plus any `[target.….dependencies]` variant.

**Rationale**: the entry-point precedent (manifests only when the walk saw them, parse
failures detect nothing) keeps map Tier-0 and jail-clean; the textual idiom is the one `map`,
specs/010, 012 and 013 already use.

## R3 — Roles and entry points from targets first, conventions second

**Decision**: roles: `lib` when `src/lib.rs` or `[lib]` exists (`proc-macro` when `[lib]
proc-macro = true`), `bin` when `src/main.rs`, `src/bin/*` or `[[bin]]` exist, `build` when
`build.rs` exists. Entry points: manifest `path =` targets (kinds lib/bin/example/test/bench,
with names), then convention (`src/lib.rs`, `src/main.rs`, `src/bin/<n>.rs`,
`src/bin/<n>/main.rs`, `examples/*`, `benches/*`, `tests/*`, `build.rs`) from the walk, deduped
by path, capped at 8 per row with `moreEntryPoints`.

**Rationale**: `rust-move.ts` already encodes the convention (AUTO_ROOT) and the manifest
target reader; reusing them keeps one definition of "crate root".

## R4 — Ranked and capped: dependents first, 24 rows, per-row caps

**Decision**: rows ordered by `dependents` descending then name; `MAP_CRATE_ROWS_MAX = 24`;
the overview's `cargo.omitted` counts the rest and a note says `map path:<dir>` narrows;
features capped at 16 (`moreFeatures`), entry points at 8 (`moreEntryPoints`); dependencies
are listed in full (bounded by the member count).

**Rationale**: Constitution II. A 200-crate workspace at the default budget must still show
its tree; the cap is the same rule `MAP_ENTRY_POINTS_MAX` applies to entry points.

## R5 — The Cargo edge set: member dependencies with kind; externals counted

**Decision**: a dependency entry names a member when its `path` resolves (from the member's
directory) to a member directory, when `workspace = true` and the root `[workspace.dependencies]`
entry's `path` resolves to a member, or when its key (or `package = "…"` rename) equals a
member's package name; kind from the table (normal/dev/build). Non-member entries increment
`externalDependencies`. `dependents` = the number of members whose dependencies name this one.

**Rationale**: exact from manifests, no cargo invocation; externals are a count because the
graph is the workspace's, not crates.io's.

## R6 — The `use` half: an additive extension of `queryImportGraph`, not a new port method

**Decision**: `ImportGraphQuery` gains `crates?: readonly { name: string; dir: string }[]`
(name = the `use` name: `[lib] name`, else the package name with `-` → `_`; dir = the member
directory, `.` for root); `ImportGraphResult` gains `crates?: readonly { from: string; to:
string; files: number }[]`. The engine child (indexer.ts) answers by selecting import rows whose
target equals a name or starts with `<name>::`, attributing each row's path to the LONGEST
member directory prefixing it (root `.` catches the rest), and counting distinct files per
(from, to). IPC: the same `index/importGraph` call with the optional field. `PORT_VERSIONS.index`
8 → 9 (additive). The in-process adapter answers no `crates` (it records no imports). Map
passes the crates only when the index is at Tier 1, attaches `files` to member dependencies
whose target matches an edge, sums `unusedDeps` (declared member deps with no edge), and on a
throw or a missing `crates` field degrades with the note "use counts unavailable".

**Rationale**: twelve test files build fake `IndexPort`s; a new required method touches all of
them for one caller. An optional field on the existing query is additive under the port's own
rules, keeps every fake conformant, and the child already has the table and the SQL.

**Alternatives considered**: a new `queryCrateImportGraph` method (rejected: twelve fakes, one
caller); aggregating the existing top-N fan-in client-side (rejected: top-N by specifier cannot
produce crate totals); reading `.rs` sources in map (rejected: Tier-0 cost on 1M LOC).

## R7 — Counts are not path-verified; freshness is the index's live freshness

**Decision**: crate edges are counts; the §7.3.3 verification loop runs per path and would
cost thousands of stats for a number, so the edges carry the index's live freshness and the
envelope's freshness is the worst of its ingredients as today. Recorded in the port doc.

## R8 — Scoping: rows follow `path`, the summary does not

**Decision**: with `path` set, crate rows are those whose directory equals the prefix or lies
under it (the root member only at the root); the overview summary always describes the whole
workspace, like `entryPoints` and `plugins`.

## R9 — Placement and the helper duplication

**Decision**: `src/tools/map-cargo.ts` — `cargoWorkspaceOf(files, read)` (members, rows,
summary), the manifest readers, `crateUseName`; imports `rust-move.ts` for AUTO_ROOT and
`manifestTargetPaths`. The ~30 lines of textual section/array/string readers are DUPLICATED
from `src/config/detect.ts` because tools may not import config; recorded here rather than
hidden. `map.ts` gains the `crate` row type, the `cargo` summary type, the row insertion, the
index call, the note and the caps.

## R10 — Proof

**Decision**: `test/map.test.ts` gains a fixture workspace (glob members, an excluded member, a
root package, a proc-macro crate, a crate with every entry-point kind, `path`/`workspace =
true`/`package =` dependencies, a dev and a build dependency, a hyphenated member, an external
dependency) with a fake index answering `crates` edges ⇒ golden under
`test/support/golden/map/cargo-workspace.expected.json` plus explicit assertions; a failing
index and a silent index ⇒ manifest-only rows + note; a 64-member fixture at a small budget ⇒
manifest, cursor, omitted count; the existing WORKSPACE fixture's envelope pinned byte-identical
(no `cargo`, no crate rows) and every existing map test untouched; `test/indexer.test.ts` (or
the engine-child suite) pins the child's aggregation over real import rows including the
underscore rule and longest-dir attribution; `test/ipc-frames.test.ts`/inprocess pins the
optional field round-trip and the empty answer. Schema description sentence + codegen; skill
row mentions crate rows; `lint:skill` decides.
