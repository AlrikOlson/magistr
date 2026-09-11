/**
 * Engine-child handler unit tests — the method table against the stub core,
 * no process wiring. The process-level behavior (ready gating, kills,
 * restart) lives in test/engine-supervisor.test.ts against real children.
 */

import { describe, expect, test } from "bun:test";
import {
  ENGINE_METHODS,
  handleRequest,
  languageForPath,
  stubCoreBinding,
} from "../src/adapters/engine-ipc";

const binding = stubCoreBinding();

describe("languageForPath", () => {
  test("maps bundled-grammar extensions and refuses the rest", () => {
    expect(languageForPath("src/a.ts")).toBe("typescript");
    expect(languageForPath("component.tsx")).toBe("tsx");
    expect(languageForPath("lib.rs")).toBe("rust");
    expect(languageForPath("script.py")).toBe("python");
    expect(languageForPath("main.go")).toBe("go");
    expect(languageForPath("README.md")).toBeUndefined();
    expect(languageForPath("Makefile")).toBeUndefined();
  });
});

describe("handleRequest", () => {
  test("ping answers pong", () => {
    expect(handleRequest(binding, "ping", {})).toEqual({ pong: true });
  });

  test("parse verdicts: ok, syntax break, and non-source passthrough", () => {
    expect(handleRequest(binding, "parse", { path: "a.ts", source: "const a = [1]" })).toEqual({
      ok: true,
    });
    const broken = handleRequest(binding, "parse", { path: "a.ts", source: "const a = [1" }) as {
      ok: boolean;
      parserMessage?: string;
    };
    expect(broken.ok).toBe(false);
    expect(broken.parserMessage).toContain("typescript");
    // Non-source files carry nothing to parse-verify.
    expect(handleRequest(binding, "parse", { path: "notes.md", source: "([" })).toEqual({
      ok: true,
    });
  });

  test("search returns per-line hits with 1-based lines", () => {
    const result = handleRequest(binding, "search", {
      pattern: "needle",
      haystack: "hay\nneedle here\nhay",
      path: "a.txt",
    }) as { hits: readonly { line: number; start: number; lineText: string }[] };
    expect(result.hits).toHaveLength(1);
    expect(result.hits[0]?.line).toBe(2);
    expect(result.hits[0]?.lineText).toBe("needle here");
  });

  test("rewrite applies edits in order and reports changed", () => {
    expect(
      handleRequest(binding, "rewrite", {
        path: "a.md",
        source: "one two one",
        edits: [
          { find: "one", replace: "1" },
          { find: "two", replace: "2" },
        ],
      }),
    ).toEqual({ content: "1 2 1", changed: true });
    expect(handleRequest(binding, "rewrite", { path: "a.md", source: "same", edits: [] })).toEqual({
      content: "same",
      changed: false,
    });
  });

  test("unknown methods and malformed params throw answerable errors", () => {
    expect(() => handleRequest(binding, "explode", {})).toThrow("unknown method");
    expect(() => handleRequest(binding, "parse", { path: "a.ts" })).toThrow("params.source");
    expect(() => handleRequest(binding, "rewrite", { path: "a", source: "b" })).toThrow(
      "params.edits",
    );
  });

  test("the method table names exactly what the child serves", () => {
    expect(ENGINE_METHODS).toEqual(["ping", "parse", "search", "structural", "rewrite"]);
  });
});
