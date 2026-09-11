# Quickstart Validation: Watcher Wiring

Prerequisites: `bun install`; the native binding or the stub core (tests use
`WORKBENCH_ENGINE_STUB_CORE=1` where the fuzzer already does). Commands from the repo root.

## Scenario 1 — External write becomes visible (US1 / SC-001, SC-006)

```
bun test test/watcher-wiring.test.ts -t "lag"
bun test test/watcher-wiring.test.ts -t "mode"
```

Expected: a new file written by another process is found by symbol-mode search within the
recorded lag for the native adapter and for the polling fallback (numbers printed and recorded
in the ship trace); with `watcher.mode=off` the symbol does not appear until a rebuild; auto
resolves to native on a local disk and to poll under `WORKBENCH_NETWORK_FS=1`.

## Scenario 2 — Bursts bounded, idle free (US2 / SC-002, SC-003)

```
bun test test/fswatch.test.ts -t "bound"
bun test test/watcher-wiring.test.ts -t "burst"
bun test test/watcher-wiring.test.ts -t "idle"
```

Expected: at most K refreshes in flight, per-path coalescing, removal wins, all N complete;
2000 external files index with zero `ENGINE_BUSY` and a recorded settle time; idle CPU over
the same tree recorded for none / native / polling, native within the bar.

## Scenario 3 — doctor is live (US3 / SC-004)

```
bun test test/doctor.test.ts -t "watcher"
```

Expected: live (adapter, pid, counts) while the record's pid is alive; last session after stop;
not active with no record; unreadable named without crashing.

## Scenario 4 — Fuzzer with an external-process mutator (US4 / SC-005)

```
bun test test/freshness-fuzz.test.ts
```

Expected: both adapters pass with the in-process mutator, the spawned-process mutator, and the
checkout-style burst; no stale line number, no warm answer older than the disk.

## Scenario 5 — Gates (SC-007)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check && bun run lint:skill
cargo test --manifest-path crates/workbench-core/Cargo.toml
```
