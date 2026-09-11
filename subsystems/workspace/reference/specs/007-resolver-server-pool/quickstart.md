# Quickstart Validation: Resolver Language-Server Session Pool

Runnable scenarios proving the feature end to end. Prerequisites: `bun install`, Rust
toolchain per `rust-toolchain.toml`, the native binding built
(`crates/workbench-core/artifacts/workbench-core.<key>.node`), the guest rebuilt with
`plugins/lsp-resolver/build.ps1` (or `build.sh`). All commands from the repo root.

## Scenario 1 — Slow server contributes on the second request (US1 / SC-001)

```
bun test test/resolver-plugin.test.ts -t "pool"
```

Expected: with the fake server in `slow-init` mode (handshake slower than the budget) the
first `PluginResolver.resolve` answers `[]` and records one budget breach; after the breach
expiry (injected clock) the second call answers `resolver-lsp`/`high` edges; the fake's
initialize counter reads 1; the outcome's `sessions.reused` is 1 on the second call.

## Scenario 2 — Teardown on every ending (US2 / SC-002)

```
cargo test --manifest-path crates/workbench-core/Cargo.toml server_pool
bun test test/server-pool.test.ts
```

Expected (Rust): fuel-exhausted and trapping guests leave no server process; `close` kills;
a guest that ends normally leaves a warm session; idle TTL (injected) kills; `shutdown` kills
all. Expected (bun): an engine child with a warm `ignore-eof` fake session, stopped by the
supervisor, leaves no fake process; on Windows and Linux the same holds after a hard kill of
the child; on darwin that test skips with a message naming the gap.

## Scenario 3 — Stateless and compatible (US3 / SC-003 / SC-004)

```
bun test test/statelessness.test.ts
bun test test/sandbox-escape.test.ts -t "subprocess"
bun test test/plugin-tck.test.ts
cargo test --manifest-path crates/workbench-core/Cargo.toml --test wit_contracts
```

Expected: all pass unmodified (the escape suite's grant strings updated to the `@0.2.0` name
where they name the manifest's import; its sideload refusals unchanged); `wit_contracts`
passes WITHOUT `WIT_BASELINE_BLESS`. A Rust unit test over a `@0.1.0` importer shows
per-invoke semantics; an `effectiveCapabilities` test shows a `@0.1.0` grant authorizing the
`@0.2.0` manifest import for a blessed record and still refusing a sideload.

## Scenario 4 — The recorded number (US4 / SC-005)

```
bun test test/resolver-plugin.test.ts -t "real server"
```

Expected with `rust-analyzer` and `cargo` on PATH: over a fixture crate in a temp dir the
scenario prints `cold=<ms> warm=<ms> handshakes=1` and the test asserts warm < cold and
`sessions.reused == 1`. Without the binaries: skipped with a message naming them. The
printed numbers are recorded in the ship trace by the build unit (`ship_record`).

## Full gate sweep (required before completion)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check && bun run lint:skill
cargo test --manifest-path crates/workbench-core/Cargo.toml
```

Expected: all exit 0, recorded server-side via `ship_check`.
