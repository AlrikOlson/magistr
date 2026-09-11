/**
 * Rust structural search and rewrite through the REAL native core
 * (chunk:rust-structural-search, specs/015). ast-grep over tree-sitter-rust
 * already works; this suite is what makes that a measured fact instead of a
 * rumor — and, just as load-bearing, pins the rules that produce ZERO hits
 * so an agent stops rediscovering them:
 *
 *   1. Inside a macro's token tree `$X` matches ONE token; `$$$X` matches
 *      the whole argument. `assert_eq!($A, $B)` misses `assert_eq!(1 + 1, 2)`.
 *   2. Visibility, generics, `where` clauses and `async` are part of the
 *      shape: a pattern must spell them to match items that carry them, and
 *      then matches only those.
 *   3. Partial items and type-position patterns match nothing; an attribute
 *      joined to its item in one pattern is rejected as multiple AST nodes.
 *   4. Method calls are `call_expression` — tree-sitter-rust has no
 *      `method_call_expression`.
 *
 * The corpus sources are inline so every expected line sits beside its
 * pattern. Line numbers are 1-based and are the assertions. The routing
 * skill's "Rust structural patterns" section is the human-readable copy of
 * these tables; lint:skill keeps its rows honest, this suite keeps them true.
 */

import { describe, expect, test } from "bun:test";
import { loadCore } from "../src/adapters/core";
import { handleRequest } from "../src/adapters/engine-ipc";

const { binding } = loadCore({ force: "native" });

/** Items, generics, lifetimes, an async method, macros of every shape. */
const ITEMS = [
  /*  1 */ "//! crate docs",
  /*  2 */ "use std::fmt::Debug;",
  /*  3 */ "use crate::util::{helper, other as alias};",
  /*  4 */ "",
  /*  5 */ "#[derive(Debug, Clone, PartialEq)]",
  /*  6 */ "pub struct Config<'a, T: Debug> {",
  /*  7 */ "    name: &'a str,",
  /*  8 */ "    value: T,",
  /*  9 */ "}",
  /* 10 */ "",
  /* 11 */ "#[derive(Debug)]",
  /* 12 */ "enum Mode { Fast, Slow { level: u8 } }",
  /* 13 */ "",
  /* 14 */ "pub trait Greeter {",
  /* 15 */ "    fn hello(&self) -> String;",
  /* 16 */ "    fn shout(&self) -> String { self.hello().to_uppercase() }",
  /* 17 */ "}",
  /* 18 */ "",
  /* 19 */ "impl<'a, T: Debug> Greeter for Config<'a, T> {",
  /* 20 */ '    fn hello(&self) -> String { format!("hi {}", self.name) }',
  /* 21 */ "}",
  /* 22 */ "",
  /* 23 */ "impl<'a, T: Debug> Config<'a, T> {",
  /* 24 */ "    pub fn new(name: &'a str, value: T) -> Self { Config { name, value } }",
  /* 25 */ "    pub async fn load(path: &str) -> Result<Self, std::io::Error> {",
  /* 26 */ "        let text = std::fs::read_to_string(path)?;",
  /* 27 */ "        let parsed = text.parse::<u8>().unwrap();",
  /* 28 */ "        let boxed = Box::new(parsed);",
  /* 29 */ "        let v: Vec<u8> = vec![1, 2, 3];",
  /* 30 */ "        let doubled: Vec<u8> = v.iter().map(|x| x * 2).collect();",
  /* 31 */ '        println!("loaded {} {:?}", path, doubled);',
  /* 32 */ '        eprintln!("done");',
  /* 33 */ "        dbg!(&boxed);",
  /* 34 */ '        if let Some(first) = v.first() { println!("{}", first); }',
  /* 35 */ "        for item in &v { let _ = item; }",
  /* 36 */ "        match parsed { 0 => {}, _ => {} }",
  /* 37 */ "        unsafe { std::ptr::null::<u8>(); }",
  /* 38 */ "        let fut = async { 1 };",
  /* 39 */ "        let _n = fut.await;",
  /* 40 */ "        let casted = parsed as u32;",
  /* 41 */ "        let r = &mut parsed.clone();",
  /* 42 */ "        let _ = (casted, r, alias(), helper());",
  /* 43 */ "        todo!()",
  /* 44 */ "    }",
  /* 45 */ "}",
  /* 46 */ "",
  /* 47 */ "fn generic<T: Clone, U>(a: T, b: U) -> T where U: Debug { a.clone() }",
  /* 48 */ "",
  /* 49 */ "const LIMIT: usize = 10;",
  /* 50 */ "static COUNTER: u8 = 0;",
  /* 51 */ "type Alias = Vec<u8>;",
  /* 52 */ "mod inner;",
  /* 53 */ "",
  /* 54 */ "#[cfg(test)]",
  /* 55 */ "mod tests {",
  /* 56 */ "    use super::*;",
  /* 57 */ "    #[test]",
  /* 58 */ "    fn works() { assert_eq!(1 + 1, 2); assert!(true); }",
  /* 59 */ "    #[test]",
  /* 60 */ "    #[should_panic]",
  /* 61 */ '    fn boom() { panic!("boom {}", 42); }',
  /* 62 */ "}",
  "",
].join("\n");

/** Plain (unmodified) items and the expression forms an agent greps for. */
const EXPRS = [
  /*  1 */ "struct Plain { a: u8 }",
  /*  2 */ "impl Plain { fn get(&self) -> u8 { self.a } }",
  /*  3 */ "impl Default for Plain { fn default() -> Self { Plain { a: 0 } } }",
  /*  4 */ "trait T { fn f(&self); }",
  /*  5 */ "fn plain(x: u8) -> u8 { x }",
  /*  6 */ "async fn later() {}",
  /*  7 */ "fn lifetimes<'a>(s: &'a str) -> &'a str { s }",
  /*  8 */ "fn main() {",
  /*  9 */ "    let o = Some(3);",
  /* 10 */ '    let v = o.expect("some");',
  /* 11 */ "    let w = o.map(|x| x + 1).unwrap_or(0);",
  /* 12 */ "    assert_eq!(v, 3);",
  /* 13 */ "    assert_eq!(1 + 1, 2);",
  /* 14 */ '    assert!(w > 0, "msg {}", w);',
  /* 15 */ "    dbg!(&v);",
  /* 16 */ "    dbg!(v);",
  /* 17 */ '    let r: &\'static str = "x";',
  /* 18 */ "    let _ = (r, plain(1), Plain::default().get());",
  /* 19 */ "    let t = (1, 2);",
  /* 20 */ "    let [a, b] = [1, 2];",
  /* 21 */ "    let _ = (t, a, b);",
  /* 22 */ "    if v == 3 { } else { }",
  /* 23 */ "    while v > 0 { break; }",
  /* 24 */ "    loop { break; }",
  /* 25 */ '    let s = String::from("s") + "t";',
  /* 26 */ "    let _ = s.len() as usize + 1;",
  /* 27 */ "}",
  "",
].join("\n");

const hits = (source: string, pattern: string): string[] =>
  binding.structuralSearch("rust", source, pattern).map((h) => `${h.kind}@${h.line}`);

interface Positive {
  readonly source: string;
  readonly pattern: string;
  readonly expect: readonly string[];
}

const POSITIVES: readonly Positive[] = [
  // --- macro invocations
  {
    source: ITEMS,
    pattern: "println!($$$ARGS)",
    expect: ["macro_invocation@31", "macro_invocation@34"],
  },
  {
    source: ITEMS,
    pattern: "$M!($$$ARGS)",
    expect: [
      "macro_invocation@20",
      "macro_invocation@31",
      "macro_invocation@32",
      "macro_invocation@33",
      "macro_invocation@34",
      "macro_invocation@43",
      "macro_invocation@58",
      "macro_invocation@58",
      "macro_invocation@61",
    ],
  },
  { source: ITEMS, pattern: "vec![$$$ITEMS]", expect: ["macro_invocation@29"] },
  { source: ITEMS, pattern: "format!($FMT, $$$REST)", expect: ["macro_invocation@20"] },
  { source: ITEMS, pattern: "panic!($$$A)", expect: ["macro_invocation@61"] },
  { source: ITEMS, pattern: "todo!()", expect: ["macro_invocation@43"] },
  {
    source: EXPRS,
    pattern: "assert_eq!($$$A)",
    expect: ["macro_invocation@12", "macro_invocation@13"],
  },
  { source: EXPRS, pattern: "assert!($$$A)", expect: ["macro_invocation@14"] },
  { source: EXPRS, pattern: "dbg!($$$X)", expect: ["macro_invocation@15", "macro_invocation@16"] },
  // --- attributes, on their own
  { source: ITEMS, pattern: "#[derive($$$D)]", expect: ["attribute_item@5", "attribute_item@11"] },
  { source: ITEMS, pattern: "#[derive(Debug)]", expect: ["attribute_item@11"] },
  { source: ITEMS, pattern: "#[should_panic]", expect: ["attribute_item@60"] },
  {
    source: ITEMS,
    pattern: "#[$A]",
    expect: [
      "attribute_item@5",
      "attribute_item@11",
      "attribute_item@54",
      "attribute_item@57",
      "attribute_item@59",
      "attribute_item@60",
    ],
  },
  // --- items: the shape must spell generics / visibility / async / where
  { source: ITEMS, pattern: "impl<$$$G> $T for $U { $$$BODY }", expect: ["impl_item@19"] },
  { source: ITEMS, pattern: "impl<$$$G> $T { $$$BODY }", expect: ["impl_item@23"] },
  { source: EXPRS, pattern: "impl $T for $U { $$$BODY }", expect: ["impl_item@3"] },
  { source: EXPRS, pattern: "impl $T { $$$BODY }", expect: ["impl_item@2"] },
  { source: ITEMS, pattern: "pub trait $T { $$$B }", expect: ["trait_item@14"] },
  { source: EXPRS, pattern: "trait $T { $$$B }", expect: ["trait_item@4"] },
  {
    source: ITEMS,
    pattern: "fn $F($$$P) -> $R { $$$B }",
    expect: ["function_item@16", "function_item@20"],
  },
  {
    source: ITEMS,
    pattern: "fn $F($$$P) { $$$B }",
    expect: ["function_item@58", "function_item@61"],
  },
  { source: ITEMS, pattern: "pub fn $F($$$P) -> $R { $$$B }", expect: ["function_item@24"] },
  { source: ITEMS, pattern: "pub async fn $F($$$P) -> $R { $$$B }", expect: ["function_item@25"] },
  { source: EXPRS, pattern: "async fn $F($$$P) { $$$B }", expect: ["function_item@6"] },
  {
    source: ITEMS,
    pattern: "fn $F<$$$G>($$$P) -> $R where $$$W { $$$B }",
    expect: ["function_item@47"],
  },
  { source: EXPRS, pattern: "fn $F<'$L>($$$P) -> $R { $$$B }", expect: ["function_item@7"] },
  { source: ITEMS, pattern: "fn $F(&self) -> $R;", expect: ["function_signature_item@15"] },
  {
    source: ITEMS,
    pattern: "fn $F(&self) -> $R { $$$B }",
    expect: ["function_item@16", "function_item@20"],
  },
  { source: ITEMS, pattern: "pub struct $S<$$$G> { $$$F }", expect: ["struct_item@6"] },
  { source: EXPRS, pattern: "struct $S { $$$F }", expect: ["struct_item@1"] },
  { source: ITEMS, pattern: "enum $E { $$$V }", expect: ["enum_item@12"] },
  { source: ITEMS, pattern: "const $C: $T = $V;", expect: ["const_item@49"] },
  { source: ITEMS, pattern: "static $C: $T = $V;", expect: ["static_item@50"] },
  { source: ITEMS, pattern: "type $A = $T;", expect: ["type_item@51"] },
  { source: ITEMS, pattern: "mod $M;", expect: ["mod_item@52"] },
  {
    source: ITEMS,
    pattern: "use $P;",
    expect: ["use_declaration@2", "use_declaration@3", "use_declaration@56"],
  },
  { source: ITEMS, pattern: "use $A::$B;", expect: ["use_declaration@2"] },
  { source: ITEMS, pattern: "use $A::{$$$ITEMS};", expect: ["use_declaration@3"] },
  // --- expressions and statements
  { source: ITEMS, pattern: "$X.unwrap()", expect: ["call_expression@27"] },
  { source: EXPRS, pattern: "$X.expect($M)", expect: ["call_expression@10"] },
  { source: EXPRS, pattern: "$X.unwrap_or($D)", expect: ["call_expression@11"] },
  { source: ITEMS, pattern: "$X?", expect: ["try_expression@26"] },
  {
    source: ITEMS,
    pattern: "let $V: $T = $E;",
    expect: ["let_declaration@29", "let_declaration@30"],
  },
  { source: ITEMS, pattern: "match $E { $$$ARMS }", expect: ["match_expression@36"] },
  { source: ITEMS, pattern: "unsafe { $$$B }", expect: ["unsafe_block@37"] },
  { source: ITEMS, pattern: "if let Some($X) = $E { $$$B }", expect: ["if_expression@34"] },
  { source: ITEMS, pattern: "if let $P = $E { $$$B }", expect: ["if_expression@34"] },
  { source: EXPRS, pattern: "if $C { $$$T } else { $$$E }", expect: ["if_expression@22"] },
  { source: ITEMS, pattern: "for $I in $IT { $$$B }", expect: ["for_expression@35"] },
  { source: EXPRS, pattern: "while $C { $$$B }", expect: ["while_expression@23"] },
  { source: EXPRS, pattern: "loop { $$$B }", expect: ["loop_expression@24"] },
  { source: ITEMS, pattern: "Box::new($X)", expect: ["call_expression@28"] },
  { source: ITEMS, pattern: "$T::new($$$A)", expect: ["call_expression@28"] },
  {
    source: ITEMS,
    pattern: "$A::$B($$$ARGS)",
    expect: ["call_expression@26", "call_expression@28"],
  },
  { source: ITEMS, pattern: "std::fs::read_to_string($P)", expect: ["call_expression@26"] },
  { source: ITEMS, pattern: "$X.parse::<$T>()", expect: ["call_expression@27"] },
  { source: EXPRS, pattern: "String::from($S)", expect: ["call_expression@25"] },
  { source: EXPRS, pattern: "$T::default()", expect: ["call_expression@18"] },
  { source: EXPRS, pattern: "Some($X)", expect: ["call_expression@9"] },
  { source: ITEMS, pattern: "$E.await", expect: ["await_expression@39"] },
  { source: ITEMS, pattern: "async { $$$B }", expect: ["async_block@38"] },
  { source: ITEMS, pattern: "|$$$P| $B", expect: ["closure_expression@30"] },
  { source: ITEMS, pattern: "|$X| $B", expect: ["closure_expression@30"] },
  { source: ITEMS, pattern: "$X.iter().map($F).collect()", expect: ["call_expression@30"] },
  { source: ITEMS, pattern: "$X.iter().map($F)", expect: ["call_expression@30"] },
  { source: EXPRS, pattern: "$RECV.map($F).unwrap_or($D)", expect: ["call_expression@11"] },
  { source: EXPRS, pattern: "$X.map(|$P| $B)", expect: ["call_expression@11"] },
  { source: ITEMS, pattern: "$E as $T", expect: ["type_cast_expression@40"] },
  { source: ITEMS, pattern: "&mut $X", expect: ["reference_expression@41"] },
  { source: ITEMS, pattern: "&$X", expect: ["reference_expression@35", "reference_expression@41"] },
  { source: ITEMS, pattern: "$X.clone()", expect: ["call_expression@41", "call_expression@47"] },
  { source: ITEMS, pattern: "$S { $$$F }", expect: ["struct_expression@24"] },
  {
    source: EXPRS,
    pattern: "$A + $B",
    expect: ["binary_expression@11", "binary_expression@25", "binary_expression@26"],
  },
  { source: EXPRS, pattern: "$E == $V", expect: ["binary_expression@22"] },
  { source: EXPRS, pattern: "let [$$$P] = $E;", expect: ["let_declaration@20"] },
  {
    source: EXPRS,
    pattern: "break;",
    expect: ["expression_statement@23", "expression_statement@24"],
  },
];

describe("Rust structural search over the real core (specs/015)", () => {
  test("the corpus parses cleanly and the core knows rust", () => {
    expect(binding.languages()).toContain("rust");
    expect(binding.parse("rust", ITEMS).hasError).toBe(false);
    expect(binding.parse("rust", EXPRS).hasError).toBe(false);
  });

  for (const c of POSITIVES) {
    test(`matches ${JSON.stringify(c.pattern)} → ${c.expect.join(", ")}`, () => {
      expect(hits(c.source, c.pattern)).toEqual([...c.expect]);
    });
  }

  test("method calls are call_expression — there is no method_call_expression kind", () => {
    const method = hits(ITEMS, "$RECV.$METHOD($$$ARGS)");
    expect(method).toHaveLength(9);
    expect(new Set(method.map((h) => h.split("@")[0]))).toEqual(new Set(["call_expression"]));
    // `$F($$$ARGS)` covers function calls AND method calls (15 in the corpus).
    expect(hits(ITEMS, "$F($$$ARGS)")).toHaveLength(15);
  });

  test("`let $V = $E;` skips `let _ = …` — `_` is a wildcard token, not an identifier node", () => {
    expect(hits(ITEMS, "let $V = $E;")).toEqual([
      "let_declaration@26",
      "let_declaration@27",
      "let_declaration@28",
      "let_declaration@38",
      "let_declaration@39",
      "let_declaration@40",
      "let_declaration@41",
    ]);
  });

  test("bare `fn $F` matches every non-pub function loosely — works, not recommended", () => {
    // Loose: signatures, generic fns and fns with bodies all match; but the
    // visibility rule still holds — `pub fn new` (24) and `pub async fn
    // load` (25) are NOT among them.
    expect(hits(ITEMS, "fn $F")).toEqual([
      "function_signature_item@15",
      "function_item@16",
      "function_item@20",
      "function_item@47",
      "function_item@58",
      "function_item@61",
    ]);
  });
});

// ---------------------------------------------------------------------------
// The zero-hit rules, each beside the counterpart that matches.

interface Negative {
  readonly rule: "macro-token" | "modifiers" | "partial-item" | "type-position";
  readonly source: string;
  readonly pattern: string;
  /** What the pattern still matches (the single-token macro cases keep some). */
  readonly expect: readonly string[];
  /** The corpus line the pattern "should" have matched. */
  readonly becauseOf: number;
  readonly instead: string;
  readonly insteadExpect: readonly string[];
}

const NEGATIVES: readonly Negative[] = [
  {
    rule: "macro-token",
    source: EXPRS,
    pattern: "assert_eq!($A, $B)",
    expect: ["macro_invocation@12"], // assert_eq!(v, 3): single tokens
    becauseOf: 13, // assert_eq!(1 + 1, 2): `1 + 1` is three tokens
    instead: "assert_eq!($$$A)",
    insteadExpect: ["macro_invocation@12", "macro_invocation@13"],
  },
  {
    rule: "macro-token",
    source: EXPRS,
    pattern: "dbg!($X)",
    expect: ["macro_invocation@16"], // dbg!(v): one token
    becauseOf: 15, // dbg!(&v): `&` and `v` are two tokens
    instead: "dbg!($$$X)",
    insteadExpect: ["macro_invocation@15", "macro_invocation@16"],
  },
  {
    rule: "macro-token",
    source: EXPRS,
    pattern: "assert!($C, $$$M)",
    expect: [],
    becauseOf: 14, // assert!(w > 0, …): `w > 0` is three tokens
    instead: "assert!($$$A)",
    insteadExpect: ["macro_invocation@14"],
  },
  {
    rule: "modifiers",
    source: ITEMS,
    pattern: "impl $T for $U { $$$BODY }",
    expect: [],
    becauseOf: 19, // impl<'a, T: Debug> Greeter for Config<'a, T>
    instead: "impl<$$$G> $T for $U { $$$BODY }",
    insteadExpect: ["impl_item@19"],
  },
  {
    rule: "modifiers",
    source: ITEMS,
    pattern: "trait $T { $$$B }",
    expect: [],
    becauseOf: 14, // pub trait Greeter
    instead: "pub trait $T { $$$B }",
    insteadExpect: ["trait_item@14"],
  },
  {
    rule: "modifiers",
    source: ITEMS,
    pattern: "struct $S { $$$F }",
    expect: [],
    becauseOf: 6, // pub struct Config<'a, T: Debug>
    instead: "pub struct $S<$$$G> { $$$F }",
    insteadExpect: ["struct_item@6"],
  },
  {
    rule: "modifiers",
    source: ITEMS,
    pattern: "async fn $F($$$P) -> $R { $$$B }",
    expect: [],
    becauseOf: 25, // pub async fn load
    instead: "pub async fn $F($$$P) -> $R { $$$B }",
    insteadExpect: ["function_item@25"],
  },
  {
    rule: "modifiers",
    source: ITEMS,
    pattern: "fn $F<$$$G>($$$P) -> $R { $$$B }",
    expect: [],
    becauseOf: 47, // fn generic<…>(…) -> T where U: Debug
    instead: "fn $F<$$$G>($$$P) -> $R where $$$W { $$$B }",
    insteadExpect: ["function_item@47"],
  },
  {
    rule: "partial-item",
    source: ITEMS,
    pattern: "pub fn $F($$$P)",
    expect: [],
    becauseOf: 24, // a fn header without its body (or `;`) matches nothing
    instead: "pub fn $F($$$P) -> $R { $$$B }",
    insteadExpect: ["function_item@24"],
  },
  {
    rule: "partial-item",
    source: EXPRS,
    pattern: "fn $F($$$P) -> $R",
    expect: [],
    becauseOf: 5, // fn plain(x: u8) -> u8 { x }
    instead: "fn $F($$$P) -> $R { $$$B }",
    insteadExpect: ["function_item@2", "function_item@3", "function_item@5"],
  },
  {
    rule: "type-position",
    source: EXPRS,
    pattern: "&'static str",
    expect: [],
    becauseOf: 17,
    instead: "let $V: &'static str = $E;",
    insteadExpect: ["let_declaration@17"],
  },
  {
    rule: "type-position",
    source: EXPRS,
    pattern: "&'$L $T",
    expect: [],
    becauseOf: 7,
    instead: "fn $F<'$L>($$$P) -> $R { $$$B }",
    insteadExpect: ["function_item@7"],
  },
];

/**
 * Header-only item patterns are LOOSE for impl/trait/struct — the missing
 * body does not stop the match — while a fn header needs its body or its
 * `;`. Modifiers still apply in every case (probe 3, specs/015).
 */
const HEADER_ONLY: readonly Positive[] = [
  { source: EXPRS, pattern: "impl $T for $U", expect: ["impl_item@3"] },
  { source: EXPRS, pattern: "impl $T", expect: ["impl_item@2", "impl_item@3"] },
  { source: ITEMS, pattern: "impl $T", expect: ["impl_item@19", "impl_item@23"] },
  { source: ITEMS, pattern: "impl<$$$G> $T for $U", expect: ["impl_item@19"] },
  { source: EXPRS, pattern: "trait $T", expect: ["trait_item@4"] },
  { source: EXPRS, pattern: "struct $S", expect: ["struct_item@1"] },
  { source: ITEMS, pattern: "pub struct $S<$$$G>", expect: ["struct_item@6"] },
  // A parameter-only fn pattern matches SIGNATURES only (`fn f(&self);`).
  { source: EXPRS, pattern: "fn $F($$$P)", expect: ["function_signature_item@4"] },
];

/** Attribute joined to its item: the core refuses the pattern outright. */
const REJECTED: readonly string[] = [
  "#[derive($$$D)] struct $S { $$$F }",
  "#[derive($$$D)]\nstruct $S { $$$F }",
  "#[test] fn $F() { $$$B }",
  "#[cfg(test)] mod $M { $$$B }",
];

describe("the zero-hit rules (specs/015 US2)", () => {
  for (const n of NEGATIVES) {
    test(`${n.rule}: ${JSON.stringify(n.pattern)} misses line ${n.becauseOf}; ${JSON.stringify(n.instead)} matches`, () => {
      const got = hits(n.source, n.pattern);
      expect(got).toEqual([...n.expect]);
      expect(got.some((h) => h.endsWith(`@${n.becauseOf}`))).toBe(false);
      expect(hits(n.source, n.instead)).toEqual([...n.insteadExpect]);
    });
  }

  for (const c of HEADER_ONLY) {
    test(`header-only ${JSON.stringify(c.pattern)} → ${c.expect.join(", ") || "nothing"}`, () => {
      expect(hits(c.source, c.pattern)).toEqual([...c.expect]);
    });
  }

  test("a pattern that spells generics matches only generic items, and vice versa", () => {
    // EXPRS has a plain impl (line 3) and no generic one; ITEMS the reverse.
    expect(hits(EXPRS, "impl<$$$G> $T for $U { $$$BODY }")).toEqual([]);
    expect(hits(ITEMS, "impl $T for $U { $$$BODY }")).toEqual([]);
    // `fn $F($$$P) -> $R { $$$B }` never sees pub, generic or async fns.
    expect(hits(ITEMS, "fn $F($$$P) -> $R { $$$B }")).not.toContain("function_item@24");
    expect(hits(ITEMS, "fn $F($$$P) -> $R { $$$B }")).not.toContain("function_item@25");
    expect(hits(ITEMS, "fn $F($$$P) -> $R { $$$B }")).not.toContain("function_item@47");
  });

  for (const pattern of REJECTED) {
    test(`attribute + item is rejected: ${JSON.stringify(pattern)}`, () => {
      let message = "";
      try {
        binding.structuralSearch("rust", ITEMS, pattern);
      } catch (error) {
        message = (error as Error).message;
      }
      expect(message).toContain("Multiple AST nodes");
      // The search tool maps exactly this message shape to PATTERN_INVALID
      // (src/tools/search.ts; the mapping itself is pinned in
      // test/search-tool.test.ts over a fake engine).
      expect(/invalid structural pattern/i.test(message)).toBe(true);
    });
  }
});

// ---------------------------------------------------------------------------
// Rewrites: metavariables and $$$ carry through, including macro token trees.

interface Rewrite {
  readonly source: string;
  readonly pattern: string;
  readonly replacement: string;
  readonly matchCount: number;
  /** The changed lines (1-based) and their new text. */
  readonly changed: readonly [number, string][];
}

const REWRITES: readonly Rewrite[] = [
  {
    source: ITEMS,
    pattern: "$X.unwrap()",
    replacement: '$X.expect("checked")',
    matchCount: 1,
    changed: [[27, '        let parsed = text.parse::<u8>().expect("checked");']],
  },
  {
    source: ITEMS,
    pattern: "println!($$$ARGS)",
    replacement: "log::info!($$$ARGS)",
    matchCount: 2,
    changed: [
      [31, '        log::info!("loaded {} {:?}", path, doubled);'],
      [34, '        if let Some(first) = v.first() { log::info!("{}", first); }'],
    ],
  },
  {
    source: ITEMS,
    pattern: "#[derive($$$D)]",
    replacement: "#[derive($$$D, Default)]",
    matchCount: 2,
    changed: [
      [5, "#[derive(Debug, Clone, PartialEq, Default)]"],
      [11, "#[derive(Debug, Default)]"],
    ],
  },
  {
    source: ITEMS,
    pattern: "Box::new($X)",
    replacement: "Arc::new($X)",
    matchCount: 1,
    changed: [[28, "        let boxed = Arc::new(parsed);"]],
  },
  {
    source: ITEMS,
    pattern: "$X?",
    replacement: "$X.unwrap()",
    matchCount: 1,
    changed: [[26, "        let text = std::fs::read_to_string(path).unwrap();"]],
  },
  {
    source: EXPRS,
    pattern: "assert_eq!($$$A)",
    replacement: 'assert_eq!($$$A, "ctx")',
    matchCount: 2,
    changed: [
      [12, '    assert_eq!(v, 3, "ctx");'],
      [13, '    assert_eq!(1 + 1, 2, "ctx");'],
    ],
  },
  {
    source: EXPRS,
    pattern: "dbg!($$$X)",
    replacement: "($$$X)",
    matchCount: 2,
    changed: [
      [15, "    (&v);"],
      [16, "    (v);"],
    ],
  },
  {
    source: EXPRS,
    pattern: "$X.expect($$$M)",
    replacement: "$X.unwrap()",
    matchCount: 1,
    changed: [[10, "    let v = o.unwrap();"]],
  },
];

describe("Rust structural rewrite over the real core (specs/015)", () => {
  for (const r of REWRITES) {
    test(`rewrite ${JSON.stringify(r.pattern)} → ${JSON.stringify(r.replacement)}`, () => {
      const out = binding.rewrite("rust", r.source, r.pattern, r.replacement);
      expect(out.matchCount).toBe(r.matchCount);
      const before = r.source.split("\n");
      const after = out.output.split("\n");
      expect(after).toHaveLength(before.length);
      const diff: [number, string][] = [];
      after.forEach((line, i) => {
        if (line !== before[i]) diff.push([i + 1, line]);
      });
      expect(diff).toEqual([...r.changed]);
    });
  }

  test("a multi-line replacement keeps the surrounding text (impl body extension)", () => {
    const out = binding.rewrite(
      "rust",
      EXPRS,
      "impl $T for $U { $$$BODY }",
      "impl $T for $U {\n    $$$BODY\n    fn added(&self) {}\n}",
    );
    expect(out.matchCount).toBe(1);
    expect(out.output).toContain(
      "impl Default for Plain {\n    fn default() -> Self { Plain { a: 0 } }\n    fn added(&self) {}\n}",
    );
    // Everything else is byte-identical.
    expect(out.output.replace(/impl Default for Plain \{[\s\S]*?\n\}/, "IMPL")).toBe(
      EXPRS.replace("impl Default for Plain { fn default() -> Self { Plain { a: 0 } } }", "IMPL"),
    );
  });

  test("a pattern with no match leaves the source unchanged with count 0", () => {
    const out = binding.rewrite("rust", ITEMS, "assert_eq!($A, $B)", "assert_eq!($B, $A)");
    expect(out.matchCount).toBe(0);
    expect(out.output).toBe(ITEMS);
  });
});

// ---------------------------------------------------------------------------
// The engine child dispatches by extension.

describe("engine child dispatch (specs/015 SC-003)", () => {
  test("a .rs path reaches the Rust grammar; an unknown extension answers no hits", () => {
    const rust = handleRequest(binding, "structural", {
      path: "crates/x/src/lib.rs",
      source: EXPRS,
      pattern: "$X.expect($M)",
    }) as { hits: readonly { kind: string; line: number }[] };
    expect(rust.hits.map((h) => `${h.kind}@${h.line}`)).toEqual(["call_expression@10"]);
    const unknown = handleRequest(binding, "structural", {
      path: "notes/x.unknownext",
      source: EXPRS,
      pattern: "$X.expect($M)",
    }) as { hits: readonly unknown[] };
    expect(unknown.hits).toEqual([]);
  });

  test("rewrite through the child is structural for .rs and literal for an unknown extension", () => {
    const structural = handleRequest(binding, "rewrite", {
      path: "src/lib.rs",
      source: EXPRS,
      edits: [{ find: "$X.expect($$$M)", replace: "$X.unwrap()" }],
    }) as { content: string; changed: boolean };
    expect(structural.changed).toBe(true);
    expect(structural.content).toContain("let v = o.unwrap();");
    const literal = handleRequest(binding, "rewrite", {
      path: "notes/x.unknownext",
      source: EXPRS,
      edits: [{ find: "$X.expect($$$M)", replace: "$X.unwrap()" }],
    }) as { content: string; changed: boolean };
    expect(literal.changed).toBe(false); // no literal "$X.expect($$$M)" in the source
  });
});
