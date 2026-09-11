# Quickstart Validation: rustfmt Formatter

Prerequisites: `bun install`; for the real-formatting scenarios, a toolchain with the rustfmt
component (`rustup which rustfmt` succeeds from the repo root). Commands from the repo root.

## Scenario 1 — Rust edits come back formatted; others unchanged (US1 / SC-001, SC-005)

```
bun test test/rustfmt-formatter.test.ts -t "formats"
bun test test/biome-formatter.test.ts test/write-engine.test.ts
```

Expected: an unformatted `.rs` write applies rustfmt's output through the write engine; a `.ts`
file in the same batch is Biome-formatted; `.md` passes through; `format:false` restores exact
bytes; the existing formatter suites pass (note wording widened).

## Scenario 2 — Degradation, never an error (US2 / SC-002)

```
bun test test/rustfmt-formatter.test.ts -t "degrades"
```

Expected: a missing binary, a non-zero exit and a hung binary each apply the verified
unformatted content and the note names the cause; no error envelope.

## Scenario 3 — Toolchain, edition, config (US3 / SC-003)

```
bun test test/rustfmt-formatter.test.ts -t "toolchain"
```

Expected: the pinned channel's rustfmt is resolved; an uninstalled pin degrades with a note;
`edition` is read from `[package]` or `[workspace.package]`; a `rustfmt.toml` `max_width`
changes the output.

## Scenario 4 — The number (US4 / SC-004)

```
bun test test/rustfmt-formatter.test.ts -t "overhead"
```

Expected: `[012 SC-004] rustfmt per-edit overhead: cold <n>ms warm <m>ms` printed and recorded.

## Scenario 5 — Gates (SC-006)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check && bun run lint:skill
cargo test --manifest-path crates/workbench-core/Cargo.toml
```
