# Phase 0 Research: Rust Index Fidelity

Grounded in `crates/workbench-core/src/indexer.rs` (`symbol_kind`, `visit`, `extract`,
`JUNK_DIRS`/`is_junk`), `src/adapters/engine-ipc/indexer.ts` (`INDEX_SCHEMA_VERSION`, the
`symbols` table, `querySymbols`, `queryOutline`), `src/tools/read.ts` (`resolveSymbol`,
`extentOf`, the `AMBIGUOUS_SYMBOL` envelope), `src/tools/search.ts` (`isTestPath`,
`scopeFiles`, candidate building), `src/tools/ranking.ts` (`FileClass`, `fileClassOf`,
`RANKING_WEIGHTS`), `src/tools/map.ts` (overview notes), `eval/ranking-corpus.jsonl` +
`scripts/eval-ranking.ts`. Pinned in think:157.

## R1 — Qualify with `Type::member`, because the SQL already tolerates it

**Decision**: Impl members are stored with `name = "Type::member"` and their own kinds
(`method` for fns in an impl, `const`/`type` for associated items). Inherent and trait impls
both qualify by the implementing type.

**Rationale**: `querySymbols` filters with `` `%${name}%` `` — a substring match — so a bare
search for `load` still finds `Config::load` (FR-004 holds by construction, not by a second
index). `read`'s `resolveSymbol` tries a literal `row.name === address` first, so
`Config::load` resolves directly with no dialect change at all; the dialect widening (R3) is
only for the fallback path.

**Alternatives considered**: a second `qualified_name` column with the bare name kept in
`name` (rejected: two names for one symbol, and every consumer has to choose — the outline,
the ambiguity list, and search would drift); storing the full module path
(`crate::config::Config::load`) (rejected: the address dialect is file-scoped today, so a
module path is unaddressable noise — spec Assumptions record it as a separate decision).

## R2 — The trait needs its own column, because one name cannot be both address and disambiguator

**Decision**: add a nullable `detail` column to the `symbols` table carrying the trait name
for trait-impl members (empty/NULL otherwise). `AMBIGUOUS_SYMBOL` candidates render
`<Type as Trait>::member` when `detail` is present, `Type::member` otherwise.

**Rationale**: the chunk requires both "`Type::method` addressable" and "`<Type as Trait>::method`
distinguishable". If the stored name were the fully-qualified form, the natural address people
type (`English::hello`) would stop resolving; if the trait were dropped, the two same-named
members in one file would be indistinguishable in the candidate list — exactly the case the
chunk names. A separate column keeps the address ergonomic and the ambiguity honest.

**Alternatives considered**: encoding the trait into `kind` (rejected: kind is a small closed
vocabulary shared across languages and consumed by ranking); rendering the trait from the
source line at ambiguity time (rejected: the ambiguity path would have to re-read and re-parse
the file to answer an error).

## R3 — Widen the address dialect to accept `::`, additively

**Decision**: `resolveSymbol` splits the address on `.` **and** `::`. Literal matching is
tried first (unchanged), so this only affects the member/parent containment fallback. `edit`
inherits it (one dialect, shared code path). The two tool schemas' description text gains the
Rust example.

**Rationale**: `Foo::new` should behave like `Foo.new` for a language whose separator is `::`.
Because the literal branch runs first and qualified names now exist, most Rust addresses never
reach the split at all; the widening is what makes a *partial* address (`Config::load` in a
file where the symbol is stored differently, or a nested case) degrade sensibly instead of
missing.

**Alternatives considered**: a Rust-only dialect (rejected: one dialect shared by read and
edit is a stated design property); normalizing `::` to `.` at the boundary (rejected: it would
also rewrite legitimate names containing dots).

## R4 — `#[cfg(test)]` is a RANGE, not a file property

**Decision**: extraction records, per file, the line ranges of modules carrying a
`#[cfg(test)]` attribute. The index stores them (a small `test_ranges` table keyed by path).
Search's candidate builder consults them so a hit whose line falls inside a range is
classified test-class even though its file is source-class; `fileClassOf` keeps its current
signature for callers with no ranges.

**Rationale**: Rust's unit-test convention puts tests *inside* the file under test, so every
per-file rule (`isTestPath`) is structurally unable to see them — the single largest Rust
ranking gap. Ranges are cheap to extract (an attribute check on `mod_item`), cheap to store,
and turn the existing three-class ranking into the right answer without touching weights.

**Alternatives considered**: classifying the whole file as test when it contains a `cfg(test)`
module (rejected: it would demote the production code in the same file — the opposite of the
goal); marking only symbols, not hits (rejected: ranking scores hits, and most hits inside a
test module are call sites, not declarations); a heuristic on the excerpt text (rejected:
guessing from a line is exactly what an index exists to avoid).

## R5 — `benches/`/`examples/` join `isTestPath`; `target/` is already junk

**Decision**: add `benches` and `examples` to the test-path segments. Verify — and pin by test
— that `target/` is already junk (`JUNK_DIRS` in `indexer.rs` contains `"target"`).

**Rationale**: the chunk lists `target/` as an acceptance item; it is already true, so the
honest unit is a verification, not a change. Adding it as "work" would claim credit for
shipped behavior.

## R6 — Doc summaries: first line, stored beside the symbol

**Decision**: extraction reads the doc comment attached to an item (`///` line comments
immediately preceding, or a `/** */` block) and stores its first line, trimmed and length-
bounded, in a new nullable `doc` column. Outline rows (map `detail >= outline`, read's symbol
ref) surface it when present; absent means absent, never `""`.

**Rationale**: the text is already in the file and the outline is already built; the only cost
is a column and a small walk backwards over preceding siblings. Length bounding keeps the
envelope budget honest (Constitution II).

**Alternatives considered**: storing the whole doc comment (rejected: the outline is a
skeleton, and the full text is one `read` away); computing summaries at query time (rejected:
re-reading and re-parsing every file to render an outline defeats the index).

## R7 — Schema bump is free, because the index is cache-class

**Decision**: `INDEX_SCHEMA_VERSION` 1 → 2 carrying both new columns (`detail`, `doc`) and the
new `test_ranges` table. No migration is written: an index at version 1 is discarded and
rebuilt, which is the existing behavior for cache-class data.

**Rationale**: Constitution V: "Cache-class data (index, telemetry) is rebuildable by any
binary version and MUST be discarded and rebuilt rather than misread." The rebuild path
already exists and is already tested; a migration would be new machinery for data we are
explicitly allowed to throw away.

## R8 — Eval: extend the corpus additively and report both directions

**Decision**: `eval/ranking-corpus.jsonl` gains ≥8 Rust rows. Candidate files gain an optional
`testRanges` array so an inline-`cfg(test)` case is expressible; rows without it behave
exactly as today. Queries cover: production-vs-`cfg(test)` hit in one file; `benches/` and
`examples/` demotion; `target/` junk; a qualified impl method found by bare name; a
`Type::method` search; a doc-carrying item (presence, not ranking); and two mixed cases with
non-Rust candidates to prove no cross-language regression. `bun run eval:rank --record`
rewrites `eval/ranking-baseline.json` in the same change, and the before/after MRR/Hit@1/Hit@5
go into the ship trace.

**Rationale**: Constitution VI makes the number the merge gate, and the corpus is the only
place a ranking change can be argued. Extending the candidate shape additively keeps every
existing row's meaning intact, so the "no non-Rust regression" claim is checkable.

## R9 — The macro note is a note

**Decision**: `map`'s overview notes gain one sentence, emitted only when the workspace's
language census contains Rust: macro-generated items (derives, `macro_rules!`) are not in the
symbol index; use text search or the resolver for them.

**Rationale**: the SDD's honesty rule — surface what the tool cannot do where the agent will
read it — and the chunk's fifth acceptance. Gating on the census keeps it out of workspaces it
does not describe.
