# Implementation Plan: rustfmt Formatter

**Branch**: `012-rust-rustfmt-formatter` | **Date**: 2026-08-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/012-rust-rustfmt-formatter/spec.md`

## Summary

Add `RustfmtFormatter` (one spawn per `.rs` write: stdin → stdout, `--edition` from the crate's
or workspace's manifest, cwd = the file's directory for `rustfmt.toml` discovery, 10 s timeout,
failure = throw with a useful message) resolved through `rustup which rustfmt` with cwd = root
(honoring `rust-toolchain.toml`) then a runnable PATH `rustfmt`; compose it behind a routing
`FormatterPort` (`.rs` → rustfmt, else the untouched Biome adapter) in `composeWrite`; widen the
write engine's degradation note to carry the formatter's message; record cold/warm per-edit
overhead on the real binary. Substrate pinned in think:168/169 (R1–R9).

## Technical Context

**Language/Version**: TypeScript (bun) only.

**Primary Dependencies**: `src/ports/formatter.ts`; `src/adapters/biome/formatter.ts` (pattern
and fallback); `src/tools/write-engine.ts` (note wording); `src/main.ts` `composeWrite`;
`src/protocol/resolver-plugin.ts` `resolveRustAnalyzer` (the resolution pattern, not imported —
adapters may not import protocol).

**Storage**: none.

**Testing**: `bun test` — `test/rustfmt-formatter.test.ts` (injected-binary degradation pins,
router pins, toolchain-gated real-rustfmt formatting/edition/config/overhead pins),
`test/write-engine.test.ts` (note wording), existing `test/biome-formatter.test.ts` unchanged.

**Target Platform**: unchanged; spawn without a shell on win32/darwin/linux.

**Performance Goals**: per-edit overhead recorded; no spawn for non-`.rs` files (router) and no
spawn when the binary is unresolvable after the first probe (memoized).

**Constraints**: no new dependency; no TOML parser; the pipeline's order and contract
unchanged; Biome behavior byte-for-byte.

**Scale/Scope**: one adapter module, one router, one composition line, one note wording, one
test file.

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS.*

- **I (grep cost structure)**: no daemon; one spawn per Rust write; unresolvable ⇒ no further
  spawns; never blocks a write.
- **II (rank, don't dump)**: the degradation note names the cause (R6).
- **III (verified writes)**: format → re-verify unchanged; a parse-breaking format falls back.
- **IV (typed escape hatches)**: rustfmt is spawned as a fixed argv, no shell; the binary is
  the toolchain's, not user-supplied config.
- **V (versioned contracts)**: `FormatterPort` implemented, not changed.
- **VI (no number, no merge)**: cold/warm overhead recorded.

## Project Structure

### Documentation (this feature)

```text
specs/012-rust-rustfmt-formatter/
├── plan.md
├── research.md           # R1–R9
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
src/adapters/rustfmt/formatter.ts     # NEW: RustfmtFormatter, resolveRustfmt, editionFor
src/adapters/rustfmt/index.ts         # NEW: exports + routingFormatter
src/tools/write-engine.ts             # note wording: "(<message>)"
src/main.ts                           # composeWrite: routingFormatter(rust → rustfmt, else Biome)
test/rustfmt-formatter.test.ts        # NEW
test/write-engine.test.ts             # note wording pin
```

**Structure Decision**: a sibling adapter to Biome with the same seams (`binary`, timeout), a
router beside it, and the composition in `main.ts` where Biome is composed today (R1/R9).

## Complexity Tracking

No constitution violations; table intentionally empty.

## Deviations (recorded at build, 2026-08-23)

1. **No PATH fallback under an unsatisfiable toolchain pin (FR-002 refined).**
   The spec said "toolchain manager first, falling back to a runnable
   `rustfmt` on PATH". With `rust-toolchain.toml` pinning a channel that is
   not installed, `rustup which` fails — and the PATH `rustfmt` is either the
   rustup proxy (which fails for that cwd) or a different toolchain's binary,
   which is exactly what honoring the pin forbids. `resolveRustfmt` therefore
   falls back to PATH only when the workspace has NO pin; a pinned workspace
   whose pin cannot be satisfied resolves to nothing and the write degrades
   with the install note. Pinned by test (an uninstalled pin ⇒ null).

2. **A root file under a virtual-workspace manifest takes the workspace
   edition.** `editionFor` returns `[workspace.package] edition` for a file
   whose nearest manifest has no `[package]` at all; a file the workspace
   cannot compile still formats with the workspace's edition rather than a
   guessed default.

3. **The fake-binary seam needs `--`.** `bun -e <script> --edition 2024`
   lets bun consume `--edition` as its own flag; the test seam passes `--`
   before the adapter appends `--edition`. A test-only fact, recorded because
   it cost a run.

4. **Nothing else deviated.** The router, the cwd policy, the timeout, the
   note wording and the memoized resolution shipped as shaped.

### SC-004 (Constitution VI, recorded on the build machine — Windows 11, rustfmt 1.9.0 via rustup, toolchain 1.95.0)

| number | value |
|---|---|
| cold per-edit overhead (resolution + first spawn, 20-fn file) | 58–61 ms |
| warm per-edit overhead (spawn only) | 26–28 ms |

The edition is load-bearing: a `let`-chain file formats under a 2024 manifest
and is rejected by rustfmt under a 2021 manifest (pinned); `rustfmt.toml`
`max_width = 40` discovered from the file's directory wraps the output (pinned).
