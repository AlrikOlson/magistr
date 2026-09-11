# Phase 1 Data Model: Rust Structural Search

No stored data, port, schema-shape or source behaviour changes. Test vocabulary and skill
content only.

## Suite tables (test/rust-structural.test.ts)

```
PositiveCase
  pattern   string
  expect    string[]        "kind@line" per hit, in hit order (line = 1-based in the corpus)
  note?     string          why this pattern is in the table

NegativeCase
  pattern   string
  rule      "macro-token" | "modifiers" | "partial-item" | "type-position"
  becauseOf string          the corpus line it should have matched
  instead   string          the counterpart pattern that matches
  instead_expect string[]  its hits

RejectedCase
  pattern   string          rejected by the core ("Multiple AST nodes")

RewriteCase
  pattern      string
  replacement  string
  matchCount   number
  changedLines string[]     the lines that differ from the corpus after the rewrite
```

## Skill section shape (skills/workbench-routing/SKILL.md)

```
## Rust structural patterns
  paragraph: three facts (macro tokens, modifiers, attributes-alone)
  table "Works": | Pattern | Matches | Kind |
  table "Needs $$$": | Pattern | Misses | Use instead |
  table "Never matches": | Pattern | Why |
  paragraph: kinds vocabulary + shorthand rule
```

## Routing rows added

```
| every macro invocation / attribute in Rust (`println!`, `#[derive]`) | `search` `mode:structural` — `$M!($$$ARGS)`, `#[derive($$$D)]`; inside a macro `$X` is one token, use `$$$` | `query`, `kind`, `path` |
| every impl / trait / fn shape in Rust                                | `search` `mode:structural` — `impl<$$$G> $T for $U { $$$B }`; visibility, generics, where and async are part of the shape | `query`, `kind`, `path` |
```
