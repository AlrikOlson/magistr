# Phase 0 Research: Rust Bench at 1M LOC

Grounded in `scripts/bench-index.ts` (synthetic corpus in a tmpdir; `EngineSupervisor`;
`index/build {root}`; tier-0 = spawn + self-test + build-kick + one lexical `search` frame over
`bulk/mod_0.ts`; polls `index/status` until `tier === 1 && !building` with a
`TIER1_GATE_S + 30` deadline; reads `<root>/.workbench/index.db` directly for language
coverage and the FTS needle; gates tier-0 5 s / tier-1 60 s; `rmSync(root)` in `finally`),
`scripts/bench-refs.ts` (same generator plus one shared symbol every module calls; queries
hot / rare / miss × 40 through `index/references`; p50 / p95 / max; gate p95 50 ms; the hot
query must answer edges), `scripts/bench-ipc.ts` (stub core, ping p95 < 1 ms, corpus-free),
`src/adapters/engine-ipc/indexer.ts` (`files` = path, language, size, mtime_ms, junk,
content_indexed, no line count; `symbols` = path, name, kind, start_line, … with a NOCASE
name index; `JUNK_DIRS` incl. `target`/`vendor`/`third_party` flagged never excluded;
`.git`/`.workbench` excluded; gitignored paths not walked; `queryReferences` = up to 200
definitions by name, import-linked files by stem, FTS5 candidates capped at
`MAX_REFERENCE_FILES` 200, per-file line scan), `crates/workbench-core/src/indexer.rs`
(`scan_workspace` = `ignore::WalkBuilder` hidden(false) follow_links(false)
require_git(false)), `specs/005-tracing-ring-buffer/plan.md` item 7 (the only record of the
synthetic baseline, in prose), `eval/ranking-baseline.json` (the precedent for a committed
recorded number), `.github/workflows` (bench:index + bench:refs on the built-artifact lane,
bench:ipc on the stub lane), the constitution's principle VI, and one shallow clone measured
on this machine (R5). Pinned in think:184/185.

## R1 — `--root` is the same run minus generation and deletion

**Decision**: `--root <path>` (absolute or cwd-relative; must be an existing directory, else
exit 2) replaces `makeCorpus` in both benches. Everything after corpus creation is unchanged:
the same supervisor, the same `index/build { root }`, the same tier-0 frame, the same
`index/status` poll, the same direct read of `<root>/.workbench/index.db`. The `finally` block
removes `<root>/.workbench`, the directory the run created, and never the root. `--keep-index`
keeps it (for inspecting the index). If `<root>/.workbench` exists before the run the bench
refuses (exit 2) unless `--keep-index` is given, because a warm index is not a cold build and
the bench must not delete state it did not create.

**Rationale**: one code path measured two ways; the acceptance is "the same report shape".

**Rejected**: a separate `bench-root.ts` (two harnesses drift); indexing a copy of the tree
(doubles the disk, and the walk is part of what is measured).

## R2 — LOC is counted by the bench from the index's files table

**Decision**: after tier 1 settles (outside the timed window) the bench reads
`SELECT path, language, junk FROM files`, counts newlines of every row with `language IS NOT
NULL AND junk = 0` (streamed reads), and reports `loc` (source, non-junk), `files`,
`sourceFiles`, `junkFiles` and `languages: [{ language, files, loc }]` sorted by LOC. Synthetic
profiles report the same counted number (the generator's `writtenLoc` stays on the console as
a cross-check).

**Rationale**: the index stores size and mtime, not lines; counting from the files table means
LOC covers exactly what the index covered (gitignored paths excluded, junk flagged), which is
the honest denominator for a "500k LOC to tier 1" rate.

**Rejected**: `tokei`/`cloc` (an external tool that counts files the walk never saw);
counting during the build (would sit inside the measurement).

## R3 — The tier-0 probe is a flag with a deterministic default

**Decision**: `--probe <relpath>:<pattern>` (split at the LAST colon; relpath is
forward-slashed, root-relative). Default under `--root`: the first existing of `Cargo.toml`,
`README.md`, `package.json`; the pattern is the first `[A-Za-z_][A-Za-z0-9_]{2,}` token of the
file's first non-empty line. No candidate → exit 2 asking for `--probe`. The probe file's
content is read before the clock starts, as today.

**Rationale**: tier-0 needs a real lexical hit before any index exists; the default must be
reproducible run to run.

## R4 — References are sampled from the built index

**Decision**: after tier 1, `bench-refs.ts --root` opens `index.db` read-only and picks: hot =
the three most frequent `symbols.name` values with `length(name) >= 3` (`GROUP BY name ORDER BY
count(*) DESC, name`); rare = names with exactly one row, ordered by name, at indices
`floor(n/4)`, `floor(n/2)`, `floor(3n/4)`; miss = `bench_refs_no_such_symbol`. Seven queries ×
`ITERATIONS_PER_QUERY` (40) after one warm-up call each; the report adds per-query p50 / p95 and
edge counts to today's overall p50 / p95 / max. Vacuity rule: at least one hot query must answer
edges. Synthetic mode keeps its three fixed queries.

**Rationale**: the hot queries stress the 200-definition cap, the stem-linked imports and the
200-file FTS candidate cap where a real monorepo is hardest (`new`, `fmt`, `from` …); the rare
ones measure the common case; the miss measures the floor. Deterministic sampling makes the
committed number reproducible on the same checkout.

## R5 — The corpus is rust-lang/rust, shallow-cloned at `D:/Code/_ext/rust`

**Decision**: `git -c core.longpaths=true clone --depth 1 --no-tags --single-branch --branch
main https://github.com/rust-lang/rust.git rust` into `D:/Code/_ext` (no submodules:
`src/llvm-project`, `src/gcc` and the remaining submodules stay empty). Measured on the
checkout the build unit will record (commit `da5114692c9ebe46b869488c5f34f92eb10b98c1`,
cloned 2026-08-24): 38,425 `.rs` files, 3,997,539 lines of Rust (`compiler/` 868,161,
`library/` 1,074,265, `src/` 1,248,574 — cargo, clippy, rust-analyzer, miri and the other
in-tree tools — `tests/` 806,539), 62,221 tracked files, 321 MB working tree. Every top-level
area alone is near or above the constitution's 1M figure; the whole is 4× it.

**Rationale**: a real monorepo (compiler, standard library, tools, tests), canonical, and it
needs no build to be indexed: the benches index, they never compile. The default branch is
`main` (a `master` clone fails: "Remote branch master not found").

**Rejected**: polkadot-sdk / zed / servo (heavier clones, Windows build dependencies, none more
canonical); any local repository (the largest on this machine, idlecraft, has 695 `.rs` files);
rust-analyzer / tokio / wasmtime as standalone repos (under 1M LOC of Rust). The dogfood gate's
"real Rust workspace" is a separate choice and may be a buildable, smaller repository.

**Known hazard for the build**: `tests/` holds deliberately malformed sources (invalid UTF-8,
unbalanced tokens, enormous literals). The indexer's behaviour over them is part of what the
run measures; a crash or a hang is a finding to record, not a reason to exclude the directory.

## R6 — Gates are judged as the constitution states them

**Decision**: gate rows `{ name, value, limit, unit, pass, note }`:
`tier0` value s, limit 5, as written; `tier1` value = seconds normalized per 500k LOC
(`tier1S * 500000 / loc`), limit 60, note "normalized per 500k LOC; measured Xs over N LOC" —
for the synthetic 500k profile the normalization is a no-op; `references_p95` ms, limit 50, as
written (the constitution says "at 1M LOC"; the recorded corpus is above it, the note says the
LOC); `ipc_p95` ms, limit 1, as written. A failed gate is `pass:false` in the JSON and exit 1
on the console unless `--report-only`; the JSON is written either way.

**Rationale**: the tier-1 gate is a throughput claim about 500k LOC; the only honest way to apply
it to 4M LOC is the rate, stated as the rate. Constitution VI: the number is recorded whether
or not it passes.

## R7 — Deadlines are flags, and a deadline reached is data

**Decision**: `--deadline <s>` bounds the tier-1 poll. Defaults unchanged for synthetic runs
(`TIER1_GATE_S + 30` in bench-index, 120 in bench-refs); `--root` defaults to 900. On expiry the
report records `tier1Reached:false`, `filesIndexed / filesTotal`, the seconds elapsed, and the
tier-1 gate `pass:false`; bench-refs then reports its measurement as not taken (exit 1).

## R8 — One report schema, written by `--json <path>`

**Decision**: `{ schema: "workbench-bench/1", script, profile, recordedAt, corpus, machine,
measurements, gates }`. `corpus` = `{ kind: "synthetic" | "root", root?, loc, files,
sourceFiles, junkFiles, languages: [{ language, files, loc }], repo?: { url, commit } }` (git
facts via `git -C <root> rev-parse HEAD` / `remote get-url origin`, absent when not a
checkout); `machine` = `{ platform, arch, cpu, cores, memoryGb, bun, coreFlavor: "native" |
"wasm" | "stub", workbenchCommit }`; `measurements` per script (bench-index: `tier0S`,
`tier1S`, `tier1Reached`, `filesIndexed`, `filesTotal`, `languagesIndexed`; bench-refs:
`calls`, `p50Ms`, `p95Ms`, `maxMs`, `queries: [{ role, name, definitions, p50Ms, p95Ms,
edges }]`; bench-ipc: `pings`, `p50Ms`, `p95Ms`, `p99Ms`, `maxMs`). The console output stays
what it is.

**Rationale**: `eval/ranking-baseline.json` is the precedent: a committed number is a file,
not a sentence in a plan.

## R9 — Where the recorded runs live

**Decision**: `bench/results/` holds `index-synthetic-500k.json`, `refs-synthetic-500k.json`,
`index-synthetic-1m.json`, `refs-synthetic-1m.json`, `ipc.json`, `index-rust-lang-rust.json`,
`refs-rust-lang-rust.json`, all recorded in one session on one machine, plus `bench/README.md`
with the table (profile, corpus LOC / files, tier-0, tier-1 (and per 500k), refs p50 / p95, IPC
p95, gates, machine, commits) and the commands that produced them. `specs/005` plan.md item 7
is left as history; the README is the current record. `biome` already excludes
`test/support/golden/**`; `bench/results/**` gets the same exclusion so the recorded JSON is
never reformatted.

## R10 — The shared helpers are a module with tests; the mode is smoke-tested with the stub

**Decision**: `scripts/bench-common.ts` holds argument parsing, machine facts, git facts, the
probe default, LOC counting over a `files` table, hot / rare / miss sampling over a `symbols`
table, gate construction with the tier-1 normalization, percentile helpers and the JSON writer;
`test/bench-common.test.ts` pins them over in-memory SQLite and temp dirs (no core needed).
`test/bench-root.test.ts` runs `bench-index.ts --stub --root <fixture> --report-only --json`
and `bench-refs.ts --stub --root <fixture> --report-only --json` as child processes over a
small fixture with a `.gitignore`d file and a junk dir, asserting the JSON shape, the fixture's
file list and hashes unchanged, `.workbench` removed, and the refusal when `.workbench`
pre-exists. The real-repository run is not a test: it is a recorded artifact (Constitution VI),
like [013 SC-006].

**Rationale**: `bench-index.ts` already imports from `src/`; the `check-import-graph` rule
constrains `src/` layering, not `scripts/`. The stub core answers lexical hits and builds a
(symbol-less) index, so the smoke test proves the plumbing without minutes of runtime; the
references smoke asserts the vacuous-measurement path rather than a p95.
