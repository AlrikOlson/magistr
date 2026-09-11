# Phase 0 Research: Rust Structural Search

Grounded in `crates/workbench-core/src/engine.rs` (`structural_search` = `Pattern::try_new`
then `find_all`, hits carry `kind = node.kind()`; `rewrite` = `replace_all` with metavariables
carried; an unparseable pattern is `BadStructuralPattern`), `langs.rs` (Rust bundled,
tree-sitter-rust 0.24), `src/adapters/engine-ipc/engine-child.ts` (`handleStructural`:
`languageForPath` then the binding; unknown extension ⇒ no hits; `handleRewrite` structural iff
bundled), `src/adapters/engine-ipc/indexer.ts` (`rs: "rust"`), `src/tools/search.ts` (kind
filter `===` or `includes`; `PATTERN_INVALID` on "invalid structural pattern"),
`schemas/tools/search.input.schema.json`, the routing skill and `scripts/check-skill-drift.ts`
(one tool per row; facets are enum args; args must exist; required sections), and two live
probes through `loadCore().binding` over Rust sources (100+ patterns, 12 rewrites). Pinned in
think:180/181.

## R1 — The proof is a corpus suite over the real core, inline sources

**Decision**: `test/rust-structural.test.ts` loads the native core (`loadCore()`, as the
core-loader and lang-pack suites do, no skip) and runs table-driven cases over two inline Rust
sources: positives `{ pattern, expect: ["kind@line", …] }`, negatives `{ pattern, hits: 0,
rule }` each paired with the counterpart that matches, rewrites `{ pattern, replacement,
matchCount, changedLines }`. Inline sources keep each expected line visible beside its
pattern.

**Rationale**: the probes ARE the evidence; a suite is the probe made permanent. Fixture files
would hide the line numbers the assertions depend on.

## R2 — The rules, as measured

**Findings (ast-grep-core 0.45.1, tree-sitter-rust 0.24.2, native core)**:

1. **Macro token trees**: inside `name!(…)` a `$X` matches one token; `$$$X` matches a run
   of tokens. `assert_eq!($A, $B)` ⇒ `assert_eq!(v, 3)` yes, `assert_eq!(1 + 1, 2)` no;
   `assert_eq!($$$A)` ⇒ both; `dbg!($X)` ⇒ `dbg!(v)` yes, `dbg!(&v)` no; `format!($FMT,
   $$$REST)` yes; `assert!($C, $$$M)` ⇒ `assert!(w > 0, …)` no.
2. **Modifiers are structural**: visibility (`pub`), generics (`<…>`), `where` clauses and
   `async` must be spelled in the pattern to match items carrying them, and a pattern that
   spells them never matches items lacking them. `impl $T for $U {…}` matches a plain impl and
   not a generic one; `impl<$$$G> $T for $U {…}` the reverse; `fn $F($$$P) -> $R { $$$B }`
   matches neither `pub fn`, generic nor `async` fns; `fn $F<$$$G>($$$P) -> $R where $$$W {
   $$$B }` matches the generic fn with a where clause.
3. **Partial items**: `pub fn $F($$$P)`, `impl $T for $U` ⇒ 0; bare `fn $F` ⇒ every fn (loose;
   documented, not recommended).
4. **Type-position patterns**: `&'$L $T`, `&'static str` ⇒ 0 (a type is not a statement or
   expression node ast-grep can root a pattern at).
5. **Attribute + item in one pattern** (`#[derive($$$D)] struct $S {…}`, `#[test] fn $F()
   {…}`, `#[cfg(test)] mod $M {…}`) ⇒ rejected: "Multiple AST nodes are detected". Attributes
   match on their own (`#[derive($$$D)]`, `#[$A]`, `#[should_panic]`, exact `#[derive(Debug)]`).
6. **Kinds observed**: `macro_invocation`, `attribute_item`, `impl_item`, `trait_item`,
   `function_item`, `function_signature_item`, `struct_item`, `enum_item`, `const_item`,
   `static_item`, `type_item`, `mod_item`, `use_declaration`, `call_expression` (function AND
   method calls — no `method_call_expression`), `try_expression`, `let_declaration`,
   `match_expression`, `if_expression`, `for_expression`, `while_expression`,
   `loop_expression`, `unsafe_block`, `async_block`, `await_expression`, `closure_expression`,
   `type_cast_expression`, `reference_expression`, `struct_expression`, `binary_expression`,
   `expression_statement`.
7. **Rewrites**: metavariables and `$$$` carry through, including inside macro token trees
   and attribute argument lists; multi-line replacements keep the surrounding text; a pattern
   with zero matches leaves the source unchanged with count 0.

## R3 — Where the documentation lives

**Decision**: the routing skill (`skills/workbench-routing/SKILL.md` and its byte-equal
`.claude/skills/` copy): two Rust routing rows — "every macro invocation / attribute in Rust"
and "every impl / trait / fn shape in Rust" — both `search` `mode:structural` with a pattern
example and `query`, `kind`, `path`; a new `## Rust structural patterns` section (after
`## Search: …`) with three tables (works — pattern → kind; needs `$$$` — macro arguments;
never matches — partial items, type positions, attribute+item, modifier mismatches) and the
kinds vocabulary with the shorthand rule. The rewrite bullet gains one sentence on Rust
(`$$$` carries macro arguments; parse-verify still guards). `lint:skill` rules: one tool per
row (no backticked tool-like words elsewhere in the row), facet values are enum args, args
exist, required sections present.

**Alternatives considered**: a docs/ page (rejected: agents read the skill, and lint:skill
keeps only the skill honest); the search schema description (rejected for the table — the
schema is language-agnostic; it gains one clause, R4).

## R4 — The schema's `kind` description

**Decision**: append "In Rust, method calls are call_expression too; item kinds end in
_item (function_item, impl_item, trait_item, struct_item, enum_item); macros are
macro_invocation." to `kind`'s description; `bun run codegen`.

## R5 — Dispatch and error mapping pins

**Decision**: `handleRequest(binding, "structural", { path: "x.rs", source, pattern })`
answers Rust hits; the same with `path: "x.unknown"` answers `hits: []`; through the search
tool with a fake engine that throws the core's "invalid structural pattern … Multiple AST
nodes" message, the envelope is `PATTERN_INVALID` (the existing mapping, pinned for the Rust
case).

## R6 — Nothing in the core changes

**Decision**: every limit measured is ast-grep / tree-sitter behaviour; the chunk documents
it. No issue is filed against the core; a future chunk could add attribute-aware item
patterns only by changing ast-grep itself, which is out of this project's hands.
