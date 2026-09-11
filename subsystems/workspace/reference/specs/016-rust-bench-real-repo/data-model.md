# Phase 1 Data Model: Rust Bench at 1M LOC

No port, schema-shape, index or source behaviour changes. One committed data format and the
bench-internal vocabulary.

## BenchReport (bench/results/*.json, schema "workbench-bench/1")

```
BenchReport
  schema        "workbench-bench/1"
  script        "bench-index" | "bench-refs" | "bench-ipc"
  profile       "synthetic-500k" | "synthetic-1m" | "root" | "ipc"
  recordedAt    ISO-8601
  corpus?       CorpusFacts        (absent for bench-ipc)
  machine       MachineFacts
  measurements  IndexMeasurements | RefsMeasurements | IpcMeasurements
  gates         Gate[]

CorpusFacts
  kind          "synthetic" | "root"
  root?         string             the --root path as given (root only)
  loc           number             newlines over files WHERE language IS NOT NULL AND junk = 0
  files         number             rows in files
  sourceFiles   number             rows with a language and junk = 0
  junkFiles     number             rows with junk = 1
  languages     { language, files, loc }[]   sorted by loc desc
  repo?         { url, commit }    git facts when the root is a checkout

MachineFacts
  platform, arch, cpu, cores, memoryGb, bun, coreFlavor ("native" | "wasm" | "stub"),
  workbenchCommit

IndexMeasurements
  tier0S, tier1S, tier1Reached, filesIndexed, filesTotal, languagesIndexed

RefsMeasurements
  calls, p50Ms, p95Ms, maxMs
  queries: ReferenceSample[]

ReferenceSample
  role          "hot" | "rare" | "miss"
  name          string
  definitions   number             symbols rows for the name (0 for miss)
  p50Ms, p95Ms  number
  edges         number             edges answered by the last call

IpcMeasurements
  pings, p50Ms, p95Ms, p99Ms, maxMs

Gate
  name          "tier0" | "tier1" | "references_p95" | "ipc_p95"
  value         number             the judged value (tier1: normalized s per 500k LOC)
  limit         number
  unit          "s" | "ms"
  pass          boolean
  note?         string             e.g. "normalized per 500k LOC; measured 71.3s over 3.9M LOC"
```

## Invariants

- `loc >= 0`; for `profile: "root"` the recorded run has `loc >= 1_000_000` (SC-003).
- `gates[].pass === (value <= limit)` always; `--report-only` changes the exit code, never
  `pass`.
- `tier1Reached === false` ⇒ the `tier1` gate is `pass:false` and `tier1S` is the deadline
  reached.
- A `--root` run never changes the file list or content hashes of the root; `.workbench/`
  under the root exists after the run iff `--keep-index`.
- Sampling is a pure function of the `symbols` table (same checkout ⇒ same queries).

## Bench-internal vocabulary (scripts/bench-common.ts)

```
BenchArgs      { root?, probe?, json?, keepIndex, deadlineS?, loc, stub, reportOnly }
Probe          { path, pattern }           path root-relative, forward-slashed
LocCount       CorpusFacts minus repo/root
Sample         ReferenceSample minus timings
```
