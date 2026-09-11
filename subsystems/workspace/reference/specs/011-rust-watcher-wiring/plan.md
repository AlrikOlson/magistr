# Implementation Plan: Watcher Wiring

**Branch**: `011-rust-watcher-wiring` | **Date**: 2026-08-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/011-rust-watcher-wiring/spec.md`

## Summary

Wire the shipped fswatch adapters into the serving path. `composeSearch` resolves
`watcher.mode` (new config enum, default `auto` = native unless the workspace is a detected
network filesystem ⇒ polling), binds the selected adapter to the `IpcIndexAdapter` through the
existing bridge — now bounded (≤ 8 refreshes in flight, per-path coalescing, removal wins,
temp-file filter, counters) so a `git checkout` burst never reaches the engine's pending
ceiling — publishes a cache-class `.workbench/watcher.json` status record the out-of-process
doctor reads (live / last-session / not-active / unreadable), and closes the subscription
before the engine child on stop. The numbers the chunk asks for are measured and recorded:
external-write-to-visible lag on a NEW file through symbol-mode search (native, polling), a
2000-file burst settle time with zero busy refusals, and idle CPU for none/native/polling. The
fuzzer gains a spawned-process mutator and a checkout-style burst. Substrate pinned in
think:164/165 (R1–R10).

## Technical Context

**Language/Version**: TypeScript (bun) only. No Rust-core change.

**Primary Dependencies**: `src/adapters/fswatch/watcher.ts` (bridge options), `src/main.ts`
(`composeSearch` + stop), new `src/protocol/watcher.ts` (mode selection, status publishing),
`src/config/schema.ts` (`watcher.mode`), `src/doctor/probes.ts` (environment watcher probe),
`test/freshness-fuzz.test.ts`, `test/fswatch.test.ts`, `test/doctor.test.ts`,
new `test/watcher-wiring.test.ts`.

**Storage**: one new cache-class JSON record in the state dir; no index/journal change.

**Testing**: `bun test` — bridge bound pins (recording index), mode selection pins, status
record + doctor probe pins, lag/burst/idle measurements over the real engine path with the stub
core (the fuzzer's existing rig), fuzzer variants; full gate sweep.

**Target Platform**: unchanged (fs.watch recursive on win32/darwin/linux; polling fallback).

**Performance Goals**: idle cost of the native watcher indistinguishable from none
(Constitution I); burst of 2000 files settles without refusals; lag recorded.

**Constraints**: `watcher.mode=off` ⇒ today's behavior byte-for-byte; nothing waits on the
watcher; every existing freshness guarantee unchanged; no new dependency.

**Scale/Scope**: one config key, one bridge option shape, one protocol module, one doctor
probe, one composition change, ~4 test files.

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS.*

- **I (grep cost structure)**: native watcher idle cost measured and recorded; polling only
  where native lies; the watcher never gates an answer (verification still does); failures
  degrade to query-time re-parse, never to an error.
- **II (rank, don't dump)**: untouched.
- **III (verified writes)**: untouched; the write engine's own temp files are filtered so tool
  writes do not double-refresh.
- **IV (typed escape hatches)**: untouched.
- **V (versioned contracts)**: the status record is cache-class with a `v` field; config gains
  one nearest-wins key; no schema/envelope change.
- **VI (no number, no merge)**: lag, burst settle, idle CPU recorded via `ship_record`.
- **Honesty**: the fuzzer's existing mutator and the shipped adapters are not claimed as
  work; the SDD placement deviation is recorded (R8).

## Project Structure

### Documentation (this feature)

```text
specs/011-rust-watcher-wiring/
├── plan.md
├── research.md           # R1–R10
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
src/config/schema.ts                  # "watcher.mode" enum key
src/adapters/fswatch/watcher.ts       # bindWatcherToIndex options: maxInFlight, ignore,
                                      #   onEvent/onSettled, idle(); temp-file filter
src/protocol/watcher.ts               # NEW: selectWatcherMode(), startWatcher() → {close},
                                      #   WatcherStatusRecord writer (throttled, stop)
src/main.ts                           # composeSearch wires startWatcher; stop closes it first
src/doctor/probes.ts                  # environment.watcher: live/last-session/not-active/unreadable
test/fswatch.test.ts                  # bound + coalescing + removal-wins + temp filter pins
test/watcher-wiring.test.ts           # NEW: mode selection, status record, lag, burst, idle
test/doctor.test.ts                   # watcher probe states
test/freshness-fuzz.test.ts           # spawned-process mutator + checkout burst variants
```

**Structure Decision**: the bridge stays in the adapter (it is the adapter's bridge); mode
selection and status publishing are composition concerns and live in the protocol layer, which
may import adapters (import-graph lint) and already writes `plugin-health.json` (R10).

## Complexity Tracking

No constitution violations; table intentionally empty.

## Deviations (recorded at build, 2026-08-23)

1. **Windows `fs.watch` overflows on a 200-file burst and says so with a NULL
   filename — which the shipped adapter ignored.** A probe over Bun's
   recursive `fs.watch` showed a synchronous burst of 200 (and 2000) writes
   delivers ONE real event and then one event whose filename is `null`:
   libuv's `ERROR_NOTIFY_ENUM_DIR`, "something under the root changed,
   rescan". `FsWatchAdapter` returned on `null`, so a `git checkout` would
   have reached the index as a single refresh. The adapter now primes a
   snapshot at subscribe and answers `null` with a snapshot diff; the differ
   (`SnapshotDiffer`) is shared with `PollingWatcherAdapter`, which was
   refactored onto it with no behavior change. Linux/macOS have no such
   signal (inotify's `IN_Q_OVERFLOW` is not surfaced by Node's `fs.watch`);
   there, the Tier-1 rebuild on the next server start and query-time
   verification remain the backstop — recorded, not solved here.

2. **Placement: `src/adapters/fswatch/wiring.ts`, not `src/protocol/watcher.ts`
   (R10 corrected by the import-graph lint).** `protocol` may not import
   `adapters`; `doctor` may. Selection + status publishing live beside the
   adapters; doctor reads the record through the same module.

3. **`read symbol:` is NOT the watcher's observable.** With the watcher off,
   `read {file, symbol}` on a brand-new file still succeeds — read parses an
   unknown file on demand (self-healing). The lag is therefore measured as
   "the index knows the file" (`querySymbols` by name) through the real
   engine-child path, with `read` and search's kind tag asserted afterwards;
   the off control pins that read works while the index and search tags stay
   blind until a rebuild. R5's "through symbol-mode search" became "search's
   kind tag + the index", which is what the tools actually read.

4. **The burst pin counts refreshes, not events ≥ N.** Because an overflow
   arrives as rescans, the honest settle predicate is "2000 refreshes landed";
   events were 2001 on the build machine (one real + the rescan diff).

5. **Small:** `bun -e` does not place trailing arguments in `argv` portably;
   the spawned mutator takes its parameters from the environment.

### SC-001 / SC-002 / SC-003 (Constitution VI, recorded on the build machine — Windows 11, bun 1.4.0)

| number | value |
|---|---|
| SC-001 lag, fs.watch (external NEW file → symbol in the index) | 103 ms (28 ms in an earlier run) |
| SC-001 lag, polling (200 ms cadence) | 183 ms (27 ms in an earlier run) |
| SC-002 burst, 2000 files rewritten at once, fs.watch | events 2001, refreshes 2001, failures 0, `ENGINE_BUSY` 0, settle 1663 ms |
| SC-003 idle CPU over 1.5 s, 2000-file tree | none 0 ms, fs.watch 0 ms, polling 374 ms |

The polling number is why `auto` never picks it on a local disk. Both
adapters pass the fuzzer with the spawned-process mutator and the
checkout-style burst. The server-verified gate sweep (`bun test`, 834/0)
reproduced the same picture: fs.watch lag 102 ms, polling 183 ms, burst
events 2001 / refreshes 2001 / failures 0 / `ENGINE_BUSY` 0 / settle 1714 ms,
idle none 0 ms / fs.watch 0 ms / polling 344 ms.
