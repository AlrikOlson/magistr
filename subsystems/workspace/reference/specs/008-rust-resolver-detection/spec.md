# Feature Specification: Rust in the Resolver Detection Table

**Feature Branch**: `008-rust-resolver-detection`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description (chunk:rust-resolver): "Add rust-analyzer to the resolver plugin's host detection table so references over Rust reach Tier-2 precision (trait methods, generics, macro-generated items) on the unchanged public resolver interface. The guest addresses a languageId; the HOST decides the binary, so Rust is a detection entry (Cargo.toml present + rust-analyzer discoverable) plus toolchain from rust-toolchain.toml. Depends on the server session pool: rust-analyzer takes seconds to initialize and would circuit-break under the per-request budget every time."

> Scope note: constitution Development Workflow rule 8 binds a feature spec to exactly one
> roadmap chunk. This spec covers `chunk:rust-resolver` only. It builds on
> `chunk:backlog-resolver-server-pool` (the warm pool, shipped) and does NOT change the
> resolver interface, the guest's LSP client, or the pool. Rename/diagnostics precision is a
> separate future feature (the bundled guest answers them empty for every language today);
> this feature is references precision, matching the shipped surface.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - References over a Rust project reach language-server precision (Priority: P1)

An agent works in a Cargo project. It asks for references to a symbol — a trait method, a
generic function, an item produced by a derive or a macro. Today the answer is tree-sitter
symbolic matching by name. With this feature the workbench detects the project's
rust-analyzer, and — because the server session pool keeps it warm — references arrive at
Tier-2 precision labeled `resolver-lsp`/`high`, on the same public resolver interface every
other language uses.

**Why this priority**: it is the chunk's reason to exist and the first place the daily-driver
claim is tested against a real Rust workspace. The pool made a slow server viable; this makes
it fire for Rust without the caller hand-building a launch spec.

**Independent Test**: in a Cargo workspace with rust-analyzer available, ask for references
twice; the second answer carries `resolver-lsp` edges over the real fixture. With
rust-analyzer unavailable, the answer is the byte-identical tree-sitter result.

**Acceptance Scenarios**:

1. **Given** a workspace with a `Cargo.toml` and a discoverable rust-analyzer, **When** the
   server starts, **Then** the detection table contains a Rust entry whose launch spec names
   the toolchain's real rust-analyzer binary.
2. **Given** that workspace and a warm pool, **When** references for an unambiguous symbol are
   requested, **Then** the upgraded edges are labeled `resolver-lsp`/`high` and point at the
   real reference sites, including sites a name-based symbolic pass would miss (a trait-impl
   method, a generic call).
3. **Given** the same request against a symbol rust-analyzer resolves inside macro-expanded
   code, **When** references are returned, **Then** they are the language server's, not the
   symbol index's (which does not see macro output).

---

### User Story 2 - No rust-analyzer, no regression (Priority: P1)

A developer without rust-analyzer installed, or on a toolchain that lacks the component, gets
exactly the product they had before: every tool answers at tree-sitter fidelity, immediately,
with no doomed server spawn and no error surfaced.

**Why this priority**: the standing "No LSP requirement" refusal. Detection that emitted a
launch spec for a binary that cannot run would spawn a server on the first request, waste it,
and record a breach — a regression disguised as a feature.

**Independent Test**: run detection in a Cargo workspace with no rust-analyzer resolvable; the
table gains no Rust entry; every reference answer is tree-sitter symbolic. Run it where only
the rustup proxy exists without the component; still no entry.

**Acceptance Scenarios**:

1. **Given** a `Cargo.toml` workspace where rust-analyzer cannot be resolved to a runnable
   binary, **When** detection runs, **Then** no Rust entry is produced and no server is ever
   spawned.
2. **Given** a workspace with no `Cargo.toml`, **When** detection runs, **Then** it produces no
   Rust entry regardless of whether rust-analyzer is installed.
3. **Given** a Rust entry is present but rust-analyzer later crashes or times out, **When**
   references are requested, **Then** the existing circuit breaker degrades to tree-sitter
   fidelity — unchanged behavior from the pool feature.

---

### User Story 3 - The project's toolchain is honored (Priority: P2)

A workspace pins a toolchain (`rust-toolchain.toml`). Detection resolves the rust-analyzer that
belongs to that toolchain, not some other one on the machine, so the references reflect the
project's actual compiler view.

**Why this priority**: a Rust monorepo commonly pins a channel; resolving the wrong
rust-analyzer would answer against the wrong sysroot. Cheap to get right by asking the
toolchain manager, expensive to get wrong.

**Independent Test**: with a pinned toolchain that has the component, the resolved binary path
is under that toolchain; with a pinned toolchain that lacks it, no entry is produced.

**Acceptance Scenarios**:

1. **Given** a workspace whose pinned toolchain has rust-analyzer, **When** detection resolves
   the binary, **Then** it is the toolchain manager's answer for that workspace, honoring the
   pin.
2. **Given** a workspace whose pinned toolchain lacks the component, **When** detection runs,
   **Then** no Rust entry is produced (the server is honestly unavailable).

---

### Edge Cases

- Only the rustup proxy is on PATH and the active toolchain lacks the component: the proxy runs
  but errors "Unknown binary" — detection must NOT emit an entry for it (verified, not assumed).
- A `Cargo.toml` that is a virtual workspace manifest (no `[package]`, only `[workspace]`):
  still a Rust project; rust-analyzer handles the workspace root.
- rust-analyzer resolvable but the workspace has no `.rs` files yet: detection still emits the
  entry; references simply return empty until there is code — no error.
- Multiple detectable servers (a TS project that also has a `Cargo.toml`): both entries are
  produced; each language is served independently (the pool keys per language).
- A bare `rust-analyzer` binary on PATH that is not a rustup proxy (a standalone install):
  accepted if it runs.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The host detection table MUST gain a Rust entry when the workspace root contains a
  `Cargo.toml` AND a rust-analyzer binary can be resolved to something that actually runs.
- **FR-002**: Binary resolution MUST prefer the toolchain manager's answer for the workspace
  (honoring a pinned `rust-toolchain.toml`), falling back to a PATH rust-analyzer only when it
  is confirmed runnable and is not an unusable proxy.
- **FR-003**: Detection MUST NOT emit a Rust entry for a binary that cannot run (an
  uninstalled-component proxy), so no doomed server is ever spawned.
- **FR-004**: The Rust entry's launch spec MUST address `languageId: "rust"` with the resolved
  binary and rust-analyzer's default stdio LSP arguments; the guest and the resolver interface
  are unchanged.
- **FR-005**: With no Rust entry, every tool MUST answer at tree-sitter fidelity exactly as
  before — no new latency, no error, no spawn.
- **FR-006**: Detection's binary resolution MUST be injectable so tests never shell out to a
  real toolchain manager; the real resolution path MUST be exercised only in an
  environment-gated test that names the binaries it needs.
- **FR-007**: The change MUST be additive to `detectLanguageServers`: the existing TypeScript
  entry and its behavior are unchanged, and detection remains a one-time, startup-time,
  off-the-request-path operation.
- **FR-008**: References precision is the scope. Rename and diagnostics remain the guest's
  MVP-empty answers for Rust as for every language; this feature MUST NOT claim rename
  precision.

### Key Entities

- **Rust detection entry**: a host-curated launch spec (`languageId: "rust"`, resolved binary,
  stdio args) produced by detection when a Cargo project with a runnable rust-analyzer is found.
- **Binary resolver**: the injectable step that turns "this is a Rust workspace" into a
  runnable rust-analyzer path (or nothing), honoring the pinned toolchain.
- **The warm pool** (existing): what makes a seconds-to-initialize rust-analyzer viable; this
  feature only feeds it a spec.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a Cargo workspace with a runnable rust-analyzer, detection produces exactly one
  Rust entry whose binary runs; in a workspace without one (no Cargo.toml, or no runnable
  rust-analyzer, or proxy-without-component), it produces none — pinned by unit tests over an
  injected resolver.
- **SC-002**: Over a real fixture Cargo crate (trait impl + generic fn + a macro-generated
  item), a warm references request returns `resolver-lsp`/`high` edges at the real sites;
  environment-gated on rustup + rust-analyzer, skipped with a message naming them.
- **SC-003**: With rust-analyzer absent, the references answer for the same fixture is
  byte-identical to the tree-sitter answer with no resolver configured (the pre-feature product
  preserved) — pinned by test.
- **SC-004**: Resolver-upgraded references latency on the warm pool over the fixture is
  recorded (Constitution VI); environment-gated.
- **SC-005**: No new dependency enters either lock file; the resolver interface, guest, and pool
  are untouched (git diff scoped to detection + tests + fixtures).

## Assumptions

- `chunk:backlog-resolver-server-pool` shipped: the warm pool, the additive `reused`/`release`
  contract, and the guest's handshake-skip are in place, so a slow rust-analyzer contributes on
  the second request. This feature adds only the detection half.
- The bundled guest answers `references` and returns `rename`/`diagnostics` empty for every
  language; Rust rename precision is filed as `chunk:resolver-rename-lane`, out of scope here.
- `workspace/symbol` by name picks the first exact-name match, so references resolve for an
  unambiguous symbol; disambiguating overloaded names is `chunk:rust-index-fidelity`'s
  AMBIGUOUS_SYMBOL work, not this feature's.
- rust-analyzer discovers its own sysroot/toolchain at runtime, so passing the toolchain's
  absolute binary to the pool (spawned directly, not via the toolchain manager) is correct.
- The machine shaping this feature has rustup + rust-analyzer (installed in unit 2);
  typescript-language-server is absent, so no cross-language detection number is claimed.
