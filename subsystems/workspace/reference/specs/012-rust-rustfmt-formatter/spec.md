# Feature Specification: rustfmt Formatter

**Feature Branch**: `012-rust-rustfmt-formatter`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description (chunk:rust-rustfmt-formatter): "Add a rustfmt FormatterPort
adapter selected for .rs files (Biome for the rest), honoring rust-toolchain.toml, with the
existing fall-back-to-verified-unformatted contract."

> Scope note: constitution Development Workflow rule 8 binds a feature spec to exactly one
> roadmap chunk. This spec covers `chunk:rust-rustfmt-formatter` only. It does NOT change the
> write pipeline's order or contract (dry-run → confirm → shadow apply → parse-verify → format
> → re-verify → journal → swap), does NOT touch the Biome adapter's behavior for the files it
> already formats, and does NOT add a `cargo fmt` command to `run` (that is the runner chunk).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A Rust edit comes back rustfmt-clean (Priority: P1)

An agent edits a `.rs` file through `edit`, `files`, `script` or a plugin. Today the write is
parse-verified and applied but comes back unformatted, so the next `cargo fmt --check` (or the
CI's Format step) is red and the agent spends a turn fixing whitespace. After this feature the
same edit is formatted by the workspace's own rustfmt before it lands, re-verified, and
journaled as the formatted content — exactly how TypeScript edits already go through Biome.

**Why this priority**: it is the chunk's purpose and the single most visible Rust paper cut;
the pipeline already has the format step, so this is one adapter and one selection rule.

**Independent Test**: write an unformatted `.rs` file through the write engine with the real
rustfmt available; the applied content equals `rustfmt`'s output for that file; a `.ts` file
written in the same batch is Biome-formatted as before.

**Acceptance Scenarios**:

1. **Given** rustfmt is installed for the workspace's toolchain, **When** a `.rs` file is
   written through the pipeline, **Then** the applied content is rustfmt's output and the diff
   the caller sees is the formatted one.
2. **Given** the same batch contains a `.ts` file, **When** it is applied, **Then** that file
   is Biome-formatted exactly as before this feature.
3. **Given** a file of any other extension, **When** it is applied, **Then** no formatter is
   spawned and the content is unchanged — as before.
4. **Given** `format: false` (undo's flag), **When** a `.rs` file is restored, **Then** rustfmt
   is not consulted and the bytes are restored exactly.

---

### User Story 2 - A missing or broken rustfmt degrades, never errors (Priority: P1)

The workspace's toolchain may lack the rustfmt component, rustup may be absent, the pinned
channel may not be installed, or rustfmt may reject the file. The write pipeline's contract is
that a formatter failure yields the verified unformatted content plus a note — never an error,
never an unapplied write. After this feature a `.rs` write in any of those situations applies
and the note says what happened and how to fix it.

**Why this priority**: the chunk pins it ("never an error"), and a formatter that can block
writes would turn a nicety into an outage on machines without the component.

**Independent Test**: point the adapter at a rustfmt that does not exist, one that exits
non-zero, and one that hangs; each write applies the verified content with a note; none errors.

**Acceptance Scenarios**:

1. **Given** no runnable rustfmt can be resolved, **When** a `.rs` file is written, **Then** it
   is applied unformatted and the note names the cause and the remedy (install the component).
2. **Given** rustfmt exits non-zero for the content, **When** the file is written, **Then** it
   is applied unformatted with a note carrying rustfmt's first error line.
3. **Given** rustfmt hangs, **When** the file is written, **Then** it is killed at the timeout
   and the file is applied unformatted with a note.
4. **Given** rustfmt returns content that no longer parses, **When** the file is written,
   **Then** the pipeline's existing "formatting broke the parse" fallback applies the verified
   content — unchanged behavior.

---

### User Story 3 - The workspace's toolchain and config are honored (Priority: P2)

A Rust workspace pins its toolchain in `rust-toolchain.toml`, declares its edition in
`Cargo.toml`, and may carry a `rustfmt.toml`. A formatter that used some other rustfmt, the
wrong edition, or the wrong config would produce diffs the project's own `cargo fmt` would
then undo. After this feature the adapter asks the toolchain manager for the pinned rustfmt
(the way the resolver chunk asks for rust-analyzer), passes the crate's edition, and runs from
the file's directory so rustfmt discovers the project's config the way `cargo fmt` would.

**Why this priority**: correctness of the formatted output relative to the project's own
tooling; without it the feature produces churn instead of removing it.

**Independent Test**: a fixture with `rust-toolchain.toml` pinning an installed channel formats;
one pinning an uninstalled channel degrades with a note; a 2024-edition crate with
edition-specific syntax formats; a workspace `rustfmt.toml` option is visibly applied.

**Acceptance Scenarios**:

1. **Given** `rust-toolchain.toml` pins an installed channel with the rustfmt component,
   **When** a `.rs` file is written, **Then** that channel's rustfmt is the one that runs.
2. **Given** the pin names a channel that is not installed, **When** a `.rs` file is written,
   **Then** the write applies unformatted and the note says the pinned toolchain's rustfmt could
   not be resolved.
3. **Given** the crate (or its workspace) declares `edition = "2024"`, **When** a file using
   2024-only syntax is written, **Then** rustfmt is run with that edition and formats it.
4. **Given** a `rustfmt.toml` in the workspace, **When** a `.rs` file is written, **Then** its
   options are reflected in the output (e.g. `max_width`).

---

### User Story 4 - The cost is measured (Priority: P2)

Formatting on every Rust write adds a process spawn per file. After this feature the per-edit
overhead is measured on the real rustfmt — cold (including toolchain resolution) and warm — and
recorded, so the cost is a number rather than a feeling.

**Independent Test**: a toolchain-gated test times the adapter on a small file and records the
cold and warm milliseconds.

**Acceptance Scenarios**:

1. **Given** rustfmt is available, **When** the adapter formats a small file twice, **Then**
   cold and warm durations are recorded in the ship trace.

---

### Edge Cases

- A `.rs` file outside any Cargo crate (no `Cargo.toml` ancestor): edition defaults to the
  newest the adapter knows rustfmt accepts for the resolved binary (2021 as the safe default),
  config discovery still runs from the file's directory.
- A crate that inherits `edition.workspace = true`: the edition comes from the root's
  `[workspace.package]`.
- Non-UTF-8 or enormous content: rustfmt's own failure is a note, never an error.
- rustfmt warnings on stderr with exit 0: output is used; warnings are not surfaced.
- Two `.rs` files in one batch: formatted independently; one failing does not affect the other.
- Windows paths and the toolchain manager's absolute binary path: spawned directly, no shell.
- The resolution result is memoized per adapter instance (per server process): a rustfmt
  installed mid-session is picked up on the next server start, like rust-analyzer detection.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The write pipeline MUST format `.rs` files with rustfmt and all other files
  exactly as today (Biome for its extensions, pass-through otherwise), via one composed
  formatter that routes by extension; the pipeline's format step MUST be unchanged.
- **FR-002**: rustfmt MUST be resolved through the toolchain manager with the workspace root
  as the working directory (honoring `rust-toolchain.toml`), falling back to a runnable
  `rustfmt` on PATH; a binary that does not run MUST be treated as absent.
- **FR-003**: Any failure to format — unresolvable binary, non-zero exit, timeout, spawn error
  — MUST yield the verified unformatted content plus a note; it MUST NOT produce an error
  envelope or an unapplied write.
- **FR-004**: The note for a formatter failure MUST carry the failure's message so the agent can
  tell "rustfmt not installed" from "rustfmt rejected the content".
- **FR-005**: The adapter MUST pass the crate's edition (`[package] edition`, else the root's
  `[workspace.package] edition`, else a safe default) to rustfmt.
- **FR-006**: The adapter MUST run rustfmt with the file's directory as the working directory so
  project `rustfmt.toml`/`.rustfmt.toml` discovery matches `cargo fmt`.
- **FR-007**: The adapter MUST enforce a timeout and kill a hung rustfmt.
- **FR-008**: Per-edit formatting overhead (cold and warm) MUST be measured on the real rustfmt
  and recorded (Constitution VI).
- **FR-009**: `format: false` MUST bypass rustfmt exactly as it bypasses Biome.
- **FR-010**: No new dependency; no TOML parser (edition extraction is textual, like the
  manifest reads elsewhere).

### Key Entities

- **Routing formatter**: the composed FormatterPort that picks rustfmt for `.rs` and delegates
  everything else to the existing Biome adapter.
- **rustfmt adapter**: resolution (toolchain manager → PATH), edition lookup, cwd policy,
  spawn/timeout, failure-as-throw.
- **Formatter note**: the pipeline's degradation note, now carrying the failure message.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With rustfmt available, a `.rs` write through the pipeline applies rustfmt's
  output; a `.ts` write in the same batch is Biome-formatted; other extensions pass through;
  `format:false` bypasses — pinned by tests (rustfmt cases toolchain-gated).
- **SC-002**: Missing binary, non-zero exit, and timeout each apply the verified unformatted
  content with a note naming the cause — pinned by tests with injected binaries (no toolchain
  needed).
- **SC-003**: The pinned toolchain's rustfmt is the one resolved; an uninstalled pin degrades
  with a note; the edition is read from the crate or the workspace; a `rustfmt.toml` option is
  reflected — pinned (toolchain-gated where a real rustfmt is needed).
- **SC-004**: Cold and warm per-edit overhead recorded in the ship trace.
- **SC-005**: Existing Biome and write-engine tests pass unchanged apart from the widened note
  text.
- **SC-006**: Full gate sweep green.

## Assumptions

- rustfmt reads stdin and writes the formatted result to stdout, exits non-zero on a parse
  error, and accepts `--edition`; config is discovered from the working directory when the input
  is stdin (verified on rustfmt 1.9.0 during shaping).
- `rustup which rustfmt` with the workspace root as cwd resolves the pinned toolchain's binary
  and fails when the component is missing — the same behavior the resolver chunk relies on for
  rust-analyzer.
- rustfmt may be absent on CI machines; toolchain-dependent tests gate on a runnable rustfmt and
  skip with a reason; the degradation paths are pinned with injected fake binaries everywhere.
