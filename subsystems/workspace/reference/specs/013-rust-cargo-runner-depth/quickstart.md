# Quickstart Validation: cargo Runner Depth

Prerequisites: `bun install`; for the live scenario, `cargo` on PATH (and `cargo nextest` for
the nextest half). Commands from the repo root.

## Scenario 1 — Test failures from both runners (US1 / SC-001)

```
bun test test/run-cargo.test.ts -t "cargo test"
bun test test/run-cargo.test.ts -t "nextest"
```

Expected: the single-binary, workspace (`--no-fail-fast`) and backtrace goldens produce the
expected failure rows (plain panic message captured, forward-slash file, location as the first
trace line, aggregated summary with rerun hints, trimmed frames); the nextest golden yields one
row per `FAIL` across two binaries, no duplicates from the repeat list, the `Summary` line with
the cancellation note.

## Scenario 2 — Diagnostics rows (US2 / SC-002, SC-003)

```
bun test test/run-cargo.test.ts -t "diagnostics"
```

Expected: JSON and human `cargo build` goldens yield equal diagnostics (E0308 with
file:line:col and the help text; the unused-variable warning once, not twice); clippy JSON and
human are equal; the green build is an empty diagnostics row; a compile-failed `cargo test`
answers diagnostics; an unrecognizable failure answers head+tail; an oversized diagnostics row
is cut errors-first with a truncation manifest.

## Scenario 3 — Seeding a workspace (US3 / SC-004)

```
bun test test/run-tool.test.ts -t "detect"
bun test test/run-tool.test.ts -t "init"
```

Expected: the fixture workspace seeds `cargo test --workspace --no-fail-fast`, `cargo build
--workspace` and one `cargo test -p <member>` per (non-excluded) member; a single crate seeds
`cargo test`/`cargo build` as before; `cargo-nextest`/`cargo-clippy` appear only with
`.config/nextest.toml` / a `clippy` component; `init` never clobbers.

## Scenario 4 — Timeout unchanged (US4 / SC-005)

```
bun test test/run-tool.test.ts -t "timeout"
bun test test/runner-plugin.test.ts
```

Expected: the timeout pin, the claim pin and the runner-plugin golden suite pass unchanged.

## Scenario 5 — Live toolchain (SC-006)

```
bun test test/run-cargo.test.ts -t "live"
```

Expected: over a temp two-crate fixture the real `cargo test --workspace --no-fail-fast`,
`cargo nextest run --workspace` (skipped with a reason when absent) and `cargo build
--message-format=json` produce rows of the goldens' shape; `[013 SC-006] …` is printed and
recorded.

## Scenario 6 — Gates (SC-007)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check && bun run lint:skill
cargo test --manifest-path crates/workbench-core/Cargo.toml
git diff --exit-code wit/baseline.json
```
