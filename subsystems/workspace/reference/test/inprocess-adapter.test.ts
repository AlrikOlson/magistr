import { describe, expect, test } from "bun:test";
import { createInProcessPorts, InProcessWorkspace } from "../src/adapters/inprocess";
import type { WatchEvent } from "../src/ports";
import { PORT_NAMES, PORT_VERSIONS } from "../src/ports";

describe("the port vocabulary", () => {
  test("names exactly the seven ports of SDD §6.4", () => {
    expect([...PORT_NAMES].sort()).toEqual([
      "engine",
      "formatter",
      "index",
      "journal",
      "resolver",
      "sandbox",
      "watcher",
    ]);
  });
});

describe("the in-process adapter", () => {
  test("implements every port at its declared signature version", () => {
    const ports = createInProcessPorts();
    for (const name of PORT_NAMES) {
      expect(ports[name].version).toBe(PORT_VERSIONS[name]);
    }
  });

  test("engine.parse verdicts carry the parser message on rejection", async () => {
    const { engine } = createInProcessPorts();
    expect(await engine.parse({ path: "a.ts", content: "const x = { y: [1] };" })).toEqual({
      ok: true,
    });
    const rejected = await engine.parse({ path: "a.ts", content: "const x = { y: [1};" });
    expect(rejected.ok).toBe(false);
    expect(rejected.parserMessage).toContain("unbalanced");
  });

  test("engine.search finds seeded content at 1-based locations", async () => {
    const ports = createInProcessPorts();
    ports.workspace.files.set("src/a.ts", "line one\nconst needle = 1;\n");
    const matches = await ports.engine.search({ mode: "lexical", pattern: "needle", paths: [] });
    expect(matches).toEqual([
      {
        location: { path: "src/a.ts", line: 2, column: 7 },
        excerpt: "const needle = 1;",
      },
    ]);
  });

  test("engine.rewrite applies edits and reports changed honestly", async () => {
    const { engine } = createInProcessPorts();
    const result = await engine.rewrite({
      path: "a.ts",
      content: "let a = 1;",
      edits: [{ find: "let", replace: "const" }],
    });
    expect(result).toEqual({ content: "const a = 1;", changed: true });
    const noop = await engine.rewrite({ path: "a.ts", content: "const a = 1;", edits: [] });
    expect(noop.changed).toBe(false);
  });

  test("index upserts bump the generation and queries are case-insensitive", async () => {
    const { index } = createInProcessPorts();
    expect((await index.status()).generation).toBe(0);
    await index.upsertFile({
      path: "src/a.ts",
      symbols: [
        {
          name: "OkEnvelope",
          kind: "interface",
          location: { path: "src/a.ts", line: 3, column: 1 },
        },
      ],
    });
    expect((await index.status()).generation).toBe(1);
    expect((await index.querySymbols({ name: "okenvelope" })).symbols).toHaveLength(1);
    expect((await index.querySymbols({ name: "okenvelope" })).freshness).toBe("warm");
    expect(
      (await index.querySymbols({ name: "okenvelope", kind: "function" })).symbols,
    ).toHaveLength(0);
    await index.removeFile("src/a.ts");
    expect((await index.querySymbols({ name: "okenvelope" })).symbols).toHaveLength(0);
  });

  test("journal assigns monotonic sequence numbers and lists from a cursor", async () => {
    const { journal } = createInProcessPorts();
    const op = {
      tool: "edit",
      argsHash: "",
      sessionId: "s1",
      files: [{ kind: "write" as const, path: "a.ts", before: "1", after: "2" }],
    };
    expect((await journal.append(op)).op.seq).toBe(1);
    expect((await journal.append(op)).op.seq).toBe(2);
    expect(await journal.list(2)).toHaveLength(1);
    expect((await journal.get(1))?.files[0]?.before).toBe("1");
    expect(await journal.get(99)).toBeNull();
  });

  test("watcher delivers to path-scoped subscribers until closed", async () => {
    const { watcher } = createInProcessPorts();
    const seen: WatchEvent[] = [];
    const subscription = await watcher.subscribe(["src"], (event) => seen.push(event));
    watcher.emit({ kind: "modified", path: "src/a.ts" });
    watcher.emit({ kind: "modified", path: "test/b.ts" });
    expect(seen).toEqual([{ kind: "modified", path: "src/a.ts" }]);
    await subscription.close();
    watcher.emit({ kind: "removed", path: "src/a.ts" });
    expect(seen).toHaveLength(1);
  });

  test("sandbox canonicalizes dot segments and refuses workspace escapes", async () => {
    const ports = createInProcessPorts(new InProcessWorkspace("/workspace"));
    expect(await ports.sandbox.canonicalize("src/../src/a.ts")).toBe("/workspace/src/a.ts");
    await expect(ports.sandbox.canonicalize("../etc/passwd")).rejects.toThrow("outside");
    const verdict = await ports.sandbox.check({ path: "../../etc/passwd", intent: "read" });
    expect(verdict.allowed).toBe(false);
    expect(verdict.reason).toBeDefined();
  });

  test("formatter trims trailing whitespace, ends with newline, and is idempotent", async () => {
    const { formatter } = createInProcessPorts();
    const first = await formatter.format({ path: "a.ts", content: "const a = 1;  " });
    expect(first).toEqual({ content: "const a = 1;\n", changed: true });
    const second = await formatter.format({ path: "a.ts", content: first.content });
    expect(second.changed).toBe(false);
  });

  test("resolver answers definitions from the index and references from search", async () => {
    const ports = createInProcessPorts();
    ports.workspace.files.set("src/a.ts", "export function greet() {}\n");
    ports.workspace.files.set("src/b.ts", "greet();\n");
    await ports.index.upsertFile({
      path: "src/a.ts",
      symbols: [
        { name: "greet", kind: "function", location: { path: "src/a.ts", line: 1, column: 17 } },
      ],
    });
    // Port v2: every resolved edge carries honest symbolic provenance —
    // never `resolver-lsp`, which only the plugin lane may claim.
    expect(await ports.resolver.resolve({ kind: "definition", symbol: "greet" })).toEqual([
      {
        location: { path: "src/a.ts", line: 1, column: 17 },
        provenance: "tier1-symbols",
        confidence: "high",
      },
    ]);
    const references = await ports.resolver.resolve({ kind: "references", symbol: "greet" });
    expect(references).toHaveLength(2);
    for (const edge of references) {
      expect(edge.provenance).not.toBe("resolver-lsp");
    }
  });
});
