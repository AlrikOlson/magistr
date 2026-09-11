# Implementation Plan: Rust Index Fidelity

**Branch**: `009-rust-index-fidelity` | **Date**: 2026-08-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/009-rust-index-fidelity/spec.md`

## Summary

Teach the Tier-1 indexer Rust's actual structure. Impl members (methods, associated
consts/types) are extracted with qualified names `Type::member` plus the trait they came from;
the shared symbol-address dialect learns `::`; `#[cfg(test)]` module ranges are extracted and
stored so a hit inside one ranks as test code even in a source file; `benches/`/`examples/`
join the test-path rules (`target/` is already junk and is only verified); items carry the
first line of their doc comment into outlines; and the workspace map states plainly that
macro-generated items are not indexed. The index is cache-class, so the schema moves 1 → 2 and
an old index is discarded and rebuilt rather than migrated. The change lands with ≥8 new Rust
eval queries and before/after MRR/Hit@1/Hit@5. Substrate pinned in think:157 (R1–R9).

## Technical Context

**Language/Version**: Rust (extraction) + TypeScript (index, ranking, tools, eval).

**Primary Dependencies**: bundled `tree-sitter-rust` (already present — no new dependency);
`crates/workbench-core/src/indexer.rs`; `src/adapters/engine-ipc/indexer.ts` (schema,
`querySymbols`, outline rows); `src/tools/read.ts` (`resolveSymbol`, ambiguity envelope);
`src/tools/search.ts` (`isTestPath`, candidate building); `src/tools/ranking.ts` (`FileClass`);
`src/tools/map.ts` (overview notes); `eval/ranking-corpus.jsonl` + `scripts/eval-ranking.ts`.

**Storage**: `.workbench/index.db` — two nullable columns on `symbols`, one new `test_ranges`
table, `INDEX_SCHEMA_VERSION` 1 → 2, no migration (cache-class rebuild).

**Testing**: `cargo test` (extraction: qualification, trait detail, assoc items, cfg(test)
ranges, doc summaries, junk verification); `bun test` (address dialect incl. `::`, ambiguity
candidates, bare-name search still finds qualified symbols, hit-aware ranking, outline doc
surfacing, map note presence/absence, schema-bump rebuild); `bun run eval:rank --record`.

**Target Platform**: unchanged (native + wasm fallback; extraction is bundled-grammar only).

**Performance Goals**: extraction stays within the existing Tier-1 gate (500k LOC < 60s); the
added work is one attribute check per `mod_item` and one backwards sibling walk per item.

**Constraints**: non-Rust languages unaffected (FR-011); ranking weights untouched; no new
dependency; the address dialect widens additively; absent doc means absent.

**Scale/Scope**: one grammar's extraction rules, one schema bump, one ranking classification
path, one map note, ≥8 eval rows.

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS.*

- **I (grep cost structure)**: no new mandatory work at query time; the index stays optional
  and rebuildable; a version-1 index is rebuilt, never misread.
- **II (rank, don't dump)**: this IS a ranking-fidelity change — hits inside `cfg(test)`
  modules stop competing with production code — and it merges only with eval numbers.
- **III (verified writes)**: `edit` inherits the widened address dialect; no write-path
  behavior changes otherwise.
- **IV (typed escape hatches)**: untouched.
- **V (versioned contracts)**: index is cache-class → discard and rebuild (explicitly allowed,
  no migration debt); envelope/tool schemas gain only optional fields and description text.
- **VI (no number, no merge)**: ≥8 Rust eval queries; before/after MRR/Hit@1/Hit@5 recorded;
  no non-Rust query may regress.
- **Honesty**: the macro limitation is stated in map's index health rather than left for an
  agent to discover by getting a wrong answer.

## Project Structure

### Documentation (this feature)

```text
specs/009-rust-index-fidelity/
├── plan.md
├── research.md           # R1–R9
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/workbench-core/src/indexer.rs      # impl qualification + trait detail, assoc items,
                                          #   cfg(test) ranges, doc summaries, Rust tests
src/adapters/engine-ipc/indexer.ts        # schema v2 (+detail,+doc,+test_ranges), row plumbing
src/ports/index-port.ts                   # SymbolEntry + detail?/doc? (additive)
src/tools/read.ts                         # `::` in the dialect; qualified ambiguity candidates
src/tools/search.ts                       # benches/examples; hit-aware test classification
src/tools/ranking.ts                      # hit-aware class helper (fileClassOf unchanged)
src/tools/map.ts                          # macro-scope note, Rust-gated
schemas/tools/{read,edit}.input.schema.json  # address description gains the Rust example
eval/ranking-corpus.jsonl                 # ≥8 Rust rows (+ optional testRanges)
eval/ranking-baseline.json                # re-recorded in this change
test/*.test.ts                            # dialect, ranking, outline, map-note, rebuild pins
```

**Structure Decision**: extraction facts are produced in Rust (where the parse already is),
stored by the existing index adapter, and consumed by the existing ranking/read/map paths. No
new module and no import-graph layer change.

## Complexity Tracking

No constitution violations; table intentionally empty.

## Deviations (recorded at build, 2026-08-23)

1. **Trait members are qualified too, because they were invisible.** A trait's
   own method signature is a `function_signature_item`, which no kinds-table
   arm ever matched — so `trait Greeter { fn hello(); }` produced NO symbol at
   all before this feature. `rust_qualified_members` therefore serves `impl`
   and `trait` bodies alike, emitting `Greeter::hello` (kind `method`, no
   `detail`, since a trait owns its own name). Discovered by a test assertion
   that assumed the bare name existed; the assumption was wrong about shipped
   behavior, and the gap was worth closing here rather than filing.

2. **The `from: 0` index migration was REMOVED, not extended.** It adopted a
   pre-runner schema-1 database in place; at schema 2 that would leave an
   index with no `detail`/`doc` columns and no `test_ranges` table — a
   silently half-shaped cache, the exact failure the version stamp exists to
   prevent. Removing it routes every pre-v2 database through
   `openVersionedDatabase`'s "no migration chain … rebuilt" path, which is
   what cache-class means (Constitution V). `test/migrations.test.ts`'s
   "adopted in place, index warm" test was deliberately inverted to
   "DISCARDED and rebuilt" and says why in a comment.

3. **A bare member address needed a new fallback.** Qualification alone would
   have REGRESSED `read symbol:"load"` for Rust: rows now store
   `Config::load`, and neither the literal branch nor the member/parent split
   matches a bare `load`. `resolveSymbol` gained a qualified-suffix fallback
   (`endsWith("::load")` / `endsWith(".load")`), so qualification is a
   precision gain rather than a new thing to type (FR-004). Caught by the test
   written for the spec, not by review.

4. **One eval row was replaced rather than made to pass.** A draft row asked
   which file answers `Greeter::hello` — the trait declaration or its
   implementation — and ranked the impl first. That label is genuinely
   arguable, so the row was replaced with `rust-qualified-name-picks-the-right-type`
   (`Config::load` between `Config` and `Session` impls), whose label is not.
   A corpus row that only passes because a coin landed one way is worse than
   no row.

5. **`target/` demotion is NOT strict.** The gated search test found that
   `orderGroups` sorts `hasDefinition` ahead of file-class tier, so a
   generated file under `target/` that contains a declaration precedes
   `benches/`/`examples/` files whose hits are call sites. Source-with-
   declaration still beats junk-with-declaration, which is the property that
   matters, and the test asserts what is true rather than what the SDD's
   "strictly tiered" wording implies. Filed as
   `chunk:ranking-tier-vs-definition-first` — changing it needs its own
   before/after MRR, and this chunk's constraint was "ranking weights
   untouched".

6. **Incidental rustfmt churn reverted.** `cargo fmt --all` reformats four
   files this chunk never touched (the tree is not rustfmt-clean at HEAD, so
   CI's Rust Format step is red independently of any change). The churn was
   reverted to keep this diff reviewable and filed as
   `chunk:rustfmt-drift-on-main`.

### SC-004 (Constitution VI, recorded)

| | corpus | MRR | Hit@1 | Hit@5 |
|---|---|---|---|---|
| before | 16 queries | 0.9375 | 0.875 | 1.0 |
| after | 26 queries | **0.9615** | **0.9231** | 1.0 |

No non-Rust regression: with the ranking change in place and the corpus still
at its original 16 rows, `eval:rank` reported the identical 0.9375 / 0.875 /
1.0 and the baseline check passed. All ten new Rust rows rank their expected
file first; the only Hit@1 misses in the extended corpus are the two
pre-existing adversarial rows.
