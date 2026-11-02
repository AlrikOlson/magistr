# Rust structural patterns


`mode:structural` and `op:rewrite` run ast-grep over tree-sitter-rust. Three facts decide whether a
Rust pattern hits (measured on the native core):

1. **Inside a macro's token tree `$X` matches one token.** `assert_eq!($A, $B)` matches
   `assert_eq!(v, 3)` but not `assert_eq!(1 + 1, 2)`; `dbg!($X)` misses `dbg!(&v)`. Use `$$$X` for
   an expression argument: `assert_eq!($$$A)`, `dbg!($$$X)`, `format!($FMT, $$$REST)`.
2. **Visibility, generics, `where` clauses and `async` are part of the shape.** A pattern must spell
   them to match items that carry them, and then matches only those: `impl $T for $U { $$$B }` never
   matches `impl<'a, T> Greeter for Config<'a, T>` (write `impl<$$$G> $T for $U { $$$B }`);
   `fn $F($$$P) -> $R { $$$B }` never matches a `pub fn`, a generic fn or an `async fn`.
3. **Attributes match on their own.** `#[derive($$$D)]`, `#[$A]`, `#[should_panic]` each match an
   `attribute_item`; joining the attribute to its item in one pattern (`#[test] fn $F() { $$$B }`)
   is rejected as multiple AST nodes — search the attribute, then the item.

| Works | Matches | Kind |
|---|---|---|
| `$M!($$$ARGS)`, `println!($$$ARGS)`, `vec![$$$ITEMS]`, `todo!()` | macro invocations | `macro_invocation` |
| `#[derive($$$D)]`, `#[$A]`, `#[should_panic]` | attributes | `attribute_item` |
| `impl<$$$G> $T for $U { $$$BODY }`, `impl $T { $$$BODY }`, header-only `impl $T` (loose) | impl blocks | `impl_item` |
| `pub trait $T { $$$B }`, `trait $T { $$$B }` | traits | `trait_item` |
| `fn $F($$$P) -> $R { $$$B }`, `pub async fn $F($$$P) -> $R { $$$B }`, `fn $F<$$$G>($$$P) -> $R where $$$W { $$$B }` | functions | `function_item` |
| `fn $F(&self) -> $R;`, parameter-only `fn $F($$$P)` | trait method signatures | `function_signature_item` |
| `pub struct $S<$$$G> { $$$F }`, `enum $E { $$$V }`, `const $C: $T = $V;`, `static …`, `type $A = $T;`, `mod $M;` | items | `struct_item`, `enum_item`, `const_item`, `static_item`, `type_item`, `mod_item` |
| `use $P;`, `use $A::{$$$ITEMS};` | imports | `use_declaration` |
| `$X.unwrap()`, `$RECV.$METHOD($$$ARGS)`, `Box::new($X)`, `$A::$B($$$ARGS)`, `Some($X)`, `$X.iter().map($F).collect()` | function AND method calls | `call_expression` |
| `$X?` | try | `try_expression` |
| `let $V = $E;`, `let $V: $T = $E;`, `let [$$$P] = $E;` | bindings (`let _ = …` is skipped: `_` is not an identifier node) | `let_declaration` |
| `match $E { $$$ARMS }`, `if let $P = $E { $$$B }`, `if $C { $$$T } else { $$$E }`, `for $I in $IT { $$$B }`, `while $C { $$$B }`, `loop { $$$B }` | control flow | `match_expression`, `if_expression`, `for_expression`, `while_expression`, `loop_expression` |
| `unsafe { $$$B }`, `async { $$$B }`, `$E.await`, `\|$$$P\| $B` | blocks, await, closures | `unsafe_block`, `async_block`, `await_expression`, `closure_expression` |
| `$E as $T`, `&mut $X`, `&$X`, `$S { $$$F }`, `$A + $B`, `$E == $V` | casts, references, struct literals, operators | `type_cast_expression`, `reference_expression`, `struct_expression`, `binary_expression` |

| Never matches | Why |
|---|---|
| `pub fn $F($$$P)`, `fn $F($$$P) -> $R` | a fn header without its body or `;` — add `{ $$$B }` (or `;` for a signature) |
| `&'static str`, `&'$L $T` | a type in type position is not a node ast-grep can root a pattern at — match the `let` or `fn` that carries it |
| `#[derive($$$D)] struct $S { $$$F }` | two AST nodes — rejected with `PATTERN_INVALID`; search the attribute and the item separately |
| `impl $T for $U { $$$B }` on a generic impl, `trait $T { … }` on a `pub trait` | fact 2: spell the modifiers |

The `kind` filter takes these node kinds; the shorthand is substring: `function` keeps `function_item`
and `function_signature_item`, `call` keeps `call_expression` (method calls included), `macro` keeps
`macro_invocation`, `impl` / `trait` / `struct` / `enum` keep their `*_item`, `attribute` keeps
`attribute_item`, `item` keeps every `*_item`. Rewrites carry `$VAR` and `$$$VARS` through macro
token trees and attribute lists (`#[derive($$$D)]` → `#[derive($$$D, Default)]`,
`println!($$$ARGS)` → `log::info!($$$ARGS)`), and the parse-verify guard still holds.
