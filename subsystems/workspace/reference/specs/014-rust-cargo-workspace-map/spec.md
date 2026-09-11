# Feature Specification: Cargo Workspace Map

**Feature Branch**: `014-rust-cargo-workspace-map`

**Created**: 2026-08-24

**Status**: Draft

**Input**: User description (chunk:rust-cargo-workspace-map): "Make map understand Cargo
workspaces: members, crate-level import graph (use + Cargo deps), features, and per-crate entry
points (lib.rs, main.rs, src/bin, examples, benches, tests)."

> Scope note: constitution Development Workflow rule 8 binds a feature spec to exactly one
> roadmap chunk. This spec covers `chunk:rust-cargo-workspace-map` only. It does NOT change
> `map`'s tree, outlines, plugin inventory, TTL or budget mechanics; it does NOT add project
> commands to `map` (the SDD §9.2 deferral to `run` stands); it does NOT resolve names — the
> `use` half of the graph is the index's existing import table aggregated by crate, never a
> claim that a path resolves; and it does NOT read external (registry) crates' manifests.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A monorepo answers "what crates are here and how do they relate" (Priority: P1)

An agent runs `map` on a Cargo workspace. Today it gets a directory tree, root-manifest entry
points and a specifier-level import summary that names things like `std::fmt::Debug`. After this
feature the answer carries one row per workspace member — its name, directory, roles (library,
binary, proc-macro, build script), edition, entry points, features and the members it depends
on — ordered by how many members depend on it, plus an overview summary of the workspace's
shape. The tree is still there below.

**Why this priority**: it is the chunk's purpose; in a 30-crate monorepo the crate list is the
orientation, and the tree alone hides it under `crates/`.

**Independent Test**: `map` over a fixture workspace (glob members, an excluded member, a root
package, a proc-macro crate, a crate with examples/benches/tests, `path`/`workspace = true`/
renamed dependencies) produces crate rows and a summary byte-equal to a checked-in golden.

**Acceptance Scenarios**:

1. **Given** a root `Cargo.toml` with `[workspace] members = ["crates/*"]`, **When** `map` runs,
   **Then** every member directory that holds a manifest is a `crate` row named from its
   `[package] name`, and an `exclude`d member is not.
2. **Given** a member with `src/lib.rs`, `src/main.rs`, `src/bin/x.rs`, `examples/e.rs`,
   `benches/b.rs`, `tests/t.rs` and `build.rs`, **When** `map` runs, **Then** its row lists
   those entry points with their kinds, and manifest `path =` targets override convention.
3. **Given** a member whose `[dependencies]` names another member (by `path`, by `workspace =
   true`, or by a `package =` rename), **When** `map` runs, **Then** the row lists that member
   as a dependency with its kind (normal, dev, build) and the target's `dependents` count rises.
4. **Given** a member with a `[features]` table, **When** `map` runs, **Then** its row lists
   the feature names with `default` first.
5. **Given** the rows, **When** `map` runs, **Then** they are ordered by dependents descending,
   then by name, and sit between the overview and the tree rows.
6. **Given** a `path` that scopes into one member, **When** `map` runs, **Then** only crate rows
   under that path appear while the overview summary still describes the whole workspace.

---

### User Story 2 - The crate graph says which dependencies are actually used (Priority: P2)

Declared dependencies are a manifest fact; whether the code uses them is an index fact. After
this feature, when the Tier-1 index is up, each member-to-member dependency carries the number
of importing files, the overview counts declared-but-unused member dependencies, and when the
index cannot answer the rows degrade to the manifest facts with a note — never an error.

**Why this priority**: the chunk names `use` beside Cargo deps; the number is what turns a
dependency list into a graph an agent can trust when deciding what a change will touch.

**Independent Test**: the fake index answers per-crate edges and the rows carry `files` counts
and the overview an `unusedDeps` count; a failing or silent index yields manifest-only rows
plus a note; the engine-side aggregation is pinned over real import rows.

**Acceptance Scenarios**:

1. **Given** the index knows files under `crates/beta` import `alpha::…`, **When** `map` runs,
   **Then** beta's dependency on alpha carries `files: <n>`.
2. **Given** a declared member dependency with no importing file, **When** the index answered,
   **Then** the overview's `unusedDeps` counts it.
3. **Given** the index query fails or the adapter records no edges, **When** `map` runs,
   **Then** the rows carry Cargo facts only and a note says the `use` counts are unavailable.
4. **Given** a member named `beta-crate`, **When** its files are imported as `beta_crate::…`,
   **Then** the count is attributed to it (Cargo's hyphen-to-underscore rule).

---

### User Story 3 - Big workspaces stay inside the budget, honestly (Priority: P2)

A 200-crate monorepo must not turn `map` into a dump. After this feature crate rows are a
ranked summary: at most a fixed number of rows (the most depended-upon first), the overview
says how many members were omitted and how to narrow, per-row lists are capped with counts,
and the budget cut keeps its truncation manifest and cursor.

**Independent Test**: a 64-member fixture at a small budget answers the overview, the leading
crate rows, a truncation manifest with a `path:` cursor, and an omitted-members count.

**Acceptance Scenarios**:

1. **Given** more members than the row cap, **When** `map` runs, **Then** the overview carries
   the omitted count and a `map path:<dir>` hint, and the tree still appears.
2. **Given** a budget smaller than the crate rows, **When** `map` runs, **Then** the manifest
   carries `dropped`, `reason: "budget"`, narrowing hints and a `path:` cursor.
3. **Given** a crate with more entry points or features than the per-row caps, **When** its row
   is built, **Then** the lists are cut and `moreEntryPoints`/`moreFeatures` say how many.

---

### User Story 4 - Non-Cargo workspaces are untouched (Priority: P1)

**Independent Test**: the existing map fixtures and every existing map test pass unchanged;
a workspace without a root `Cargo.toml` produces no `cargo` summary and no crate rows.

**Acceptance Scenarios**:

1. **Given** no `Cargo.toml` in the walk, **When** `map` runs, **Then** the envelope is
   byte-identical to today's.
2. **Given** a single crate (a root `Cargo.toml` without `[workspace]`), **When** `map` runs,
   **Then** the summary says `kind: "crate"` and one crate row describes it.

---

### Edge Cases

- A root manifest that is both a workspace and a package: the root package is a member with
  directory `.`.
- Member globs beyond `<dir>/*` (`**`, character classes): not expanded; named in a note.
- A member directory without a readable manifest: skipped (never a guess); malformed manifests
  detect nothing for that member.
- `[lib] name = "…"` overrides the `use` name; crate names with hyphens are matched as
  underscores.
- `[target.'cfg(…)'.dependencies]` tables: read like the plain tables (a dependency is a
  dependency for the graph's purpose).
- Dependencies on non-members (registry, git, or paths outside the workspace): counted as
  `externalDependencies`, never listed.
- More than 256 members: manifests beyond the cap are not read; the overview says so.
- A member whose directory is under another member's directory: files attribute to the
  longest matching member directory.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When the walk lists a root `Cargo.toml`, `map` MUST answer `crate` rows (one per
  member, including a root package) between the overview and the tree, and a `cargo` summary
  on the overview; without one, the envelope MUST be unchanged.
- **FR-002**: Members MUST be resolved from `[workspace] members`/`exclude` read textually,
  `<dir>/*` globs expanded against the walk, manifests read only when the walk listed them.
- **FR-003**: Each crate row MUST carry name, directory (`path`), roles, edition (when
  declared), entry points (manifest targets first, then `src/lib.rs`, `src/main.rs`,
  `src/bin/*`, `examples/*`, `benches/*`, `tests/*`, `build.rs` conventions, deduped), features
  (`default` first), member dependencies with kind, `dependents`, and `externalDependencies`.
- **FR-004**: Crate rows MUST be ordered by dependents descending then name, capped at a fixed
  maximum with the omitted count on the overview; per-row entry points and features MUST be
  capped with `more…` counts.
- **FR-005**: When the Tier-1 index is up, `map` MUST ask it for per-crate `use` edges through
  an ADDITIVE extension of the existing import-graph query (optional request field, optional
  result field; the port's major unchanged; every existing adapter and fake still conforms) and
  MUST attach `files` counts to member dependencies and `unusedDeps` to the summary; a failing
  or silent index MUST degrade to manifest facts with a note.
- **FR-006**: The engine-side aggregation MUST attribute an import row to the longest member
  directory prefixing its path and match targets equal to the crate's `use` name or starting
  with `<name>::`; the in-process adapter (which records no imports) MUST answer no edges.
- **FR-007**: Crate rows MUST follow the `path` scope like tree rows; the overview summary MUST
  describe the whole workspace.
- **FR-008**: The existing per-row budget cut, truncation manifest and `path:` cursor MUST cover
  crate rows unchanged.
- **FR-009**: No TOML parser and no new dependency; tools-layer code MUST NOT import the config
  layer (the import-graph rule), so the manifest helpers live beside `map`.
- **FR-010**: The `map` schema description MUST name crate rows; the routing skill MUST stay
  consistent (`lint:skill` decides).

### Key Entities

- **Crate row**: one workspace member as `map` reports it.
- **Cargo summary**: kind, member count, edge count, omitted count, unused-dependency count,
  and the top members by dependents, on the overview.
- **Crate edge**: importer member directory → imported crate name → importing-file count, from
  the index.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The fixture workspace's crate rows and summary equal the golden — members, roles,
  entry points, features, dependencies with kinds and `files`, dependents, ordering, exclusion,
  root package — pinned.
- **SC-002**: The `use` half: fake edges attach `files` and `unusedDeps`; a failing index and
  the in-process adapter degrade with a note; the engine-side aggregation groups real import
  rows by member directory and honors the underscore rule — pinned.
- **SC-003**: The 64-member fixture at a small budget answers the overview, leading rows, a
  manifest with a `path:` cursor and the omitted count; per-row caps carry `more…` counts —
  pinned.
- **SC-004**: Every existing map test passes unchanged; a non-Cargo workspace's envelope is
  byte-identical; a single crate answers `kind: "crate"` — pinned.
- **SC-005**: `PORT_VERSIONS.index` bumps additively; IPC frame tests and the engine-child
  round-trip carry the optional field; full gate sweep green.

## Assumptions

- Member manifests are small; reading up to 256 per call is within `map`'s Tier-0 cost
  envelope (the walk itself is larger).
- The index's import table stores Rust `use` targets as written (`alpha::add`), verified
  against `crates/workbench-core/src/indexer.rs` during shaping; nested `use a::{b, c}` groups
  record the group's prefix, which still starts with the crate name.
- Cargo's `use` name is the package name with hyphens replaced by underscores unless `[lib]
  name` says otherwise.
