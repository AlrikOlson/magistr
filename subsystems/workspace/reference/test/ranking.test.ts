/**
 * Golden-file coverage for the six ranking-pipeline stages (SDD §7.4).
 * Inputs live in this file; every expected output is a checked-in golden
 * under test/support/golden/ranking/. Regenerate with UPDATE_GOLDEN=1 after
 * an intentional behavior change — and remember the pipeline's own law: a
 * weight change also needs a before/after MRR from `bun run eval:rank`.
 */

import { describe, expect, test } from "bun:test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  type CandidateGroup,
  type ClassifiedGroup,
  type ClassifiedHit,
  classifyHit,
  dedupAcrossFiles,
  fileClassOf,
  fileClassOfGroup,
  inTestRange,
  orderGroups,
  orderHits,
  rankGroups,
  trimToBudget,
} from "../src/tools/ranking";

const GOLDEN_DIR = path.join(import.meta.dir, "support", "golden", "ranking");
const UPDATE = process.env.UPDATE_GOLDEN === "1";

function golden(name: string, actual: unknown): void {
  const file = path.join(GOLDEN_DIR, `${name}.json`);
  // Round-trip through JSON so undefined-valued fields compare like the disk copy.
  const plain = JSON.parse(JSON.stringify(actual));
  if (UPDATE) {
    mkdirSync(GOLDEN_DIR, { recursive: true });
    writeFileSync(file, `${JSON.stringify(plain, null, 2)}\n`);
    return;
  }
  const expected = JSON.parse(readFileSync(file, "utf8"));
  expect(plain).toEqual(expected);
}

function hit(partial: Partial<ClassifiedHit> & { line: number }): ClassifiedHit {
  return {
    column: 1,
    excerpt: "",
    kindClass: "other",
    ...partial,
  } as ClassifiedHit;
}

function group(
  file: string,
  hits: readonly ClassifiedHit[],
  flags: { junk?: boolean; test?: boolean } = {},
): ClassifiedGroup {
  const junk = flags.junk ?? false;
  const testFlag = flags.test ?? false;
  return {
    file,
    junk,
    test: testFlag,
    fileClass: fileClassOf({ junk, test: testFlag }),
    hits,
    totalHits: hits.length,
  };
}

describe("stage 1 — kind classification", () => {
  const cases: readonly { name: string; query: string; hit: Parameters<typeof classifyHit>[0] }[] =
    [
      // Warm tags decide.
      {
        name: "symbol tag function",
        query: "needle",
        hit: { line: 1, column: 1, excerpt: "export function needle() {}", kind: "function" },
      },
      {
        name: "structural call node",
        query: "needle",
        hit: { line: 1, column: 1, excerpt: "needle(1);", kind: "call_expression" },
      },
      {
        name: "structural import node",
        query: "needle",
        hit: { line: 1, column: 1, excerpt: "import needle", kind: "import_statement" },
      },
      {
        name: "structural comment node",
        query: "needle",
        hit: { line: 1, column: 1, excerpt: "// needle", kind: "comment" },
      },
      {
        name: "structural string node",
        query: "needle",
        hit: { line: 1, column: 1, excerpt: '"needle"', kind: "string_fragment" },
      },
      {
        name: "references definition",
        query: "needle",
        hit: { line: 1, column: 1, excerpt: "fn needle()", kind: "definition" },
      },
      {
        name: "references use falls back to excerpt",
        query: "needle",
        hit: { line: 1, column: 1, excerpt: "needle(v)", kind: "use" },
      },
      // Cold heuristics.
      {
        name: "cold comment",
        query: "needle",
        hit: { line: 1, column: 4, excerpt: "// needle does things" },
      },
      {
        name: "cold hash comment",
        query: "needle",
        hit: { line: 1, column: 3, excerpt: "# needle in python" },
      },
      {
        name: "cold import",
        query: "needle",
        hit: { line: 1, column: 10, excerpt: 'import { needle } from "./mod"' },
      },
      {
        name: "cold rust use",
        query: "needle",
        hit: { line: 1, column: 5, excerpt: "use crate::needle::Thing;" },
      },
      {
        name: "cold fn declaration",
        query: "needle",
        hit: { line: 1, column: 4, excerpt: "fn needle(x: u32) -> u32 {" },
      },
      {
        name: "cold const declaration",
        query: "needle",
        hit: { line: 1, column: 7, excerpt: "const needle = 42;" },
      },
      {
        name: "cold string span",
        query: "needle",
        hit: { line: 1, column: 14, excerpt: 'console.log("needle here");' },
      },
      {
        name: "cold call",
        query: "needle",
        hit: { line: 1, column: 1, excerpt: "needle(argument);" },
      },
      {
        name: "cold bare mention",
        query: "needle",
        hit: { line: 1, column: 5, excerpt: "x + needle + y" },
      },
    ];

  test("warm tags decide, cold heuristics fall back (golden)", () => {
    golden(
      "stage1-classify",
      cases.map((c) => ({ name: c.name, class: classifyHit(c.hit, c.query) })),
    );
  });
});

describe("stage 2 — file-class weighting", () => {
  test("source > tests > vendor, junk dominates (golden)", () => {
    const cases = [
      { name: "plain source", junk: false, test: false },
      { name: "test path", junk: false, test: true },
      { name: "vendor", junk: true, test: false },
      { name: "vendored test is vendor", junk: true, test: true },
    ];
    golden(
      "stage2-file-class",
      cases.map((c) => ({ name: c.name, class: fileClassOf(c) })),
    );
  });
});

describe("stage 3 — definition-first ordering within a group", () => {
  test("declarations, then confidence, then kind weight, then line (golden)", () => {
    const hits: ClassifiedHit[] = [
      hit({ line: 2, excerpt: "// mentions needle", kindClass: "comment" }),
      hit({ line: 8, excerpt: "needle();", kindClass: "call" }),
      hit({ line: 5, excerpt: "function needle() {}", kindClass: "declaration" }),
      hit({ line: 3, excerpt: "needle(x);", kindClass: "call", confidence: "low" }),
      hit({ line: 9, excerpt: "needle(y);", kindClass: "call", confidence: "high" }),
      hit({ line: 1, excerpt: "x = needle", kindClass: "other" }),
    ];
    golden(
      "stage3-order-hits",
      orderHits(hits).map((h) => ({
        line: h.line,
        kindClass: h.kindClass,
        ...(h.confidence === undefined ? {} : { confidence: h.confidence }),
      })),
    );
  });
});

describe("stage 4 — dedup across files", () => {
  test("a line in >= minFiles files collapses to the best file + count (golden)", () => {
    const boilerplate = 'export const GENERATED = "needle";';
    const groups = [
      group("vendor/gen-a.ts", [hit({ line: 1, excerpt: boilerplate, kindClass: "declaration" })], {
        junk: true,
      }),
      group("src/real.ts", [
        hit({ line: 3, excerpt: boilerplate, kindClass: "declaration" }),
        hit({ line: 9, excerpt: "needle();", kindClass: "call" }),
      ]),
      group("vendor/gen-b.ts", [hit({ line: 1, excerpt: boilerplate, kindClass: "declaration" })], {
        junk: true,
      }),
      // Only two files share this line — below minFiles, both stay.
      group("src/pair-one.ts", [hit({ line: 2, excerpt: "twice(needle);", kindClass: "call" })]),
      group("src/pair-two.ts", [hit({ line: 4, excerpt: "twice(needle);", kindClass: "call" })]),
    ];
    golden(
      "stage4-dedup",
      dedupAcrossFiles(groups).map((g) => ({
        file: g.file,
        totalHits: g.totalHits,
        hits: g.hits.map((h) => ({
          line: h.line,
          excerpt: h.excerpt,
          ...(h.duplicates === undefined ? {} : { duplicates: h.duplicates }),
        })),
      })),
    );
  });
});

describe("stage 5 — group ordering", () => {
  test("definition, confidence, tier, score+path relevance, hits, path (golden)", () => {
    const groups = [
      group("src/deep/nested/mention.ts", [
        hit({ line: 1, excerpt: "needle + 1", kindClass: "other" }),
      ]),
      group(
        "test/needle.test.ts",
        [
          hit({ line: 1, excerpt: "needle();", kindClass: "call" }),
          hit({ line: 2, excerpt: "needle();", kindClass: "call" }),
        ],
        { test: true },
      ),
      // Definition in a test file still leads (law #2: definitions first).
      group(
        "test/def-here.test.ts",
        [hit({ line: 1, excerpt: "function needle() {}", kindClass: "declaration" })],
        { test: true },
      ),
      group(
        "vendor/lock.json",
        [hit({ line: 1, excerpt: '"needle": "1.0.0"', kindClass: "string" })],
        { junk: true },
      ),
      // Path relevance: the query token in the path outranks the deep mention.
      group("src/needle/core.ts", [hit({ line: 1, excerpt: "needle + 2", kindClass: "other" })]),
      group("src/refs-high.ts", [
        hit({ line: 1, excerpt: "needle(a)", kindClass: "call", confidence: "high" }),
      ]),
      group("src/refs-low.ts", [
        hit({ line: 1, excerpt: "needle(b)", kindClass: "call", confidence: "low" }),
      ]),
    ];
    golden(
      "stage5-order-groups",
      orderGroups(groups, ["needle"]).map((g) => g.file),
    );
  });
});

describe("stage 6 — trim to budget", () => {
  const wire = (file: string, excerpt: string) => ({
    file,
    junk: false,
    totalHits: 1,
    hits: [{ line: 1, column: 1, excerpt }],
  });
  const groups = [
    wire("a.ts", "short"),
    wire("b.ts", "x".repeat(200)),
    wire("c.ts", "y".repeat(200)),
  ];

  test("keeps whole groups until the estimate is spent (golden)", () => {
    golden("stage6-trim", {
      allFit: trimToBudget(groups, 4000),
      partial: trimToBudget(groups, 100),
      firstTooBig: trimToBudget([wire("big.ts", "z".repeat(4000))], 100),
      empty: trimToBudget([], 100),
    });
  });
});

describe("full pipeline", () => {
  test("candidates in, ranked wire groups out (golden)", () => {
    const boilerplate = "module.exports = { needle: true };";
    const candidates: CandidateGroup[] = [
      {
        file: "test/core.test.ts",
        junk: false,
        test: true,
        hits: [
          { line: 1, column: 1, excerpt: "needle(); needle();" },
          { line: 2, column: 1, excerpt: "expect(needle).toBe(1);" },
        ],
      },
      {
        file: "src/core.ts",
        junk: false,
        test: false,
        hits: [
          { line: 10, column: 3, excerpt: "needle();" },
          { line: 1, column: 17, excerpt: "export function needle() {}", kind: "function" },
        ],
      },
      {
        file: "src/docs.ts",
        junk: false,
        test: false,
        hits: [{ line: 4, column: 4, excerpt: "// needle explains this" }],
      },
      {
        file: "vendor/a.js",
        junk: true,
        test: false,
        hits: [{ line: 1, column: 1, excerpt: boilerplate }],
      },
      {
        file: "vendor/b.js",
        junk: true,
        test: false,
        hits: [{ line: 1, column: 1, excerpt: boilerplate }],
      },
      {
        file: "vendor/c.js",
        junk: true,
        test: false,
        hits: [{ line: 1, column: 1, excerpt: boilerplate }],
      },
    ];
    golden("full-pipeline", rankGroups(candidates, "needle"));
  });
});

// ------------------------------- Rust test regions (specs/009 R4 / SC-003)
// Rust puts unit tests INSIDE the file under test, so per-file classification
// cannot see them. Hits inside a `#[cfg(test)]` module are test code.

describe("hit-aware test regions", () => {
  const ranges = [{ startLine: 20, endLine: 40 }];

  test("a production hit outranks a cfg(test) hit in the SAME file", () => {
    const groups = rankGroups(
      [
        {
          file: "src/lib.rs",
          junk: false,
          test: false,
          testRanges: ranges,
          hits: [
            { line: 25, column: 9, excerpt: "        let c = compute(2);" },
            { line: 5, column: 5, excerpt: "    compute(input)" },
          ],
        },
      ],
      "compute",
    );
    expect(groups[0]?.hits.map((h) => h.line)).toEqual([5, 25]);
  });

  test("a file whose every hit is inside a test region classes as test", () => {
    const [testOnly, production] = rankGroups(
      [
        {
          file: "src/lib.rs",
          junk: false,
          test: false,
          testRanges: ranges,
          hits: [{ line: 30, column: 9, excerpt: "        compute(1);" }],
        },
        {
          file: "src/other.rs",
          junk: false,
          test: false,
          hits: [{ line: 3, column: 1, excerpt: "compute(input)" }],
        },
      ],
      "compute",
    );
    // Ordering puts the production file first; the test-only file is demoted
    // even though its path is ordinary source.
    expect(testOnly?.file).toBe("src/other.rs");
    expect(production?.file).toBe("src/lib.rs");
  });

  test("a file with any production hit keeps its own class", () => {
    expect(
      fileClassOfGroup({
        file: "src/lib.rs",
        junk: false,
        test: false,
        testRanges: ranges,
        hits: [
          { line: 30, column: 1, excerpt: "compute(1);" },
          { line: 5, column: 1, excerpt: "compute(x)" },
        ],
      }),
    ).toBe("source");
  });

  test("no ranges means exactly today's behavior", () => {
    const group = { file: "src/lib.rs", junk: false, test: false, hits: [] };
    expect(fileClassOfGroup(group)).toBe(fileClassOf(group));
    expect(fileClassOfGroup({ ...group, junk: true })).toBe("vendor");
    expect(fileClassOfGroup({ ...group, test: true })).toBe("test");
  });

  test("inTestRange is inclusive on both ends", () => {
    expect(inTestRange(20, ranges)).toBe(true);
    expect(inTestRange(40, ranges)).toBe(true);
    expect(inTestRange(19, ranges)).toBe(false);
    expect(inTestRange(41, ranges)).toBe(false);
    expect(inTestRange(25, undefined)).toBe(false);
  });
});
