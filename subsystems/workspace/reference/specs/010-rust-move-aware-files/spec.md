# Feature Specification: Rust Move-Aware Files

**Feature Branch**: `010-rust-move-aware-files`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description (chunk:rust-move-aware-files): "Make files move/rename Rust-aware:
rewrite mod declarations, use crate::/super::/self:: paths and Cargo.toml [[bin]] path /
workspace members in the one dry-run diff, confirm-gated."

> Scope note: constitution Development Workflow rule 8 binds a feature spec to exactly one
> roadmap chunk. This spec covers `chunk:rust-move-aware-files` only. It does NOT map Cargo
> workspaces (`chunk:rust-cargo-workspace-map`), does NOT wire the file watcher
> (`chunk:rust-watcher-wiring`), and does NOT change how any non-Rust language's move behaves
> (that behavior is pinned byte-for-byte, FR-011).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Renaming a Rust module file leaves a compiling crate (Priority: P1)

An agent renames a module file in place — `src/a/b.rs` becomes `src/a/bee.rs`. Today the
tool moves the file and says plainly that Rust imports are not rewritten, so the crate no
longer compiles: the parent's `mod b;` points at a file that is gone, and every
`crate::a::b::…`, `super::b::…`, `self::b::…` and `b::…` path in the crate names a module that
no longer exists. After this feature, the same call rewrites the declaring `mod b;` to
`mod bee;`, rewrites every path in the crate that resolves to the old module, and shows all of
it in the one confirm-gated diff — and the crate compiles after apply.

**Why this priority**: rename-in-place is the single most common Rust file move, and it is
fully deterministic: the module's parent does not change, so every rewrite is a name swap in a
known set of places. It is the MVP because it proves the whole mechanism (declaration,
absolute paths, relative paths, uniform paths) on the simplest geometry.

**Independent Test**: a fixture crate with one leaf module referenced by `crate::`-absolute,
`super::`-relative, `self::`-relative and uniform (unprefixed) paths from three different
files; rename the module file; the declaration and every path are rewritten, and `cargo check`
passes on the result.

**Acceptance Scenarios**:

1. **Given** `src/a.rs` declares `pub mod b;` and `src/a/b.rs` exists, **When** the agent moves
   `src/a/b.rs` to `src/a/bee.rs`, **Then** the diff rewrites that declaration to `pub mod bee;`
   — visibility, attributes and trailing comments preserved.
2. **Given** `src/lib.rs` contains `use crate::a::b::helper;`, **When** the same move runs,
   **Then** the diff rewrites it to `use crate::a::bee::helper;`.
3. **Given** `src/a.rs` contains `self::b::helper()` and `b::helper()` (a uniform path whose
   first segment is a module declared in that file), **When** the same move runs, **Then** both
   are rewritten to name `bee`, in the same relative style.
4. **Given** `src/a/other.rs` contains `super::b::helper()`, **When** the same move runs,
   **Then** it is rewritten to `super::bee::helper()`.
5. **Given** the rewritten fixture, **When** `cargo check` runs on it, **Then** it exits 0.

---

### User Story 2 - Moving a Rust module file to another directory leaves a compiling crate (Priority: P1)

An agent moves `src/a/b.rs` to `src/c/b.rs`. The module's parent changes: the declaration must
leave `src/a.rs` and appear in `src/c.rs` (or `src/c/mod.rs`), every crate path that resolved
to `crate::a::b` must now resolve to `crate::c::b`, and the moved file's own `super::…` paths —
which now point at a different parent — must be re-rooted so they still reach what they
reached before. After this feature all of that rides the same diff, and the crate compiles.

**Why this priority**: it is the second geometry the mechanism must handle and the one the
Rust dogfood gate names ("move a module"). It differs from US1 in exactly two ways — the
declaration moves between files, and relative paths can stop being expressible relatively —
so it is independently testable on top of US1.

**Independent Test**: the US1 fixture plus a second parent module; move the leaf module under
it; the old parent loses the declaration, the new parent gains it, every path is rewritten
(absolute where the old relative form no longer reaches), and `cargo check` passes.

**Acceptance Scenarios**:

1. **Given** `src/a.rs` declares `pub mod b;` and `src/c.rs` exists as a module file, **When**
   the agent moves `src/a/b.rs` to `src/c/b.rs`, **Then** the diff removes the declaration
   from `src/a.rs` and adds `pub mod b;` to `src/c.rs`, keeping the original visibility.
2. **Given** `src/a.rs` contained `self::b::helper()` (no longer reachable relatively after the
   move), **When** the same move runs, **Then** it is rewritten to `crate::c::b::helper()`.
3. **Given** the moved file contained `super::sibling()` (a function in `src/a.rs`), **When**
   the same move runs, **Then** the moved file's path is rewritten to
   `crate::a::sibling()` so it still reaches the same item.
4. **Given** no `src/c.rs` and no `src/c/mod.rs` exist, **When** the move runs, **Then** the
   old declaration is still removed (it would otherwise point at a missing file), paths are
   still rewritten to the new module path, and the notes say exactly which parent module file
   is missing and what declaration it needs — nothing is synthesized.
5. **Given** the rewritten fixture, **When** `cargo check` runs on it, **Then** it exits 0.

---

### User Story 3 - Manifest facts that a file move can change are rewritten; those it cannot are stated (Priority: P2)

A Cargo manifest can pin a target to a file path: `[[bin]] path = "src/tools/cli.rs"`,
`[lib] path = "src/lib_root.rs"`. Moving that file today silently breaks the build. After this
feature, a path entry that names the moved file is rewritten in `Cargo.toml` inside the same
diff. Workspace members and `path` dependencies, by contrast, name directories — and the
files tool moves single files, never directories — so no single-file move can change them;
this feature states that rather than pretending to handle it.

**Why this priority**: it is the remaining deterministic rewrite the chunk names, and the
"workspace members" half of the chunk's title turns out, on reading the tool, to have no
trigger in v1 — recording that honestly is part of the unit.

**Independent Test**: a manifest with a `[[bin]] path` entry naming a file; move the file; the
entry is rewritten, the manifest is still well-formed TOML, and a manifest without such an
entry is untouched.

**Acceptance Scenarios**:

1. **Given** `Cargo.toml` has `[[bin]] name = "cli" path = "src/tools/cli.rs"`, **When** the
   agent moves `src/tools/cli.rs` to `src/bin/cli.rs`, **Then** the entry's path is rewritten
   to `src/bin/cli.rs` and the manifest stays well-formed.
2. **Given** `Cargo.toml` names the moved file in no path entry, **When** a move runs, **Then**
   the manifest is not part of the diff.
3. **Given** the moved file is an auto-discovered target (`src/main.rs`, `src/bin/<name>.rs`,
   `tests/<name>.rs`, `examples/<name>.rs`, `benches/<name>.rs`), **When** it is moved,
   **Then** the notes say the target is discovered by location and name the manifest key that
   would pin it — no manifest edit is invented.
4. **Given** a `[workspace] members` list, **When** any single-file move runs, **Then** it is
   unchanged — verified by test, and stated in the specification as not applicable to v1's
   single-file moves.

---

### User Story 4 - The notes say exactly what was and was not rewritten (Priority: P2)

The move's notes today say "rust/go imports name modules, not files, and are not rewritten".
After this feature, a Rust move's notes name the module path before and after, the
declaration file that changed, the number of paths rewritten across how many files, and —
case by case — what was deliberately left alone: paths inside a brace-grouped `use` tree that
name the moved module (reported with file and line), a moved module that has child modules
(the move proceeds as a plain file move; no Rust rewrite is attempted, because the children
would be orphaned), a file whose declaration could not be found, a `#[path]`-declared module,
and paths from other crates in the workspace that name this crate's module. The generic note
stops mentioning Rust and keeps its honest claim for Go.

**Why this priority**: Constitution III's diff-before-apply only protects the user if the diff
predicts the outcome. A rewrite that is silently partial is worse than none, so the partiality
is stated in the same envelope.

**Independent Test**: run the moves above and the negative shapes; read the notes.

**Acceptance Scenarios**:

1. **Given** a successful Rust move, **When** the notes are read, **Then** they name
   `crate::<old>` → `crate::<new>`, the declaration file, and the rewrite counts.
2. **Given** an importer with `use crate::a::{b, other};`, **When** `b` is moved, **Then** the
   brace member is not rewritten and the notes name the file and line so the agent can finish.
3. **Given** a module file with child modules (`src/a/mod.rs` beside `src/a/x.rs`, or
   `src/a.rs` beside `src/a/`), **When** it is moved to a different module path, **Then** no
   Rust rewrite is attempted and the notes say why and list the children.
4. **Given** a `.rs` file whose declaring `mod` item cannot be found by convention, **When** it
   is moved, **Then** no Rust rewrite is attempted and the notes say the declaration was not
   found.
5. **Given** a Go move, **When** the notes are read, **Then** they still say Go imports are not
   rewritten, and no note mentions Rust.

---

### Edge Cases

- `src/a/mod.rs` ↔ `src/a.rs` (style conversion of the same module): the module path does not
  change, so no declaration or path rewrite is needed; the notes say so.
- A module declared in both `src/lib.rs` and `src/main.rs` (one file, two crate roots): the
  declaration is rewritten wherever it is found among the candidate parents.
- Declarations carrying visibility and attributes (`pub(crate) mod b;`, `#[cfg(test)] mod b;`,
  a `///` doc line, a trailing `// comment`): rewritten in place, everything but the name kept.
- `super::super::x` chains: resolved against the importer's module location, any depth.
- `pub use` re-exports and glob imports (`use crate::a::b::*;`): same contiguous-path rule.
- A path that merely shares a prefix (`crate::a::bb::x` when `crate::a::b` moves): not rewritten
  — segment boundaries are respected.
- `#[path = "…"]` declarations: out of scope; detected and stated, not rewritten.
- Paths that name this crate from a sibling workspace crate (`mycrate::a::b`): out of scope;
  stated in the notes.
- The moved file lives outside any Cargo crate (no `Cargo.toml` ancestor): the workspace root
  is treated as the crate directory and the convention still applies.
- Files under `target/` are never scanned or rewritten (already junk-classed).
- The generated rewrite is subject to the same dry-run diff, confirm gate, parse-verify and
  journal as any other write (Constitution III); `Cargo.toml` passes through the existing TOML
  well-formedness check.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Moving a `.rs` file MUST rewrite the `mod <name>;` declaration in its parent
  module file: renamed in place when the parent is unchanged, removed from the old parent and
  added to the new parent (when that parent module file exists) when the parent changes.
  Visibility, attributes and surrounding text on the declaration MUST be preserved.
- **FR-002**: Every `crate::`-rooted path in the crate that resolves to the moved module or an
  item inside it MUST be rewritten to the new module path, respecting segment boundaries.
- **FR-003**: Every `super::`-, `self::`- or uniform (unprefixed, first segment a module
  declared in the same file) path MUST be resolved against the importer's own module location
  and rewritten when it resolves to the moved module — re-emitted in the same relative form
  when that form still reaches the new location, otherwise as a `crate::`-absolute path.
- **FR-004**: The moved file's own `super::` paths MUST be re-rooted when its parent changes so
  they still reach the items they reached before.
- **FR-005**: A `[[bin]]`, `[lib]`, `[[test]]`, `[[example]]` or `[[bench]]` manifest `path`
  entry that names the moved file MUST be rewritten in `Cargo.toml`, and the manifest MUST
  remain well-formed.
- **FR-006**: Every rewrite MUST ride the one dry-run diff and the existing confirm gate; a
  Rust move MUST remain confirm-gated and nothing MUST apply without confirmation (or the
  existing trusted-harness bypass).
- **FR-007**: When a rewrite is deliberately not performed — brace-grouped `use` members,
  non-leaf modules, undiscoverable declarations, `#[path]` declarations, cross-crate paths,
  auto-discovered targets, a missing destination parent module file — the notes MUST say so
  specifically enough (file, line where applicable, and what is needed) for the agent to
  finish by hand.
- **FR-008**: The generic "not rewritten" note MUST no longer mention Rust; it MUST keep its
  claim for Go.
- **FR-009**: Module paths MUST be derived from file location by Rust's file-module conventions
  (`src/lib.rs`/`src/main.rs`/declared and auto-discovered target roots, `<name>.rs`,
  `<name>/mod.rs`) and MUST be confirmed by locating the actual declaring `mod` item; when it
  cannot be found, no Rust rewrite is attempted.
- **FR-010**: Nothing MUST be synthesized: no new module file, no new manifest entry, no
  declaration added to a file that does not exist.
- **FR-011**: Non-Rust languages MUST be byte-for-byte unaffected: their rewrites, gating and
  notes are unchanged by this feature — pinned.
- **FR-012**: Workspace members and `path` dependencies are directory facts that a single-file
  move cannot change; the feature MUST verify they are untouched and MUST state, in the spec
  and plan, that this half of the chunk's title has no v1 trigger rather than claim it as work.

### Key Entities

- **Module path**: the `crate::a::b` address of a `.rs` file, derived from its location
  relative to its crate root and confirmed by its declaring `mod` item.
- **Declaration**: the `mod <name>;` item in the parent module file that binds a file into the
  module tree, with its visibility and attributes.
- **Path rewrite**: one contiguous `crate::`/`super::`/`self::`/uniform path in one file whose
  resolved module is the moved module, plus the form it is re-emitted in.
- **Unrewritten site**: a place the feature deliberately did not change, with the reason and
  location, surfaced in the notes.
- **Manifest path entry**: a `path = "…"` value under a target table in `Cargo.toml`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a fixture crate, renaming a leaf module file in place rewrites the declaration
  and every `crate::`, `super::`, `self::` and uniform path that resolves to it, and
  `cargo check` exits 0 on the result — pinned by a toolchain-gated test.
- **SC-002**: In the same fixture, moving the leaf module under another parent removes and adds
  the declaration, re-roots the moved file's `super::` paths, rewrites importers (absolute where
  needed), and `cargo check` exits 0 — pinned by a toolchain-gated test.
- **SC-003**: A manifest `[[bin]] path` naming the moved file is rewritten and the manifest
  stays well-formed; a manifest without such an entry is not in the diff; workspace members
  are untouched — pinned by test.
- **SC-004**: Every Rust move answers `CONFIRM_REQUIRED` with the aggregated diff before
  applying, and `dryRun:true` applies nothing — pinned by test.
- **SC-005**: Each negative shape (brace-group member, non-leaf module, undiscoverable
  declaration, missing destination parent, auto-discovered target) produces the specific note
  the scenario names — pinned by test.
- **SC-006**: The six existing non-Rust move fixtures and the Go note are unchanged — pinned by
  the existing tests continuing to pass without edits.
- **SC-007**: The full gate sweep stays green.

## Assumptions

- Rust 2018+ path semantics: `crate::` is the crate root, `super::` the parent module,
  `self::` the current module, and an unprefixed path whose first segment is a module declared
  in the current file resolves relative to that module. 2015-edition crates that rely on
  unprefixed crate-rooted paths from nested modules are out of scope and not detected.
- Module files follow the documented conventions (`<name>.rs`, `<name>/mod.rs`, target roots);
  `#[path]` overrides are rare enough to be detected and stated, not rewritten.
- The files tool moves single files in v1 (directories are refused today); directory moves,
  and therefore non-leaf module moves and workspace-member changes, are out of scope here.
- `cargo` may be absent on a test machine; the compile-proof tests gate on its presence
  (the established 008 pattern) and every other property is pinned by toolchain-free tests.
- No TOML parser is introduced: manifest rewrites are textual edits to a `path = "…"` value
  verified by the existing well-formedness check, matching how `map` reads the manifest today.
