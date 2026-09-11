# Feature Specification: Rust Structural Search

**Feature Branch**: `015-rust-structural-search`

**Created**: 2026-08-24

**Status**: Draft

**Input**: User description (chunk:rust-structural-search): "Verify and document Rust structural
search and rewrite: ast-grep patterns over macro invocations, attributes, $$$ARGS, impl/trait
kinds, with a fixture corpus, a documented kinds table, and Rust rows in the routing skill."

> Scope note: constitution Development Workflow rule 8 binds a feature spec to exactly one
> roadmap chunk. This spec covers `chunk:rust-structural-search` only. It MEASURES and DOCUMENTS;
> it does NOT change the native core, the search or edit tools, the envelope, or the pattern
> language — every limit it records is ast-grep / tree-sitter-rust behaviour, written down where
> agents read it so they stop rediscovering it by getting zero hits.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An agent's Rust pattern hits what it should, and the suite says so (Priority: P1)

An agent searches a Rust workspace with `search mode:structural` for a macro invocation, an
attribute, an impl block, a trait, a function shape, a `?`, a `.unwrap()`, an `.await`, a
closure, a `match`. Today nothing pins that any of these work over Rust through the real
core. After this feature a fixture-corpus suite runs every such pattern through the native
core and asserts the matched node kinds and lines, and rewrites of the same shapes assert
their output — so a regression in the core, the grammar, or ast-grep is caught here, not in
an agent's session.

**Why this priority**: it is the chunk's purpose; an unpinned capability is a rumor.

**Independent Test**: `bun test test/rust-structural.test.ts` runs the corpus through
`loadCore()` and every expected (pattern → kinds@lines) row holds.

**Acceptance Scenarios**:

1. **Given** the corpus, **When** `println!($$$ARGS)`, `$M!($$$ARGS)`, `vec![$$$ITEMS]`,
   `format!($FMT, $$$REST)`, `todo!()` run, **Then** each matches its `macro_invocation` nodes
   at the expected lines.
2. **Given** the corpus, **When** `#[derive($$$D)]`, `#[$A]`, `#[should_panic]` run, **Then**
   each matches its `attribute_item` nodes.
3. **Given** the corpus, **When** `impl $T for $U { $$$BODY }`, `impl<$$$G> $T for $U {
   $$$BODY }`, `trait $T { $$$B }`, `pub trait $T { $$$B }`, `struct $S { $$$F }`, `enum $E {
   $$$V }`, `fn $F($$$P) -> $R { $$$B }`, `pub async fn …`, `fn $F<$$$G>(…) where $$$W {…}`,
   `fn $F(&self) -> $R;`, const/static/type/mod/use items run, **Then** each matches its
   `*_item` / `use_declaration` / `function_signature_item` nodes.
4. **Given** the corpus, **When** expression patterns run (`$X.unwrap()`, `$X?`, `let $V =
   $E;`, `match $E { $$$ARMS }`, `unsafe { $$$B }`, `if let $P = $E { $$$B }`, `for`/`while`/
   `loop`, `Box::new($X)`, `$A::$B($$$ARGS)`, `$E.await`, `async { $$$B }`, `|$$$P| $B`,
   `$X.iter().map($F).collect()`, `$E as $T`, `&mut $X`, `$S { $$$F }`, `$A + $B`,
   `$RECV.$METHOD($$$ARGS)`), **Then** each matches its expression kind — and method calls are
   `call_expression`, there being no `method_call_expression` kind in tree-sitter-rust.
5. **Given** the corpus, **When** rewrites run (`$X.unwrap()` → `$X.expect("checked")`,
   `println!($$$ARGS)` → `log::info!($$$ARGS)`, `#[derive($$$D)]` → `#[derive($$$D, Default)]`,
   `$X?` → `$X.unwrap()`, `impl $T for $U { $$$BODY }` → a body with an added method,
   `assert_eq!($$$A)` → `assert_eq!($$$A, "ctx")`), **Then** the output equals the expected
   text and the match counts hold.
6. **Given** a `.rs` path, **When** the engine child's structural request runs, **Then** it
   dispatches to the Rust grammar (a path with an unknown extension answers no hits).

---

### User Story 2 - The rules that produce zero hits are written down (Priority: P1)

Three ast-grep facts cost an agent a session each when discovered by surprise. After this
feature they are pinned as negatives and documented beside the positives: (a) inside a
macro's token tree a `$X` metavariable matches ONE token — `assert_eq!($A, $B)` matches
`assert_eq!(v, 3)` but not `assert_eq!(1 + 1, 2)`, `dbg!($X)` matches `dbg!(v)` but not
`dbg!(&v)`; `$$$X` matches the multi-token argument; (b) visibility, generics, `where`
clauses and `async` are structural — `impl $T for $U {…}` never matches `impl<'a, T> … for
… <'a, T>`, `fn $F($$$P) -> $R {…}` never matches a `pub fn`, a generic fn or an `async fn`;
(c) partial items (`pub fn $F($$$P)`, `impl $T for $U`) and type-position patterns
(`&'$L $T`, `&'static str`) match nothing, and an attribute joined to its item in one pattern
is rejected as multiple AST nodes.

**Independent Test**: the suite's negative table: each pattern, its zero-hit corpus line, and
the rule it demonstrates; the `PATTERN_INVALID` path for the multi-node pattern through the
search tool's error mapping.

**Acceptance Scenarios**:

1. **Given** `assert_eq!(1 + 1, 2)`, **When** `assert_eq!($A, $B)` runs, **Then** zero hits;
   **When** `assert_eq!($$$A)` runs, **Then** one hit.
2. **Given** `impl<'a, T: Debug> Greeter for Config<'a, T>`, **When** `impl $T for $U {
   $$$BODY }` runs, **Then** zero hits; **When** `impl<$$$G> $T for $U { $$$BODY }` runs,
   **Then** one hit.
3. **Given** `pub fn new(…)`, **When** `fn $F($$$P) -> $R { $$$B }` runs, **Then** it is not
   among the hits; **When** `pub fn $F($$$P) -> $R { $$$B }` runs, **Then** it is.
4. **Given** `#[derive($$$D)] struct $S { $$$F }`, **When** it runs, **Then** the core rejects
   it as multiple AST nodes and the search tool answers `PATTERN_INVALID`.

---

### User Story 3 - The routing skill tells agents how to search Rust structurally (Priority: P2)

The routing skill has one generic structural row and no Rust guidance. After this feature it
carries Rust rows in the routing table, a "Rust structural patterns" section with the
works / needs-`$$$` / never-matches table, and the Rust node-kind vocabulary the `kind` filter
accepts (with the shorthand rule: `function` ⇒ `function_item` and `function_signature_item`,
`call` ⇒ `call_expression` including method calls, `macro` ⇒ `macro_invocation`, `impl` /
`trait` / `struct` / `enum` ⇒ their `*_item`, `attribute` ⇒ `attribute_item`).

**Independent Test**: `bun run lint:skill` exits 0 with the new rows; the skill drift test
passes; both skill copies are byte-equal.

**Acceptance Scenarios**:

1. **Given** the routing table, **When** an agent looks for "every macro call" or "every
   impl of a trait" in Rust, **Then** a row names `search` `mode:structural` with a Rust
   pattern and the arguments that matter.
2. **Given** the kinds section, **When** an agent filters by `kind`, **Then** the listed
   values are exactly the kinds the suite observed.
3. **Given** `lint:skill`, **When** it runs, **Then** exit 0.

---

### Edge Cases

- A bare `fn $F` pattern matches every function loosely (ast-grep tolerates the partial
  item); documented as "works, but loose", not recommended.
- An exact attribute pattern (`#[derive(Debug)]`) matches only that exact derive list.
- The `kind` filter's `includes` shorthand means `item` matches every `*_item` kind; the
  section says so.
- The search tool's structural mode is per file through the engine child; the suite pins the
  core and the child's dispatch, not the tool's grouping (already pinned over a fake engine).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A test suite MUST run a Rust fixture corpus through the real native core's
  structural search and rewrite and assert, per pattern, the matched node kinds and lines (or
  the rewritten output and match count).
- **FR-002**: The suite MUST pin the negatives with their rules: macro token-tree
  metavariables, structural modifiers (visibility, generics, where, async), partial items,
  type-position patterns, attribute-plus-item patterns (rejected).
- **FR-003**: The suite MUST pin the engine child's dispatch of a `.rs` path to the Rust
  grammar and the search tool's `PATTERN_INVALID` answer for a multi-node pattern.
- **FR-004**: The routing skill MUST gain Rust rows in the routing table and a Rust
  structural section with the works / needs-`$$$` / never-matches table and the kinds
  vocabulary; both copies MUST stay byte-equal; `lint:skill` MUST exit 0.
- **FR-005**: The search schema's `kind` description MUST say that `call_expression` covers
  Rust method calls; codegen regenerated.
- **FR-006**: No source under `src/` or `crates/` changes behaviour; the chunk measures and
  documents.

### Key Entities

- **Corpus**: two Rust sources (items-and-generics; plain-items-and-expressions) inline in
  the suite, line-numbered by construction.
- **Pattern row**: pattern, expected kinds@lines (or zero + rule), category.
- **Rewrite row**: pattern, replacement, expected match count, expected changed lines.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every positive pattern in the corpus table matches its expected kinds@lines
  through the real core; every rewrite row yields its expected output — pinned.
- **SC-002**: Every negative row yields zero hits (or the typed rejection) for the stated
  reason, with the `$$$` / modifier-spelled counterpart matching — pinned.
- **SC-003**: The engine child dispatches `.rs` to Rust; an unknown extension answers no
  hits; the search tool maps the multi-node rejection to `PATTERN_INVALID` — pinned.
- **SC-004**: `lint:skill` exit 0 with the Rust rows and section; skill copies byte-equal;
  the kinds listed are exactly those the suite observed.
- **SC-005**: Full gate sweep green; no behaviour change under `src/` or `crates/`.

## Assumptions

- The native core artifact is present wherever the suite runs (the core-loader and lang-pack
  suites already require it); the suite does not add a skip.
- ast-grep-core 0.45 and tree-sitter-rust 0.24 as pinned in Cargo.lock; the table is
  measured against them and says so.
