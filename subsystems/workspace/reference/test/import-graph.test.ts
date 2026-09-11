import { describe, expect, test } from "bun:test";
import {
  ALLOWED_EDGES,
  checkImportGraph,
  extractImportSpecifiers,
} from "../scripts/check-import-graph";

describe("extractImportSpecifiers", () => {
  test("finds static, side-effect, export-from and dynamic imports", () => {
    const source = [
      `import { a } from "./a";`,
      `import type { B } from "../ports";`,
      `import "./side-effect";`,
      `export { c } from "./c";`,
      `export type { D } from "./d";`,
      `const e = await import("./e");`,
    ].join("\n");
    expect(extractImportSpecifiers(source)).toEqual([
      "./a",
      "../ports",
      "./c",
      "./d",
      "./side-effect",
      "./e",
    ]);
  });

  test("ignores package imports in classification but still extracts them", () => {
    expect(extractImportSpecifiers(`import { z } from "zod";`)).toEqual(["zod"]);
  });
});

describe("checkImportGraph", () => {
  test("tools importing an adapter is the canonical violation", () => {
    const violations = checkImportGraph([
      { path: "src/tools/search.ts", specifiers: ["../adapters/inprocess"] },
    ]);
    expect(violations).toHaveLength(1);
    expect(violations[0]?.reason).toBe("forbidden edge tools → adapters");
  });

  test("tools reaching around the port barrel into generated code fails", () => {
    const violations = checkImportGraph([
      { path: "src/tools/search.ts", specifiers: ["../generated/schemas"] },
    ]);
    expect(violations).toHaveLength(1);
    expect(violations[0]?.reason).toBe("forbidden edge tools → generated");
  });

  test("directory-form imports classify by their layer, not as app", () => {
    const violations = checkImportGraph([
      { path: "src/tools/deep/nested.ts", specifiers: ["../../adapters"] },
    ]);
    expect(violations).toHaveLength(1);
    expect(violations[0]?.reason).toBe("forbidden edge tools → adapters");
  });

  test("the declared rule holds: protocol → tools → ports ← adapters", () => {
    const violations = checkImportGraph([
      { path: "src/protocol/server.ts", specifiers: ["../tools/search", "../ports"] },
      { path: "src/tools/search.ts", specifiers: ["../ports", "../envelope"] },
      { path: "src/adapters/inprocess/inprocess.ts", specifiers: ["../../ports"] },
      { path: "src/ports/engine.ts", specifiers: ["./types", "./versions"] },
    ]);
    expect(violations).toEqual([]);
  });

  test("adapters may not import tools or protocol", () => {
    const violations = checkImportGraph([
      { path: "src/adapters/ipc/bridge.ts", specifiers: ["../../tools/search"] },
    ]);
    expect(violations).toHaveLength(1);
    expect(violations[0]?.reason).toBe("forbidden edge adapters → tools");
  });

  test("a file in an undeclared directory fails closed", () => {
    const violations = checkImportGraph([{ path: "src/machinery/secret.ts", specifiers: [] }]);
    expect(violations).toHaveLength(1);
    expect(violations[0]?.reason).toContain("outside every declared layer");
  });

  test("ports stay at the bottom: no edge from ports to tools or adapters", () => {
    expect(ALLOWED_EDGES.ports).not.toContain("tools");
    expect(ALLOWED_EDGES.ports).not.toContain("adapters");
    expect(ALLOWED_EDGES.tools).not.toContain("adapters");
  });
});

describe("the real tree", () => {
  test("src/ has no import-graph violations", async () => {
    const modules = [];
    for await (const file of new Bun.Glob("src/**/*.ts").scan(".")) {
      modules.push({
        path: file.replaceAll("\\", "/"),
        specifiers: extractImportSpecifiers(await Bun.file(file).text()),
      });
    }
    expect(modules.length).toBeGreaterThan(0);
    expect(checkImportGraph(modules)).toEqual([]);
  });
});
