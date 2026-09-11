# Recorded native perf runs

The retained constitution names four perf gates: tier-0 usefulness within 5 s
of first start, 500k LOC to tier 1 in under 60 s, references p95 under 50 ms
(stated at 1M LOC), worker round-trip p95 under 1 ms. The JSON files under
`results/` are the native recordings, one per profile, written by
[`crates/magistr-cli/benches/workspace_bench.rs`](../../../crates/magistr-cli/benches/workspace_bench.rs)
through `./scripts/workspace-bench.sh` in the same `workbench-bench/1` shape
the reference's `bench/results/` use, so the two read alike. A failing gate is
recorded, not hidden. Nothing here measures this checkout: the corpora are
generated in a temp dir and removed at exit.

Machine: Apple M5 Max (18 cores), 48 GiB, macOS 26.5.2 arm64, rustc 1.95.0,
native worker (`magistr --workspace-worker`), release profile. Recorded
11 September 2026 on the working tree of the commit that landed
`arch-14c10b`; the JSON's `magistrCommit` names that commit's parent
(`7458ee0`) because the runs preceded the commit. System load was 15–22 during
the runs; the numbers are upper bounds on an idle machine.

## Index (`--profile index`)

| profile | corpus | tier-0 | tier-1 | tier-1 per 500k LOC | gates |
| --- | --- | --- | --- | --- | --- |
| synthetic-500k | 504,019 LOC / 2,013 source files (13 language markers + bulk TypeScript) | 0.051 s | 2.87 s (2,013/2,015 files) | 2.84 s | tier0 pass · tier1 pass |

Tier-0 is measured from `WorkspaceService::open` (worker spawn, self-test,
build kick) to the first lexical hit. All 13 bundled language ids are present
in the map after the build and 12 carry outlines (JSON has none by
construction); the plain-text needle matches in the docs and the lockfile.

## References (`--profile refs`)

| profile | corpus | calls | p50 | p95 | max | hot query | gate |
| --- | --- | --- | --- | --- | --- | --- | --- |
| synthetic-500k | 506,003 LOC / 2,001 files | 120 | 16.99 ms | 29.06 ms | 32.12 ms | shared_ref_target, 1,001 edges (capped; p95 30.9 ms) | pass |

Per query: hot p50 24.4 ms, rare (`bulk_5_2`) p50 15.9 ms, miss p50 14.7 ms.
Queries go through `WorkspaceService::search` in references mode with a
1,000,000-token budget so the whole capped edge set comes back, as the
reference's raw `index/references` frame did.

## Worker round-trip (`--profile worker`, 2,000 pings)

| p50 | p95 | p99 | max | gate |
| --- | --- | --- | --- | --- |
| 0.096 ms | 0.192 ms | 0.299 ms | 0.663 ms | pass |

The native `Ping` runs the parser self-test inside the worker on every call,
so this bounds the frame round-trip from above; the reference's stub ping
touched no core.

## What the first run found

The first references run refused the hot query (`BUDGET_TOO_SMALL` at 82,000
occurrences) and cost 570 ms for a miss: the native lane read, hashed and
line-scanned every file in scope per query instead of taking candidates from
the index as the retained indexer did. Three root causes were fixed in this
iteration and are recorded in `PROVENANCE.md`: index-driven candidates within
the retained bounds (200 definitions, 200 files, 50 occurrences per file,
1,000 in total; capped with a note, never refused), one ignore-rule matcher
per index answer instead of one per row, and an early-terminating candidate
probe that is verified when scanned rather than when named. The live walk
that still scopes every search (about 10 ms at 2,000 files) and the
unindexed symbol-name lookup (about 3.5 ms at 82,000 symbols) are the
remaining floor, filed as `workspace-references-scope-floor`.

## Re-recording

```
./scripts/workspace-bench.sh                 # 500k profiles + worker
./scripts/workspace-bench.sh --loc 1000000   # the 1M profiles
cargo bench -p magistr-cli --bench workspace_bench -- --profile refs --loc 500000 --json out.json
```
