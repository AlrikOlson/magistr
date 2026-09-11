# Phase 0 Research: Rust Move-Aware Files

Grounded in `src/tools/files.ts` (`resolveMove`, `importerExtensions`, `rewriteImporter`,
`rewriteMovedContent`, the move note, `alwaysGated`), `src/tools/write-engine.ts`
(`WELL_FORMED_EXTENSIONS` — `toml` is checked with `Bun.TOML.parse`, not parse-verified),
`src/tools/map.ts` (`cargoEntryPoints`: regex `[[bin]]` name/path extraction, no TOML
parser), `test/files-tool.test.ts` (`MOVE_FIXTURES` for six languages and the pinned
"rust and go imports are honestly not rewritten" test), `test/resolver-plugin.test.ts`
(`Bun.which("cargo")` + `--version` gating), `schemas/tools/files.input.schema.json`, and
`skills/workbench-routing/SKILL.md` lines 84 and 132. Pinned in think:160/161.

## R1 — Derive the module path by convention, then CONFIRM it by finding the declaration

**Decision**: a `.rs` file's module path is computed from its location relative to its crate
root directory (the nearest ancestor holding `Cargo.toml`, else the workspace root) using the
file-module conventions: a root file (`src/lib.rs`, `src/main.rs`, `src/bin/<n>.rs`,
`src/bin/<n>/main.rs`, `tests|examples|benches/<n>.rs`, `…/<n>/main.rs`, `build.rs`, and any
manifest `path` target) is the crate root `crate`; below it, each directory segment is a
module, `<name>.rs` is module `name`, `<name>/mod.rs` is module `name`. The parent module file
of `src/a/b.rs` is `src/a.rs` or `src/a/mod.rs` (or the root file when the file sits directly
under the root directory). The derivation is then CONFIRMED by locating a `mod b;` item (any
visibility, any attributes) in that parent; if none is found, no Rust rewrite is attempted and
the notes say so.

**Rationale**: convention alone would silently mis-rewrite a `#[path]`-declared or orphaned
file; finding the declaration makes the rewrite safe by construction — the only files changed
are ones the tool could prove are bound into the module tree. It also gives the note a
concrete file to name.

**Alternatives considered**: walking every `mod` declaration in the crate to build the tree
bottom-up (rejected: O(crate) parsing for every move, and the convention already names the one
or two files that can declare the module); using the Tier-1 index (rejected: the index is
optional — Constitution I — and moves must work cold).

## R2 — Rewrite contiguous paths only; brace-grouped `use` members are REPORTED, not rewritten

**Decision**: a path is rewritten when it is a contiguous textual path (`crate::a::b`,
`crate::a::b::helper`, `super::b::…`, `self::b::…`, `b::…`) whose resolved module is the
moved module or an item inside it, at segment boundaries. A `use` tree where the moved module
appears as a brace member (`use crate::a::{b, other};`, `use crate::a::{b::helper, x};`) is
NOT rewritten; its file and line go into the notes.

**Rationale**: the contiguous case is a deterministic text substitution inside one path, which
is exactly what every other family's rewrite already is (specifier-based, name-resolution-free,
§14). Splitting a use tree is a structural edit with several valid outputs; guessing one in a
move would be the overclaim the original note refused. Naming the site lets the agent finish
with one `edit`.

**Alternatives considered**: splitting the group into two `use` items (rejected: several valid
renderings, and rustfmt would re-group them anyway); leaving the group silently (rejected:
violates the honest-notes requirement).

## R3 — Relative and uniform paths resolve against the IMPORTER's module, and re-emit relative when they still can

**Decision**: for an importer at module `M`, `self::x` resolves to `M::x`, `super::x` to
`parent(M)::x` (chains allowed), and an unprefixed path `x::…` resolves to `M::x::…` only when
the importer file itself declares `mod x;` (or `mod x {`). If the resolved path is the moved
module (or inside it), it is rewritten: in the same relative form when that form reaches the
new module path from `M` (rename in place: `super::b` → `super::bee`, `b::helper` →
`bee::helper`), otherwise as `crate::<new path>`.

**Rationale**: the module-location derivation from R1 gives every importer its `M` for free;
re-emitting relative where possible keeps diffs minimal and idiomatic for the common rename,
and the absolute fallback is always correct after a directory move.

**Alternatives considered**: always emitting `crate::`-absolute (rejected: turns a one-token
rename into a style change across the crate); trying to compute a new relative form through
`super::` chains (rejected: more cases, no compile benefit).

## R4 — The moved file's own `super::` paths are re-rooted; its `self::` paths are not

**Decision**: when the parent changes, every `super::…` path inside the moved file is rewritten
to the `crate::`-absolute path it resolved to before the move (`super::sibling` in `crate::a::b`
→ `crate::a::sibling`). `self::` paths move with the file and are untouched. `crate::` paths
inside the moved file follow R2 like any importer's.

**Rationale**: `super::` is the one relative form whose meaning changes with the file's
location; leaving it would make the moved file compile against the wrong parent.

## R5 — Parent-change rules: remove always, add only into an existing parent, synthesize nothing

**Decision**: when the parent module changes, the declaration is removed from the old parent
unconditionally (it would otherwise name a missing file), and added to the new parent module
file (`<dir>.rs` or `<dir>/mod.rs`, whichever exists; preference to the existing one) with
the original visibility and attributes, appended after the last existing `mod` item or at the
top. When neither parent file exists, nothing is created; the note names the missing file and
the declaration it needs, and the declaration that the grandparent would need.

**Rationale**: creating module files and chaining declarations up to the root is authoring,
not relocating — and it has several defensible shapes. The tool's job is a predictable diff.

## R6 — Non-leaf modules are not rewritten; same-module style conversion needs no rewrite

**Decision**: a moved file is non-leaf when it is `mod.rs` with sibling `.rs` files or
directories, or `<name>.rs` with a `<name>/` directory beside it. Moving a non-leaf module to a
different module path attempts no Rust rewrite (plain file move, as today) and the note lists
the child modules and says directories are not movable in v1. Moving `src/a/mod.rs` to
`src/a.rs` (or back) keeps the module path, so nothing needs rewriting; the note says so.

**Rationale**: files v1 refuses directories (`FILE_NOT_FOUND` says "directories are not
movable"), so the children cannot follow; a half-rewrite would produce a crate that is broken
in a new way. The style conversion is a real refactor that is correct with zero rewrites.

## R7 — Manifest facts: rewrite `path` target entries textually; workspace members are NOT APPLICABLE

**Decision**: in the crate's `Cargo.toml`, a `path = "<moved file, crate-relative>"` value under
`[lib]`, `[[bin]]`, `[[test]]`, `[[example]]` or `[[bench]]` is rewritten to the new
crate-relative path as a textual edit (the same section-split regex discipline as
`map.cargoEntryPoints`), and the write engine's TOML well-formedness check verifies the
result. `[workspace] members` and `path = "../x"` dependencies name DIRECTORIES, which a
single-file move cannot change; the feature pins that they are untouched and records this half
of the chunk title as having no v1 trigger. An auto-discovered target that is moved gets a
note naming the table that would pin it.

**Rationale**: no new dependency (a TOML parser would also reformat the file); the write
engine already guarantees well-formedness; claiming "workspace members rewritten" for an event
that cannot occur would be the kind of claim the 009 unit demoted to verification.

## R8 — Scope of the scan: the crate's `.rs` files, excluding junk; cross-crate paths are out

**Decision**: importer scanning covers every non-junk `.rs` file under the crate directory
(`target/` is already junk). Paths that name this crate from a sibling workspace crate
(`mycrate::a::b`) are out of scope; the note says so when the moved module is `pub`-visible.

**Rationale**: the crate-name prefix rewrite is deterministic too, but it needs the package
name (hyphen → underscore) and a workspace-wide scan, and it belongs with the workspace map
chunk's knowledge; filed as a follow-up rather than folded in.

## R9 — Code placement: a pure module beside files.ts, dispatched from resolveMove

**Decision**: the Rust logic lives in a new `src/tools/rust-move.ts` exporting pure functions
(module-path derivation, declaration lookup/rewrite, path resolution/rewrite, manifest entry
rewrite, and one `planRustMove(...)` that returns writes + notes over the same
`files`/`read` inputs `resolveMove` already has). `resolveMove` gains a branch for `.rs`
sources that merges those writes into the existing `writes`/`beforeByPath` and replaces the
generic note with the Rust note; `importerExtensions("rs")` stays `[]` so the generic
specifier loop is untouched for Rust and unchanged for everyone else.

**Rationale**: `files.ts` is already 808 lines and its per-family helpers are text
substitutions; the Rust rules need importer-module context and a declaration edit, which is a
different shape. A pure module tests in isolation over in-memory files (fast, no toolchain).

## R10 — Proof: toolchain-gated `cargo check` plus toolchain-free unit pins

**Decision**: the compile-after-apply acceptance is a test that builds a fixture crate in a
temp directory, runs the move through the real composed write access with `autoApply:true`,
then runs `cargo check --offline` (with an isolated `CARGO_TARGET_DIR`) and asserts exit 0.
It is gated on `Bun.which("cargo")` answering `--version` exit 0 (the 008 pattern), skips with
a reason otherwise. Every other property — each rewrite rule, each negative note, gating,
dry-run, manifest — is pinned by toolchain-free tests over the in-process rig.

**Rationale**: the bun lane's in-process parse is bracket-balance only, so "compiles" can only
be proven by the compiler; gating keeps CI honest where cargo is absent without weakening what
is pinned everywhere else.
