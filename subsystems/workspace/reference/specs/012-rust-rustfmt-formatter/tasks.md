# Tasks: rustfmt Formatter

**Input**: Design documents from `specs/012-rust-rustfmt-formatter/` (plan.md, research.md
R1–R9, data-model.md, quickstart.md). Substrate pinned in think:168/169.

**Tests**: included — every success criterion is pinned by a test or a recorded number
(Constitution VI); real-rustfmt cases are toolchain-gated.

**Organization**: phases map to the spec's user stories; each story is independently testable.
Execute in order; do not re-derive decisions recorded in research.md.

## Phase 1: Foundational — the adapter (blocking)

- [x] T001 `src/adapters/rustfmt/formatter.ts`: `resolveRustfmt(root)` (rustup which with
      cwd = root, then PATH confirmed by `--version`; injectable), `editionFor(root, path,
      read)` (nearest Cargo.toml `[package] edition`, else root `[workspace.package]`, default
      2021; textual), `RustfmtFormatter` implementing `FormatterPort` with `{root, binary?,
      resolver?, timeoutMs?}`: `.rs` only (passthrough otherwise), memoized resolution,
      spawn `<bin> --edition <e>` with cwd = dirname(file), stdin/stdout, failure = throw with
      the messages in data-model.md, 10 s kill (R2–R5).
- [x] T002 `src/adapters/rustfmt/index.ts`: exports + `routingFormatter({rules, fallback})`
      (R1).
- [x] T003 Unit tests (no toolchain) in `test/rustfmt-formatter.test.ts`: editionFor over
      in-memory manifests (package, workspace inheritance, missing), router picks by extension
      and falls back, resolver null ⇒ throw with the install remedy, fake binary exit 1 ⇒ throw
      with the stderr line, fake binary sleeping past timeoutMs ⇒ killed + throw, binary seam
      used directly.

## Phase 2: User Story 1 + 2 — through the pipeline (P1) 🎯 MVP

- [x] T004 [US1/US2] `src/tools/write-engine.ts`: the degradation note carries the formatter's
      message (R6); `test/write-engine.test.ts` pins the wording; existing "the formatter
      failed" matches still hold (SC-005).
- [x] T005 [US1/US2] `src/main.ts` `composeWrite`: `routingFormatter({ rules: [{ extensions:
      ["rs"], formatter: new RustfmtFormatter({root}) }], fallback: new BiomeFormatter({root})
      })`.
- [x] T006 [US2] Pipeline pins over the in-process ports + the router with injected fakes:
      missing / exit-1 / hung rustfmt ⇒ write applies the verified unformatted content, envelope
      ok, note names the cause (SC-002).
- [x] T007 [US1] Toolchain-gated pipeline pins: a `.rs` write applies rustfmt's output; a `.ts`
      file in the same batch is Biome-formatted; `.md` passes through; `format:false` restores
      exact bytes (SC-001).

## Phase 3: User Story 3 — toolchain, edition, config (P2)

- [x] T008 [US3] Toolchain-gated: a fixture `rust-toolchain.toml` pinning an UNINSTALLED
      channel ⇒ resolution null ⇒ degraded note; the repo's pinned channel resolves (the
      resolver's output path contains the channel name); a 2024-edition fixture with
      edition-specific syntax formats when the manifest says 2024; a `rustfmt.toml` with
      `max_width = 40` visibly wraps (SC-003).

## Phase 4: User Story 4 — the number (P2)

- [x] T009 [US4] Toolchain-gated: format a ~20-line file twice; print and record cold/warm ms
      (`[012 SC-004]`) (SC-004).

## Phase 5: Polish

- [x] T010 Update `plan.md` Deviations, tick this file; confirm `bun run lint:skill` needs no
      skill change (the checker decides).
- [x] T011 Server-verified gate sweep via `ship_check` (bun test, tsc, lint quartet, cargo
      test), `ship_finalize`, `roadmap_complete_chunk`, commit.
