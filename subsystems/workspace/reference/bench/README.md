# Recorded perf-gate runs

The constitution (principle VI) names four perf gates: tier-0 usefulness within 5 s, 500k LOC
to tier 1 in under 60 s, references p95 under 50 ms at 1M LOC, IPC round-trip p95 under 1 ms.
The JSON files under `results/` are the recorded runs, one per (script, profile), all from one
session on one machine; the table below is read from them. A failing gate is recorded, not
hidden. Re-record with the commands at the end; commit the JSON beside the code that changed
the number.

Machine: AMD Ryzen 9 7950X3D (32 threads), 63 GB, Windows 11 x64, bun 1.4.0, native core.
Recorded 2026-08-24 on the working tree of the commit that landed specs/017 (the JSON's
`workbenchCommit` names that commit's parent, `5b7364c`, because the runs preceded the commit);
the IPC record is from `e00443b`. The pre-017 numbers are kept in the text below for the
before/after.

## Index (`bench-index.ts`)

| profile | corpus | tier-0 | tier-1 | tier-1 per 500k LOC | gates |
|---|---|---|---|---|---|
| synthetic-500k | 504,019 LOC / 2,015 files | 0.14 s | 1.19 s (2,015/2,015 files) | 1.18 s | tier0 pass · tier1 pass |
| synthetic-1m | 1,008,019 LOC / 4,015 files | 0.13 s | 2.38 s (4,015/4,015 files) | 1.18 s | tier0 pass · tier1 pass |
| rust-lang/rust @ da51146 | 4,402,825 LOC / 62,197 files (37,821 source, 1,539 junk; rust 3,956,694) | 0.14 s | 63.48 s (62,197/62,197 files) | 7.21 s | tier0 pass · tier1 pass |

The tier-1 gate is a claim about 500k LOC; above that the table judges the rate (seconds per
500k LOC) and the JSON's gate note says so.

## References (`bench-refs.ts`)

| profile | corpus | calls | p50 | p95 | max | hot query | gate |
|---|---|---|---|---|---|---|---|
| synthetic-500k | 506,003 LOC / 2,001 files | 120 | 1.20 ms | 5.20 ms | 7.90 ms | shared_ref_target, 1,033 edges (p95 6.18 ms) | pass |
| synthetic-1m | 1,012,003 LOC / 4,001 files | 120 | 2.24 ms | 6.55 ms | 8.42 ms | shared_ref_target, 1,033 edges (p95 6.81 ms) | pass |
| rust-lang/rust @ da51146 | 4,402,825 LOC / 62,197 files | 280 | 42.74 ms | **342.57 ms** | 368.92 ms | `main` (24,042 defs, capped at 200; 260 edges) p95 347 ms; `foo` p95 351 ms; `Foo` p95 351 ms; rares p95 45–49 ms; miss p95 7.2 ms | **FAIL** |

## IPC (`bench-ipc.ts`, stub core, 2,000 pings)

| p50 | p95 | p99 | max | gate |
|---|---|---|---|---|
| 0.032 ms | 0.056 ms | 0.076 ms | 1.443 ms | pass |

## What the real-repository runs found

**Fixed (specs/017): the tier-1 build was O(n²) in file count.** The first run over
rust-lang/rust reached 17,856 of 62,197 files in 900 s and never hit tier 1; the build rate
halved whenever the corpus doubled (183 files/s at 2k files, 91 at 4k, 20 over rust-lang/rust).
The term was `DELETE FROM content_fts WHERE path = ?` in the per-file write path: `path` is an
`UNINDEXED` FTS5 column, so every delete scanned the whole table, even on a fresh build with
nothing to delete (0.23 ms per delete at 2k rows, 9.8 ms at 32k, 19.8 ms at 64k). The full-text
row now carries its `files` row id and is deleted by it (0.010 ms at 64k rows), and a path with
no `files` row issues no delete. Before → after: synthetic 500k 11.07 s → 1.19 s, synthetic 1M
44.29 s → 2.38 s (per 500k LOC: 21.97 s → 1.18 s, now flat), rust-lang/rust not reached in 900 s
→ 63.48 s for 62,197 files (7.21 s per 500k LOC).

**Open: references p95 over rust-lang/rust is 342.57 ms against a 50 ms gate.** The shape of
the miss is in the per-query rows: the three hot names (`main` 24,042 definitions, `foo` 4,295,
`Foo` 4,151) cost ~300 ms each; the three single-definition names cost ~36 ms (p95 45–49 ms,
just under the gate); the miss costs 2.9 ms. So the floor for any name that has a definition
is ~35 ms above the miss, and the hot names pay a further ~265 ms. Filed as
`chunk:references-p95-real-repo` with those numbers; the constitution's flip trigger for the
no-graph-DB refusal ("reopens if references p95 misses its gate") is now armed by data.

The synthetic 500k profile (what CI gates) could see neither finding: one shared hot symbol in a
2k-file corpus of near-identical modules is not a 62k-file repository with 24k `main`s.

## Re-recording

```
bun run bench:ipc   -- --json bench/results/ipc.json
bun run bench:index -- --json bench/results/index-synthetic-500k.json
bun run bench:refs  -- --json bench/results/refs-synthetic-500k.json
bun run bench:index -- --loc 1000000 --json bench/results/index-synthetic-1m.json
bun run bench:refs  -- --loc 1000000 --json bench/results/refs-synthetic-1m.json
bun run bench:index -- --root D:/Code/_ext/rust --json bench/results/index-rust-lang-rust.json
bun run bench:refs  -- --root D:/Code/_ext/rust --json bench/results/refs-rust-lang-rust.json
```

`--root` never modifies the tree; it creates `<root>/.workbench/` and removes it at exit
(`--keep-index` keeps it, and a pre-existing one is refused). The corpus is a shallow clone:
`git -c core.longpaths=true clone --depth 1 --no-tags --single-branch --branch main
https://github.com/rust-lang/rust.git D:/Code/_ext/rust` (38,425 `.rs` files, 3,997,539 lines
of Rust at `da51146`, no submodules). LOC in the table is counted by the bench from the index's
own files table (source rows that are not junk), so it covers exactly what the index covered.
