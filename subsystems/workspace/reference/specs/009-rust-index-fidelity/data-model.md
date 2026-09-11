# Phase 1 Data Model: Rust Index Fidelity

One cache-class store changes shape (the index), and two in-flight shapes gain optional
fields. No durable-class data is touched, so no migration is written (research R7).

## Changed: the symbol index (`.workbench/index.db`), `INDEX_SCHEMA_VERSION` 1 → 2

```
symbols(path, name, kind, start_line, start_col, end_line)
      + detail TEXT NULL     -- trait name for a trait-impl member; NULL otherwise
      + doc    TEXT NULL     -- first line of the item's doc comment; NULL otherwise

+ test_ranges(path TEXT NOT NULL, start_line INTEGER NOT NULL, end_line INTEGER NOT NULL)
  -- one row per #[cfg(test)] module; rebuilt with the file, deleted with the file
```

Rules:
- `name` for an impl member is `Type::member`; for a free item it is unchanged (bare).
- `detail` is set only when the member came from `impl Trait for Type`.
- `doc` is trimmed, single-line, and length-bounded (see Constants).
- `test_ranges` rows are per file and inclusive on both ends.
- Version 1 on disk ⇒ discarded and rebuilt (existing cache-class behavior, FR-008).

## Extended (additive): `Symbol` / `SymbolEntry` / outline rows

```
Symbol (Rust core)          + detail: Option<String>, doc: Option<String>
SymbolEntry (src/ports)     + detail?: string, doc?: string
MapSymbol / ReadSymbolRef   + doc?: string   (detail surfaces via ambiguity candidates)
Extraction (Rust core)      + test_ranges: Vec<(u32, u32)>
```

Absent means absent: a symbol with no doc carries no `doc` key at all (FR-007, SC-005).

## Extended (additive): ranking candidate

```
CandidateFile  + testRanges?: readonly {start: number, end: number}[]
```

Classification becomes hit-aware for files that carry ranges:

```
hit line inside a test range        -> fileClass "test"   (file may still be source)
file junk                          -> "vendor"           (unchanged, wins)
file path is test-like             -> "test"             (unchanged)
otherwise                          -> "source"           (unchanged)
```

`fileClassOf(group)` keeps its current signature and meaning; the hit-aware decision is a new
function used where hits are scored, so every non-Rust caller is untouched (FR-011).

## Extended (additive): eval corpus row

```
candidates[].testRanges?: [{start, end}, ...]
```

Rows without the field behave exactly as today, so existing expectations and the recorded
baseline stay comparable (research R8).

## Test-path segments (`isTestPath`)

```
existing: test, tests, __tests__, spec, specs  +  *.test.*, *.spec.*, *_test.*
added:    benches, examples
```

## Junk directories (verification only, no change)

`target` is already in `JUNK_DIRS` alongside `node_modules`, `vendor`, … — pinned by test in
this feature, not modified (research R5).

## Constants

| Name | Value | Where |
|---|---|---|
| `INDEX_SCHEMA_VERSION` | 1 → **2** | src/adapters/engine-ipc/indexer.ts |
| doc summary max length | 200 chars | crates/workbench-core/src/indexer.rs |
| `RANKING_WEIGHTS.fileClass` | unchanged (source 2, test 1, vendor 0) | src/tools/ranking.ts |
