# Implementation Plan: Rust Move-Aware Files

**Branch**: `010-rust-move-aware-files` | **Date**: 2026-08-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/010-rust-move-aware-files/spec.md`

## Summary

Give `files` `op:move` a Rust branch. For a `.rs` source, derive the module path from the
file's location (crate root → `<name>.rs` / `<name>/mod.rs` conventions), confirm it by finding
the declaring `mod <name>;` item in the parent module file, and produce — inside the same
confirm-gated write batch — the declaration rewrite (rename in place, or remove-from-old and
add-to-existing-new), every contiguous `crate::`/`super::`/`self::`/uniform path in the crate
that resolves to the moved module (re-emitted relative when still reachable, else absolute),
the moved file's own re-rooted `super::` paths, and any `Cargo.toml` target `path` entry that
named the file. What is deliberately not rewritten (brace-group `use` members, non-leaf modules,
undiscoverable declarations, `#[path]`, cross-crate paths, auto-discovered targets, a missing
destination parent) is stated in the notes with file and line. Workspace members are a
directory fact with no single-file trigger: verified untouched, not claimed. The generic note
drops "rust/". Substrate pinned in think:160/161 (R1–R10).

## Technical Context

**Language/Version**: TypeScript (bun) only. No Rust-core change.

**Primary Dependencies**: `src/tools/files.ts` (`resolveMove`, `importerExtensions`, note
assembly, `alwaysGated`); `src/tools/write-engine.ts` (TOML well-formedness, unchanged);
`src/tools/map.ts` (`cargoEntryPoints` — the section-split discipline reused, not the
function); `test/files-tool.test.ts` rig; `test/resolver-plugin.test.ts` gating pattern.

**Storage**: none changed.

**Testing**: `bun test` — new `test/rust-move.test.ts` (pure rules + rig-level moves +
toolchain-gated `cargo check` fixture), `test/files-tool.test.ts` (the pinned Rust negative
test inverted to the positive; Go note kept; six non-Rust fixtures untouched); full gate sweep.

**Target Platform**: unchanged.

**Performance Goals**: a Rust move scans the crate's `.rs` files once (the same order of work
the TS family's importer scan already does); no index dependency (Constitution I).

**Constraints**: non-Rust families byte-for-byte (FR-011); nothing synthesized (FR-010); no new
dependency (no TOML parser); every write through `runWriteBatch`; move stays confirm-gated.

**Scale/Scope**: one new pure module, one branch in `resolveMove`, description text in one
schema and one skill paragraph, one test file plus one inverted test.

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS.*

- **I (grep cost structure)**: works cold (no index); cost is one crate-scoped scan per move.
- **II (rank, don't dump)**: not a read tool; notes are specific, bounded, and name what was
  left for the agent.
- **III (verified writes)**: every rewrite is a `WriteOp` in the one batch → dry-run diff →
  confirm gate → parse-verify (`.rs`) / TOML well-formedness (`Cargo.toml`) → journal → swap.
  Move stays `alwaysGated`.
- **IV (typed escape hatches)**: untouched.
- **V (versioned contracts)**: no schema field change; description text only; no stored data.
- **VI (no number, no merge)**: not a ranking change; the number here is a compiler exit code,
  recorded by a gated test and in the ship trace.
- **Honesty**: the chunk's "workspace members" is recorded as not applicable to single-file
  moves (FR-012) rather than claimed.

## Project Structure

### Documentation (this feature)

```text
specs/010-rust-move-aware-files/
├── plan.md
├── research.md           # R1–R10
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
src/tools/rust-move.ts                    # NEW: pure module — module paths, declaration
                                          #   find/rewrite, path resolve/rewrite, manifest
                                          #   entry rewrite, planRustMove(...)
src/tools/files.ts                        # resolveMove: .rs branch merges plan writes/notes;
                                          #   generic note drops "rust/"
schemas/tools/files.input.schema.json     # op:move description names Rust (text only)
skills/workbench-routing/SKILL.md         # files move paragraph: Rust sentence (hand edit)
test/rust-move.test.ts                    # NEW: rules, rig moves, negatives, gated cargo check
test/files-tool.test.ts                   # Rust negative test → positive; Go pin kept
```

**Structure Decision**: the Rust rules need importer-module context and a declaration edit,
which is a different shape from the per-family text substitutions in `files.ts`; a pure module
keeps `files.ts` as the dispatcher and tests the rules without a toolchain (R9).

## Complexity Tracking

No constitution violations; table intentionally empty.

## Deviations (recorded at build, 2026-08-23)

1. **A per-file `use` binding table was added (R3 widened).** Research R3 let a
   uniform (unprefixed) path resolve only when its first segment is a module
   DECLARED in the same file. The most common Rust shape is the other one:
   `use crate::a::b;` followed by `b::helper()` — `b` is a `use`-bound local
   name, not a declared module. Under R3 as written, renaming `b` to `bee`
   would rewrite the `use` line and leave every `b::helper()` dangling, so the
   crate would not compile. `rewritePaths` now collects the file's `use`
   bindings (single items and brace-group members, aliases respected) and
   resolves uniform paths through them; the local name follows the rename
   (`bee::helper()`), an aliased binding keeps its alias, and a binding that
   came from a reported brace group is left alone. Still specifier-based —
   nothing outside the file is consulted. Pinned by test.

2. **`mod.rs` ascends past itself.** `locateModule` treated `<dir>/mod.rs` as a
   parent candidate for the file `<dir>/mod.rs` itself, so its parents came
   back as itself. Caught by the derivation test; the walk now skips the file
   under inspection and continues upward without a parent yet.

3. **At the root directory, the new declaration goes to the root(s) that own
   the old tree.** When the destination's parent directory is the root
   directory and both `src/lib.rs` and `src/main.rs` exist, the declaration
   is inserted into the root(s) whose content declares the moved module's
   top segment, falling back to every root. R5 said "the root file" and did
   not decide the two-root case.

4. **The compile proof asserts exit code, not empty stderr.** The first
   fixture had a private `mod a;`, so `cargo check` printed dead-code warnings
   (exit 0) and an `stderr === ""` assertion failed for the wrong reason. The
   fixture now declares `pub mod a;` and the test asserts exit 0 plus no
   `error` line — "compiles" is the claim, not "silent".

5. **Recorded limits, not changed:** paths inside string literals and comments
   are rewritten like code (intra-doc links want that; a string that merely
   looks like a path will change too); brace groups nested more than one
   level (`use crate::{a::{b, c}}`) are not detected for the
   "not rewritten" report; 2015-edition unprefixed crate-rooted paths from
   nested modules are not resolved (spec Assumptions).

### SC-001 / SC-002 (recorded)

On the build machine (`cargo 1.x` on PATH) the three gated proofs RAN:
the fixture crate `cargo check`s clean before any move, after the in-place
rename (`src/a/b.rs` → `src/a/bee.rs`) and after the directory move
(`src/a/b.rs` → `src/c/b.rs`), each exit 0 through the real composed write
access with `autoApply:true`. On a machine without cargo the suite records a
named skip instead.
