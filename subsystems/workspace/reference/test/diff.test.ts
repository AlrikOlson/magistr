/**
 * The unified diff (SDD §3) — the write engine's public report dialect.
 * Rendering is pinned exactly: headers, hunk ranges, context merging and
 * the no-trailing-newline marker are contract, not cosmetics.
 */

import { describe, expect, test } from "bun:test";
import { unifiedDiff } from "../src/envelope";

describe("unifiedDiff", () => {
  test("unrelated text falls back to a complete replacement within a bounded search", () => {
    const before = Array.from({ length: 1200 }, (_, i) => `before ${i}\n`).join("");
    const after = Array.from({ length: 1200 }, (_, i) => `after ${i}\n`).join("");
    const diff = unifiedDiff({ path: "large.txt", before, after });
    expect(diff).toContain("Diff search budget exceeded");
    expect(
      diff
        .split("\n")
        .filter((line) => line.startsWith("-") && !line.startsWith("---"))
        .map((line) => `${line.slice(1)}\n`)
        .join(""),
    ).toBe(before);
    expect(
      diff
        .split("\n")
        .filter((line) => line.startsWith("+") && !line.startsWith("+++"))
        .map((line) => `${line.slice(1)}\n`)
        .join(""),
    ).toBe(after);
  });
  test("identical contents produce the empty string", () => {
    expect(unifiedDiff({ path: "f.ts", before: "x\n", after: "x\n" })).toBe("");
    expect(unifiedDiff({ path: "f.ts", before: null, after: null })).toBe("");
  });

  test("a one-line change renders one hunk with context", () => {
    const diff = unifiedDiff({ path: "f.ts", before: "a\nb\nc\n", after: "a\nB\nc\n" });
    expect(diff).toBe("--- a/f.ts\n+++ b/f.ts\n@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n");
  });

  test("creation renders /dev/null and an empty from-range", () => {
    const diff = unifiedDiff({ path: "new.ts", before: null, after: "one\ntwo\n" });
    expect(diff).toBe("--- /dev/null\n+++ b/new.ts\n@@ -0,0 +1,2 @@\n+one\n+two\n");
  });

  test("deletion renders /dev/null on the to-side", () => {
    const diff = unifiedDiff({ path: "old.ts", before: "gone\n", after: null });
    expect(diff).toBe("--- a/old.ts\n+++ /dev/null\n@@ -1,1 +0,0 @@\n-gone\n");
  });

  test("nearby changes merge into one hunk; distant ones split", () => {
    const lines = Array.from({ length: 20 }, (_, i) => `line${i + 1}`);
    const near = [...lines];
    near[1] = "changed2";
    near[5] = "changed6";
    const merged = unifiedDiff({
      path: "f.ts",
      before: `${lines.join("\n")}\n`,
      after: `${near.join("\n")}\n`,
    });
    expect(merged.match(/^@@ /gm)).toHaveLength(1);

    const far = [...lines];
    far[1] = "changed2";
    far[17] = "changed18";
    const split = unifiedDiff({
      path: "f.ts",
      before: `${lines.join("\n")}\n`,
      after: `${far.join("\n")}\n`,
    });
    expect(split.match(/^@@ /gm)).toHaveLength(2);
  });

  test("a missing trailing newline gets the standard marker on both sides", () => {
    const diff = unifiedDiff({ path: "f.ts", before: "a\nb", after: "a\nc" });
    expect(diff).toBe(
      "--- a/f.ts\n+++ b/f.ts\n@@ -1,2 +1,2 @@\n a\n-b\n" +
        "\\ No newline at end of file\n+c\n\\ No newline at end of file\n",
    );
  });

  test("a trailing-newline-only change is a real diff, not equality", () => {
    const diff = unifiedDiff({ path: "f.ts", before: "a", after: "a\n" });
    expect(diff).toBe(
      "--- a/f.ts\n+++ b/f.ts\n@@ -1,1 +1,1 @@\n-a\n\\ No newline at end of file\n+a\n",
    );
  });

  test("a negative or fractional context is refused", () => {
    expect(() => unifiedDiff({ path: "f.ts", before: "a\n", after: "b\n", context: -1 })).toThrow(
      RangeError,
    );
    expect(() => unifiedDiff({ path: "f.ts", before: "a\n", after: "b\n", context: 1.5 })).toThrow(
      RangeError,
    );
  });

  test("an edit script survives asymmetric sizes", () => {
    const diff = unifiedDiff({ path: "f.ts", before: "a\n", after: "x\ny\nz\na\nq\n" });
    expect(diff.startsWith("--- a/f.ts\n+++ b/f.ts\n")).toBe(true);
    expect(diff).toContain("+x");
    expect(diff).toContain("+q");
    expect(diff).toContain(" a");
  });
});
