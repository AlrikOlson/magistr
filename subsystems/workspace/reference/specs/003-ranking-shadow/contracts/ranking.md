# Contract: workbench:ranking@0.1.0 (consumed unchanged)

Feature: 003-ranking-shadow | Source of truth: `wit/ranking.wit`, blessed in
`wit/baseline.json` (enforced by `crates/workbench-core/tests/wit_contracts.rs`). This
feature consumes the interface as blessed. No WIT vocabulary changes are in scope; the
contract gates must pass with an untouched baseline.

## The interface (as blessed)

- `enum kind-class` — mirrors `KindClass` in `src/tools/ranking.ts` (stage-1 classes;
  `%import`/`%string` are WIT keyword escapes).
- `enum file-class` — mirrors `FileClass` (stage-2 classes).
- `record hit-context` — `{file, line (1-based), column, excerpt, kind-class, file-class}`:
  exactly the facts the stage-5 scorer already sees, and nothing more (no query text).
- `record feature` — `{name: string, value: f64}`.
- `contribute: func(hit: hit-context) -> list<feature>` — the ONLY export. There is no
  ordering, veto, filtering, or scoring verb in the vocabulary; the leash is structural.
- `world ranking-provider` — exports `ranking`, imports nothing.

## Host-side obligations (what the plumbing must guarantee)

1. **Namespacing**: the host prefixes every returned feature name with `<plugin-name>/`
   before it reaches the cache, the log, or the scorer (FR-011). A guest cannot spoof
   another signal's namespace.
2. **Validation**: non-finite or absurdly large values (|v| > 1e6) are dropped at the host
   boundary (spec edge case); an entirely malformed batch counts toward plugin health.
3. **Batching lives in the host lane**: `invoke_ranking_contribute` instantiates once and
   calls `contribute` per hit within one sandbox invocation (runner/lang-pack lane
   precedent). The per-call canonical ABI shape of the blessed contract is preserved.
4. **Off-response-path only**: no code path may await this lane while a search response is
   pending (SC-005, pinned by test).
5. **Application**: a namespaced feature affects a score only via
   `RANKING_WEIGHTS.plugin[<name>]` for a promoted, healthy signal with a warm cache entry
   (FR-002, FR-006). Everything else is logging.

## Guest-side conformance (what the TCK checks over the shipped plugin)

- Declares the `ranking-provider` world; effective capability set resolves EMPTY.
- Pure: same `hit-context` in, same `list<feature>` out; no imports beyond the canonical
  ABI intrinsics.
- Survives the lang-pack-fuzz-style chapter: arbitrary excerpts (including invalid UTF-8
  boundaries the component model already excludes, long lines, empty excerpts) must return
  a well-formed (possibly empty) feature list, never trap.

## Grammar-of-names clause

The first-party signal documents its one feature as `exported` (host-visible as
`ranking-visibility/exported`, value 0.0 or 1.0). Any future feature a signal emits is
additive; removing or re-meaning a published feature name is a breaking change governed by
the same additive-within-a-major rule as the WIT stamps (`wit/README.md`).
