# Feature Specification: Rust Index Fidelity

**Feature Branch**: `009-rust-index-fidelity`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description (chunk:rust-index-fidelity): "Teach the indexer Rust structure: impl methods qualified (Type::method, <Type as Trait>::method), associated consts/types, #[cfg(test)] modules as tests for ranking, doc-comment summaries in outlines, target/ always junk, macro-generated items honestly out of scope and said so."

> Scope note: constitution Development Workflow rule 8 binds a feature spec to exactly one
> roadmap chunk. This spec covers `chunk:rust-index-fidelity` only. It does NOT rewrite Rust
> module paths on file moves (`chunk:rust-move-aware-files`), does NOT map Cargo workspaces
> (`chunk:rust-cargo-workspace-map`), and does NOT touch the resolver lane (shipped in
> chunks `backlog-resolver-server-pool` / `rust-resolver`).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Rust methods are addressable the way Rust names them (Priority: P1)

An agent wants to read or edit a method on a type — `Config::load`, `English::hello`. Today
the index stores it under its bare name (`load`, `hello`), so the address `Config::load` does
not resolve, and a file with two `new` methods on different types answers "ambiguous" with
candidates that do not say which type each belongs to. After this feature, impl members are
stored qualified, the address dialect accepts Rust's `::` as well as `.`, and an ambiguous
address lists candidates that name the type — and the trait, when the method comes from a
trait implementation.

**Why this priority**: symbol addressing is how `read` and `edit` target code without line
numbers. For Rust it is currently unusable on the most common shape in the language (methods
in `impl` blocks), which makes the whole symbol-address dialect a TypeScript feature.

**Independent Test**: index a Rust file with an inherent impl, a trait impl, and associated
consts/types; read `Type::method` and `Type::ASSOC`; read a bare method name that exists on
two types and see qualified candidates.

**Acceptance Scenarios**:

1. **Given** a file with `impl Config { pub fn load() }`, **When** the agent reads symbol
   `Config::load`, **Then** the method's own extent is returned (not the whole impl block, not
   a miss).
2. **Given** a file where two types each have a `new`, **When** the agent reads symbol `new`,
   **Then** the answer is an ambiguity whose candidates name both types, so the next call can
   be exact.
3. **Given** a method that comes from `impl Greeter for English`, **When** it collides with an
   inherent method of the same name, **Then** the candidate list distinguishes them by trait.
4. **Given** the same file, **When** the agent searches symbols for the bare name `load`,
   **Then** the qualified symbol is still found (qualification must not hide symbols from
   search).
5. **Given** associated consts and types inside an impl, **When** they are indexed, **Then**
   they are qualified like methods and keep their own kinds.

---

### User Story 2 - Test code ranks like test code, in Rust's idiom (Priority: P1)

Rust puts unit tests *inside* the source file, in a `#[cfg(test)] mod tests` block, and puts
other non-shipping code under `benches/` and `examples/`. Today a hit inside a `#[cfg(test)]`
module ranks exactly like production code in the same file, so searching a common helper name
surfaces its test call sites alongside its definition. After this feature, hits inside a
`#[cfg(test)]` module rank as test-class, and `benches/`/`examples/` join the test-path rules.

**Why this priority**: ranking is the product (Constitution II), and Rust's inline test
convention defeats every per-file rule the ranker has. This is the single largest Rust ranking
gap and the one the eval corpus can measure.

**Independent Test**: search a symbol used in both production and `#[cfg(test)]` code in the
same file; the production hit outranks the test hit. Run the eval harness and compare
MRR/Hit@1/Hit@5 before and after.

**Acceptance Scenarios**:

1. **Given** a source file with a `#[cfg(test)] mod tests` block, **When** a symbol is hit both
   above and inside that block, **Then** the hit inside ranks as test-class and the production
   hit ranks higher.
2. **Given** a file under `benches/` or `examples/`, **When** it is classified, **Then** it is
   test-class, like `tests/` already is.
3. **Given** anything under `target/`, **When** it is scanned, **Then** it is junk-class
   (already true today — this feature verifies it rather than changing it).
4. **Given** the ranking eval corpus extended with Rust queries, **When** the harness runs,
   **Then** the recorded MRR/Hit@1/Hit@5 are reported before and after this change.

---

### User Story 3 - Outlines say what an item is for (Priority: P2)

A Rust codebase documents itself with `///` doc comments. An outline that lists twenty
function names without their one-line summaries makes the agent open each one. After this
feature, an indexed item carries the first line of its doc comment, and outlines surface it.

**Why this priority**: it is the cheapest possible ranking-free precision win — the text is
already in the file and the outline is already being built — and it directly reduces the
read-everything loop that the whole product exists to avoid.

**Independent Test**: index a file whose items carry `///` docs; request the outline; each
documented item shows its summary and undocumented items show none.

**Acceptance Scenarios**:

1. **Given** an item with a `///` (or `/** */`) doc comment, **When** its outline row is
   returned, **Then** the row carries the first line of that comment.
2. **Given** an item with no doc comment, **When** its outline row is returned, **Then** no
   summary is present (absent, not an empty string).
3. **Given** a doc comment spanning several lines, **When** the summary is stored, **Then** it
   is the first line only, bounded in length.

---

### User Story 4 - The index says what it cannot see (Priority: P2)

Derive macros and `macro_rules!` generate items that no tree-sitter parse can see. An agent
that believes the symbol index is complete will conclude a generated item does not exist.
After this feature, the workspace map's index health says plainly that macro-generated items
are not indexed, so the agent knows to fall back to text search or the resolver for them.

**Why this priority**: Constitution I's "no stale line number leaves the server" has a twin —
no silent incompleteness. The cost is one honest note; the cost of omitting it is an agent
that trusts a wrong answer.

**Independent Test**: request the workspace map for a Rust workspace and read the note.

**Acceptance Scenarios**:

1. **Given** a workspace containing Rust files, **When** the map overview is requested,
   **Then** its notes state that macro-generated items are not in the symbol index.
2. **Given** a workspace with no Rust files, **When** the map overview is requested, **Then**
   the note is absent (no noise for projects it does not apply to).

---

### Edge Cases

- A method whose type is generic (`impl<T> Stack<T>`): qualified by the type's base name, not
  its parameters, so the address stays typeable.
- Nested modules containing impls: qualification names the type, not the full module path —
  module-path addressing is a separate concern and stays out.
- Two trait impls of the same trait for different types: distinct qualifications, no collision.
- The same `Type::method` appearing twice in one file (inherent plus trait impl): a real
  ambiguity, answered as one, with the trait distinguishing the candidates.
- A `#[cfg(test)]` module nested inside another module: still test-class.
- `#[cfg(feature = "x")]` and other cfg attributes: NOT test-class — only `cfg(test)`.
- A doc comment on an item inside a `#[cfg(test)]` module: indexed like any other.
- Rebuilding the index after this change: the index is cache-class, so an old index is
  discarded and rebuilt rather than misread.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Members declared inside a Rust `impl` block — methods, associated consts,
  associated types — MUST be indexed under a qualified name that names the implementing type,
  keeping their own kinds.
- **FR-002**: When a member comes from a trait implementation, the index MUST retain which
  trait, and any ambiguity answer MUST use it to distinguish otherwise identical candidates.
- **FR-003**: The symbol-address dialect MUST accept Rust's `::` separator wherever it accepts
  `.`, for both reading and editing, without changing existing dotted behavior.
- **FR-004**: Qualification MUST NOT hide a symbol from a bare-name symbol search.
- **FR-005**: A hit inside a `#[cfg(test)]` module MUST be classified as test-class for
  ranking even when its file is otherwise source-class.
- **FR-006**: `benches/` and `examples/` path segments MUST classify as test-class; `target/`
  MUST remain junk-class.
- **FR-007**: An indexed item MUST be able to carry a one-line documentation summary taken
  from its doc comment, and outlines MUST surface it when present.
- **FR-008**: Any index storage change MUST be handled as cache-class data: the old index is
  discarded and rebuilt, never misread, with no user-visible migration step.
- **FR-009**: The workspace map's index health MUST state, for workspaces containing Rust,
  that macro-generated items are not indexed.
- **FR-010**: The ranking evaluation corpus MUST gain at least eight Rust queries exercising
  the behaviors above, and the change MUST be merged only with before/after MRR/Hit@1/Hit@5
  recorded (Constitution VI).
- **FR-011**: Non-Rust languages MUST be unaffected: their extraction, classification,
  outlines, and eval numbers are unchanged by this feature.

### Key Entities

- **Qualified symbol**: an indexed item whose name carries its implementing type
  (`Type::member`), plus the trait it came from when it came from one.
- **Test range**: a region of a source file (a `#[cfg(test)]` module) whose hits rank as test
  code although the file is source.
- **Doc summary**: the first line of an item's doc comment, stored with the item.
- **Index health note**: the honest statement of what the symbol index does not contain.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a fixture with inherent impls, a trait impl, associated consts/types and
  duplicate bare names, every qualified address resolves to its own extent, and every genuine
  ambiguity answers with candidates that name the type (and trait where applicable) — pinned
  by test.
- **SC-002**: A bare-name symbol search still finds every qualified symbol — pinned by test.
- **SC-003**: A hit inside a `#[cfg(test)]` module ranks below the same symbol's production
  hit in the same file — pinned by test.
- **SC-004**: The eval corpus contains ≥8 Rust queries; before/after MRR, Hit@1 and Hit@5 are
  recorded in the ship trace, and no non-Rust query regresses.
- **SC-005**: Outline rows carry doc summaries for documented items and none for undocumented
  ones — pinned by test.
- **SC-006**: The map overview's notes state the macro limitation for a Rust workspace and omit
  it for a workspace without Rust — pinned by test.
- **SC-007**: The full gate sweep stays green, and an index built by the previous version is
  rebuilt rather than misread — pinned by test.

## Assumptions

- The bundled tree-sitter Rust grammar exposes impl blocks with their type and (for trait
  impls) their trait, and exposes doc comments as sibling comment nodes; no new grammar or
  dependency is needed.
- The symbol index is cache-class (rebuildable), so its storage may change without a migration
  path, per the project's data-class rules.
- Module-path qualification (`crate::config::Config::load`) is deliberately out of scope: the
  address dialect is file-scoped today, and widening it is a separate decision.
- Macro expansion is out of scope permanently for the tree-sitter tier; the resolver lane is
  where macro-generated items become visible, and that is already shipped for references.
- The ranking weights themselves are not retuned here; this feature changes which class a hit
  belongs to, and the eval numbers report the effect.
