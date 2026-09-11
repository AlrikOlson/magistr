/**
 * The Biome formatter adapter (chunk:backlog-biome-formatter, SDD §7.5).
 * Pins the chunk's acceptance with the REAL platform binary: ts/js/json come
 * back Biome-formatted, everything else passes through without a spawn, a
 * workspace biome.json is discovered from the root, and a broken binary
 * degrades through the pipeline to the verified unformatted content with a
 * note — while `format: false` (undo's flag) never formats at all.
 */

import { afterEach, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { BiomeFormatter } from "../src/adapters/biome";
import { createInProcessPorts } from "../src/adapters/inprocess";
import type { ResponseEnvelope } from "../src/envelope";
import { PORT_VERSIONS } from "../src/ports";
import {
  runWriteBatch,
  type WriteEngineDeps,
  type WriteReport,
  type WriteWorkspaceAccess,
} from "../src/tools/write-engine";

const MISSING_BINARY = join(tmpdir(), "workbench-no-such-biome.exe");

const roots: string[] = [];
function tempRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "wb-biome-"));
  roots.push(root);
  return root;
}
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function accessOf(files: Map<string, string>): WriteWorkspaceAccess {
  return {
    read: (path) => {
      const content = files.get(path);
      if (content === undefined) {
        const error = new Error(`ENOENT: ${path}`) as NodeJS.ErrnoException;
        error.code = "ENOENT";
        return Promise.reject(error);
      }
      return Promise.resolve(content);
    },
    write: (path, content) => {
      files.set(path, content);
      return Promise.resolve();
    },
    remove: (path) => {
      files.delete(path);
      return Promise.resolve();
    },
  };
}

/** In-process engine/journal, in-memory files, the REAL Biome formatter. */
function makeDeps(formatter: BiomeFormatter): {
  deps: WriteEngineDeps;
  files: Map<string, string>;
} {
  const ports = createInProcessPorts();
  const deps: WriteEngineDeps = {
    engine: ports.engine,
    formatter,
    journal: ports.journal,
    workspace: accessOf(ports.workspace.files),
  };
  return { deps, files: ports.workspace.files };
}

function ok(envelope: ResponseEnvelope<WriteReport>): readonly WriteReport[] {
  if (!envelope.ok) throw new Error(`expected ok envelope, got ${envelope.error.code}`);
  return envelope.results;
}

describe("the adapter alone", () => {
  test("carries the FormatterPort version", () => {
    const formatter = new BiomeFormatter({ root: tempRoot() });
    expect(formatter.version).toBe(PORT_VERSIONS.formatter);
  });

  test("formats ts with Biome defaults and is idempotent", async () => {
    const formatter = new BiomeFormatter({ root: tempRoot() });
    const first = await formatter.format({
      path: "src/x.ts",
      content: "const x=1\nfunction  f( ){return x}",
    });
    expect(first.changed).toBe(true);
    expect(first.content).toContain("const x = 1;");
    expect(first.content).toContain("\treturn x;");
    const second = await formatter.format({ path: "src/x.ts", content: first.content });
    expect(second.changed).toBe(false);
    expect(second.content).toBe(first.content);
  });

  test("formats json", async () => {
    const formatter = new BiomeFormatter({ root: tempRoot() });
    const result = await formatter.format({ path: "data.json", content: '{"b":1,"a":2}' });
    expect(result.changed).toBe(true);
    expect(result.content).toContain('{ "b": 1, "a": 2 }');
  });

  test("discovers the workspace biome.json from the root", async () => {
    const root = tempRoot();
    writeFileSync(
      join(root, "biome.json"),
      JSON.stringify({ formatter: { indentStyle: "space", indentWidth: 4 } }),
    );
    const formatter = new BiomeFormatter({ root });
    const result = await formatter.format({
      path: "src/x.ts",
      content: "function f(){return 1}",
    });
    expect(result.content).toContain("    return 1;");
  });

  test("uncovered languages pass through unchanged without spawning", async () => {
    // The broken binary proves no spawn happens: a spawn would throw.
    const formatter = new BiomeFormatter({ root: tempRoot(), binary: MISSING_BINARY });
    for (const path of ["main.py", "lib.rs", "notes.md", "Makefile"]) {
      const result = await formatter.format({ path, content: "x  =  1" });
      expect(result).toEqual({ content: "x  =  1", changed: false });
    }
  });

  test("a missing binary throws instead of guessing", async () => {
    const formatter = new BiomeFormatter({ root: tempRoot(), binary: MISSING_BINARY });
    expect(formatter.format({ path: "src/x.ts", content: "const x=1" })).rejects.toThrow();
  });

  test("a parse error in the content throws with Biome's exit", async () => {
    const formatter = new BiomeFormatter({ root: tempRoot() });
    expect(formatter.format({ path: "src/x.ts", content: "const x=" })).rejects.toThrow(
      /biome format exited/,
    );
  });
});

describe("through the §7.5 pipeline", () => {
  test("written ts lands Biome-formatted on disk", async () => {
    const { deps, files } = makeDeps(new BiomeFormatter({ root: tempRoot() }));
    const results = ok(
      await runWriteBatch(
        deps,
        { writes: [{ path: "src/n.ts", content: "const n=1\nfunction  g( ){return n}" }] },
        1,
      ),
    );
    expect(results[0]?.applied).toBe(true);
    const onDisk = files.get("src/n.ts");
    expect(onDisk).toContain("const n = 1;");
    expect(onDisk).toContain("\treturn n;");
  });

  test("a formatter crash falls back to the verified unformatted content with a note", async () => {
    const { deps, files } = makeDeps(
      new BiomeFormatter({ root: tempRoot(), binary: MISSING_BINARY }),
    );
    const envelope = await runWriteBatch(
      deps,
      { writes: [{ path: "src/n.ts", content: "const n=1\n" }] },
      1,
    );
    ok(envelope);
    expect(files.get("src/n.ts")).toBe("const n=1\n");
    expect(envelope.ok && envelope.notes?.some((n) => n.includes("the formatter failed"))).toBe(
      true,
    );
  });

  test("format: false (undo's flag) never formats", async () => {
    const { deps, files } = makeDeps(new BiomeFormatter({ root: tempRoot() }));
    ok(
      await runWriteBatch(
        deps,
        { writes: [{ path: "src/n.ts", content: "const n=1\n" }], format: false },
        1,
      ),
    );
    expect(files.get("src/n.ts")).toBe("const n=1\n");
  });
});
