/**
 * The files tool (SDD §9.5) — chunk:m3-files-tool acceptance:
 *
 *   1. an import-aware move updates every referencing import, verified
 *      across 6 fixture languages (typescript, javascript, python, java,
 *      c, c++);
 *   2. deletes are recoverable via undo (journal-backed, §7.6);
 *   3. all files operations appear in the journal.
 *
 * The bun lane runs the in-process ports (bracket-balance parse-verify,
 * passthrough formatter, in-memory journal); what this suite pins is the
 * verb resolution, the specifier rewriting, the gating, and the choke-point
 * contract of files itself. The trash-backed remove of the composed
 * WriteWorkspaceAccess (protocol/write-access) runs against a real
 * temporary workspace.
 */

import { afterEach, describe, expect, test } from "bun:test";
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInProcessPorts, type InProcessPorts } from "../src/adapters/inprocess";
import type { ResponseEnvelope } from "../src/envelope";
import { checkConfirmGrant, KEY_LENGTH, mintConfirmGrant } from "../src/jail";
import { createWriteAccess } from "../src/protocol/write-access";
import { type FilesDeps, type FilesInput, runFiles } from "../src/tools/files";
import { runUndo } from "../src/tools/undo";
import type { WriteEngineDeps, WriteReport } from "../src/tools/write-engine";

function sha256(content: string): string {
  return new Bun.CryptoHasher("sha256").update(content).digest("hex");
}

function enoent(path: string): NodeJS.ErrnoException {
  const error = new Error(`ENOENT: no such file: ${path}`) as NodeJS.ErrnoException;
  error.code = "ENOENT";
  return error;
}

interface Rig {
  readonly deps: FilesDeps;
  readonly write: WriteEngineDeps;
  readonly ports: InProcessPorts;
  readonly files: Map<string, string>;
}

/** In-process rig over seeded files, mirrored on edit-verbs.test. */
function makeRig(seed: Record<string, string>): Rig {
  const ports = createInProcessPorts();
  const files = ports.workspace.files;
  for (const [path, content] of Object.entries(seed)) files.set(path, content);
  const write: WriteEngineDeps = {
    engine: ports.engine,
    formatter: ports.formatter,
    journal: ports.journal,
    workspace: {
      read: (path) =>
        files.has(path) ? Promise.resolve(files.get(path) as string) : Promise.reject(enoent(path)),
      write: (path, content) => {
        files.set(path, content);
        return Promise.resolve();
      },
      remove: (path) => {
        files.delete(path);
        return Promise.resolve();
      },
    },
  };
  const key = Buffer.alloc(KEY_LENGTH, 7);
  const deps: FilesDeps = {
    write,
    confirm: {
      mint: (diffHash, contentHash) => mintConfirmGrant(key, diffHash, contentHash),
      check: (token) => checkConfirmGrant(key, token),
    },
    list: () => Promise.resolve([...files.keys()].map((path) => ({ path, junk: false }))),
  };
  return { deps, write, ports, files };
}

function ok(envelope: ResponseEnvelope<WriteReport>): readonly WriteReport[] {
  if (!envelope.ok) throw new Error(`expected ok envelope, got ${envelope.error.code}`);
  return envelope.results;
}

function refused(envelope: ResponseEnvelope<WriteReport>) {
  if (envelope.ok) throw new Error("expected an error envelope, got ok");
  return envelope.error as typeof envelope.error & Record<string, unknown>;
}

function run(rig: Rig, input: FilesInput): Promise<ResponseEnvelope<WriteReport>> {
  return runFiles(rig.deps, input, 1, "suite");
}

/** One language's move fixture: workspace, the move, and what must change. */
interface MoveFixture {
  readonly lang: string;
  readonly seed: Record<string, string>;
  readonly from: string;
  readonly to: string;
  /** path → substring that must appear in the rewritten importer. */
  readonly expectAfter: Record<string, string>;
  /** path → substring that must be gone from the rewritten importer. */
  readonly expectGone: Record<string, string>;
}

const MOVE_FIXTURES: readonly MoveFixture[] = [
  {
    lang: "typescript",
    seed: {
      "src/util/helper.ts": "export function helper(): number {\n  return 1;\n}\n",
      "src/app.ts":
        'import { helper } from "./util/helper";\n\nexport function run(): number {\n  return helper();\n}\n',
      "src/feature/main.ts":
        'import { helper } from "../util/helper";\n\nexport const value = helper();\n',
    },
    from: "src/util/helper.ts",
    to: "src/core/helper.ts",
    expectAfter: {
      "src/app.ts": 'from "./core/helper"',
      "src/feature/main.ts": 'from "../core/helper"',
    },
    expectGone: {
      "src/app.ts": "util/helper",
      "src/feature/main.ts": "util/helper",
    },
  },
  {
    lang: "javascript",
    seed: {
      "lib/util.js": "module.exports = { util: () => 1 };\n",
      "lib/index.js": 'const { util } = require("./util.js");\n\nmodule.exports = { util };\n',
      "app.js": 'const { util } = require("./lib/util.js");\n\nconsole.log(util());\n',
    },
    from: "lib/util.js",
    to: "lib/nested/util.js",
    expectAfter: {
      "lib/index.js": 'require("./nested/util.js")',
      "app.js": 'require("./lib/nested/util.js")',
    },
    expectGone: {
      "lib/index.js": '"./util.js"',
      "app.js": '"./lib/util.js"',
    },
  },
  {
    lang: "python",
    seed: {
      "pkg/helper.py": "def helper():\n    return 1\n",
      "app.py": "from pkg.helper import helper\n\nprint(helper())\n",
      "pkg/other.py": "import pkg.helper\n\nvalue = pkg.helper.helper()\n",
    },
    from: "pkg/helper.py",
    to: "pkg/core/helper.py",
    expectAfter: {
      "app.py": "from pkg.core.helper import helper",
      "pkg/other.py": "import pkg.core.helper",
    },
    expectGone: {
      "app.py": "from pkg.helper import",
      "pkg/other.py": "import pkg.helper\n",
    },
  },
  {
    lang: "java",
    seed: {
      "com/example/Util.java":
        "package com.example;\n\npublic class Util {\n  public static int max() {\n    return 1;\n  }\n}\n",
      "com/app/App.java":
        "package com.app;\n\nimport com.example.Util;\n\nclass App {\n  int run() {\n    return Util.max();\n  }\n}\n",
      "com/app/Calc.java":
        "package com.app;\n\nimport static com.example.Util.max;\n\nclass Calc {\n  int c() {\n    return max();\n  }\n}\n",
    },
    from: "com/example/Util.java",
    to: "com/core/Util.java",
    expectAfter: {
      "com/app/App.java": "import com.core.Util;",
      "com/app/Calc.java": "import static com.core.Util.max;",
    },
    expectGone: {
      "com/app/App.java": "com.example.Util",
      "com/app/Calc.java": "com.example.Util",
    },
  },
  {
    lang: "c",
    seed: {
      "include/util.h": "int util(void);\n",
      "src/main.c": '#include "../include/util.h"\n\nint main(void) {\n  return util();\n}\n',
      "src/other.c": '#include "include/util.h"\n\nint other(void) {\n  return util();\n}\n',
    },
    from: "include/util.h",
    to: "include/core/util.h",
    expectAfter: {
      "src/main.c": '#include "../include/core/util.h"',
      "src/other.c": '#include "include/core/util.h"',
    },
    expectGone: {
      "src/main.c": '"../include/util.h"',
      "src/other.c": '"include/util.h"',
    },
  },
  {
    lang: "cpp",
    seed: {
      "lib/vec.hpp": "struct Vec {\n  int x;\n};\n",
      "app/main.cpp": '#include "../lib/vec.hpp"\n\nint main() {\n  return Vec{1}.x;\n}\n',
    },
    from: "lib/vec.hpp",
    to: "lib/math/vec.hpp",
    expectAfter: {
      "app/main.cpp": '#include "../lib/math/vec.hpp"',
    },
    expectGone: {
      "app/main.cpp": '"../lib/vec.hpp"',
    },
  },
];

describe("acceptance: import-aware move across 6 fixture languages", () => {
  for (const fixture of MOVE_FIXTURES) {
    describe(fixture.lang, () => {
      test("move is confirm-gated with the aggregated diff and a token", async () => {
        const rig = makeRig(fixture.seed);
        const gated = await run(rig, { op: "move", from: fixture.from, to: fixture.to });
        const error = refused(gated);
        expect(error.code).toBe("CONFIRM_REQUIRED");
        expect(error["confirmToken"]).toBeString();
        // Nothing moved.
        expect(rig.files.has(fixture.from)).toBe(true);
        expect(rig.files.has(fixture.to)).toBe(false);
      });

      test("move updates every referencing import and relocates the file", async () => {
        const rig = makeRig(fixture.seed);
        const before = rig.files.get(fixture.from) as string;
        const results = ok(
          await run(rig, { op: "move", from: fixture.from, to: fixture.to, autoApply: true }),
        );
        expect(rig.files.has(fixture.from)).toBe(false);
        expect(rig.files.get(fixture.to)).toBe(before);
        for (const [path, needle] of Object.entries(fixture.expectAfter)) {
          expect(rig.files.get(path) as string).toContain(needle);
        }
        for (const [path, needle] of Object.entries(fixture.expectGone)) {
          expect(rig.files.get(path) as string).not.toContain(needle);
        }
        // Every write of the batch rides one journal op (§7.6).
        const seqs = new Set(results.map((r) => r.journalSeq));
        expect(seqs.size).toBe(1);
        expect([...seqs][0]).toBeNumber();
      });
    });
  }

  test("the confirm-token two-call flow applies the identical move", async () => {
    const fixture = MOVE_FIXTURES[0] as MoveFixture;
    const rig = makeRig(fixture.seed);
    const gated = await run(rig, { op: "move", from: fixture.from, to: fixture.to });
    const token = refused(gated)["confirmToken"] as string;
    const applied = await run(rig, {
      op: "move",
      from: fixture.from,
      to: fixture.to,
      confirm: token,
    });
    ok(applied);
    expect(rig.files.has(fixture.from)).toBe(false);
    expect(rig.files.has(fixture.to)).toBe(true);
  });

  test("the moved file's own relative imports are recomputed from its new directory", async () => {
    const rig = makeRig({
      "src/util/shared.ts": "export const shared = 1;\n",
      "src/util/helper.ts": 'import { shared } from "./shared";\n\nexport const h = shared;\n',
    });
    ok(
      await run(rig, {
        op: "move",
        from: "src/util/helper.ts",
        to: "src/core/helper.ts",
        autoApply: true,
      }),
    );
    expect(rig.files.get("src/core/helper.ts") as string).toContain('from "../util/shared"');
  });

  test("moving an already-broken file applies with diagnostics, not PARSE_REJECTED", async () => {
    // §7.5 guards NEW syntax errors; a move introduces none. The WriteOp
    // baseline seam carries the source's before-content, so the relocated
    // breakage is reported as diagnostics rather than rejected or blessed.
    const rig = makeRig({
      "src/broken.ts": "export function broken() {\n  return (1;\n}\n",
    });
    const envelope = await run(rig, {
      op: "move",
      from: "src/broken.ts",
      to: "src/moved-broken.ts",
      autoApply: true,
    });
    const results = ok(envelope);
    const created = results.find((r) => r.path === "src/moved-broken.ts");
    expect(created?.diagnostics ?? []).not.toBeEmpty();
  });

  test("STALE_ANCHOR when the source changed since it was read", async () => {
    const fixture = MOVE_FIXTURES[0] as MoveFixture;
    const rig = makeRig(fixture.seed);
    const staleHash = sha256("what the agent read earlier");
    const envelope = await run(rig, {
      op: "move",
      from: fixture.from,
      to: fixture.to,
      expectedHash: staleHash,
      autoApply: true,
    });
    const error = refused(envelope);
    expect(error.code).toBe("STALE_ANCHOR");
    expect(error.contentHash).toBe(sha256(rig.files.get(fixture.from) as string));
  });
});

describe("acceptance: deletes are recoverable via undo", () => {
  test("delete removes the file; undo restores the exact content", async () => {
    const content = "export const keep = 42;\n";
    const rig = makeRig({ "src/keep.ts": content });
    const results = ok(await run(rig, { op: "delete", from: "src/keep.ts" }));
    expect(results[0]?.action).toBe("deleted");
    expect(rig.files.has("src/keep.ts")).toBe(false);

    const undone = await runUndo(rig.write, { steps: 1 }, 1, "suite");
    if (!undone.ok) throw new Error(`undo failed: ${undone.error.code}`);
    expect(rig.files.get("src/keep.ts")).toBe(content);
  });

  test("a delete beyond the line limit is confirm-gated", async () => {
    const big = `${Array.from({ length: 250 }, (_, i) => `const x${i} = ${i};`).join("\n")}\n`;
    const rig = makeRig({ "src/big.ts": big });
    const gated = await run(rig, { op: "delete", from: "src/big.ts" });
    expect(refused(gated).code).toBe("CONFIRM_REQUIRED");
    expect(rig.files.has("src/big.ts")).toBe(true);
  });

  test("deleting a missing file answers FILE_NOT_FOUND", async () => {
    const rig = makeRig({});
    const error = refused(await run(rig, { op: "delete", from: "src/ghost.ts" }));
    expect(error.code).toBe("FILE_NOT_FOUND");
  });
});

describe("acceptance: all files operations appear in the journal", () => {
  test("create, mkdir, copy, delete and move each journal one op tagged tool:files", async () => {
    const rig = makeRig({ "src/a.ts": "export const a = 1;\n" });
    ok(await run(rig, { op: "create", to: "src/new.ts" }));
    ok(await run(rig, { op: "mkdir", to: "src/dir" }));
    ok(await run(rig, { op: "copy", from: "src/a.ts", to: "src/b.ts" }));
    ok(await run(rig, { op: "delete", from: "src/b.ts" }));
    ok(await run(rig, { op: "move", from: "src/a.ts", to: "src/moved.ts", autoApply: true }));

    const ops = await rig.ports.journal.list(0);
    expect(ops).toHaveLength(5);
    for (const op of ops) {
      expect(op.tool).toBe("files");
      expect(op.sessionId).toBe("suite");
    }
    const kinds = ops.map((op) => op.files.map((f) => `${f.kind}:${f.path}`).sort());
    expect(kinds[0]).toEqual(["create:src/new.ts"]);
    expect(kinds[1]).toEqual(["create:src/dir/.gitkeep"]);
    expect(kinds[2]).toEqual(["create:src/b.ts"]);
    expect(kinds[3]).toEqual(["delete:src/b.ts"]);
    expect(kinds[4]).toEqual(["create:src/moved.ts", "delete:src/a.ts"]);
  });
});

describe("verb semantics", () => {
  test("create is touch: an existing file is left untouched", async () => {
    const rig = makeRig({ "src/a.ts": "export const a = 1;\n" });
    const envelope = await run(rig, { op: "create", to: "src/a.ts" });
    const results = ok(envelope);
    expect(results).toHaveLength(0);
    expect(envelope.notes?.join("\n")).toContain("left untouched");
    expect(rig.files.get("src/a.ts")).toBe("export const a = 1;\n");
    expect(await rig.ports.journal.list(0)).toHaveLength(0);
  });

  test("mkdir is idempotent: the second call is a no-op", async () => {
    const rig = makeRig({});
    ok(await run(rig, { op: "mkdir", to: "src/dir" }));
    const second = await run(rig, { op: "mkdir", to: "src/dir" });
    expect(ok(second)).toHaveLength(0);
    expect(await rig.ports.journal.list(0)).toHaveLength(1);
  });

  test("copy onto an existing differing target is confirm-gated; identical is a no-op", async () => {
    const rig = makeRig({
      "src/a.ts": "export const a = 1;\n",
      "src/b.ts": "export const b = 2;\n",
      "src/same.ts": "export const a = 1;\n",
    });
    const gated = await run(rig, { op: "copy", from: "src/a.ts", to: "src/b.ts" });
    expect(refused(gated).code).toBe("CONFIRM_REQUIRED");
    expect(rig.files.get("src/b.ts")).toBe("export const b = 2;\n");

    const noop = await run(rig, { op: "copy", from: "src/a.ts", to: "src/same.ts" });
    expect(ok(noop)).toHaveLength(0);
  });

  test("move onto itself is refused as PATTERN_INVALID", async () => {
    const rig = makeRig({ "src/a.ts": "export const a = 1;\n" });
    const error = refused(await run(rig, { op: "move", from: "src/a.ts", to: "./src/a.ts" }));
    expect(error.code).toBe("PATTERN_INVALID");
  });

  test("dryRun previews the whole move and touches nothing", async () => {
    const fixture = MOVE_FIXTURES[0] as MoveFixture;
    const rig = makeRig(fixture.seed);
    const preview = await run(rig, {
      op: "move",
      from: fixture.from,
      to: fixture.to,
      dryRun: true,
    });
    const results = ok(preview);
    expect(results.length).toBeGreaterThanOrEqual(3);
    for (const row of results) expect(row.applied).toBe(false);
    expect(rig.files.has(fixture.from)).toBe(true);
    for (const path of Object.keys(fixture.expectAfter)) {
      expect(rig.files.get(path)).toBe(fixture.seed[path] as string);
    }
    expect(await rig.ports.journal.list(0)).toHaveLength(0);
  });

  test("rust module paths are rewritten (specs/010); go imports are honestly not", async () => {
    // Inverted from "rust and go imports are honestly not rewritten" when
    // chunk:rust-move-aware-files landed: a .rs move now rewrites the
    // declaring `mod` item and the crate's paths (rust-move.ts), and says so.
    const rig = makeRig({
      "src/helper.rs": "pub fn helper() -> u32 {\n    1\n}\n",
      "src/core.rs": "pub fn core() -> u32 {\n    2\n}\n",
      "src/lib.rs": "mod core;\nmod helper;\n\nuse crate::helper::helper;\n",
      "pkg/util.go": "package util\n\nfunc Util() int {\n\treturn 1\n}\n",
      "cmd/main.go": 'package main\n\nimport "example.com/pkg"\n\nfunc main() {}\n',
    });
    const moved = await run(rig, {
      op: "move",
      from: "src/helper.rs",
      to: "src/core/helper.rs",
      autoApply: true,
    });
    ok(moved);
    expect(rig.files.get("src/lib.rs")).toBe("mod core;\n\nuse crate::core::helper::helper;\n");
    expect(rig.files.get("src/core.rs")).toBe("mod helper;\n\npub fn core() -> u32 {\n    2\n}\n");
    expect((moved.notes ?? []).join("\n")).toContain(
      "rust: module crate::helper → crate::core::helper",
    );
    // Go keeps its honest claim, and the generic note no longer mentions Rust.
    const go = await run(rig, {
      op: "move",
      from: "pkg/util.go",
      to: "pkg/other/util.go",
      autoApply: true,
    });
    ok(go);
    expect(rig.files.get("cmd/main.go")).toContain('import "example.com/pkg"');
    const goNotes = (go.notes ?? []).join("\n");
    expect(goNotes).toContain("go imports name packages, not files, and are not rewritten");
    expect(goNotes.toLowerCase()).not.toContain("rust");
  });
});

describe("the composed write access: trash-backed remove (§7.6)", () => {
  let root: string | null = null;
  afterEach(() => {
    if (root !== null) rmSync(root, { recursive: true, force: true });
    root = null;
  });

  test("remove renames into .workbench/trash/ — nothing is unlinked", async () => {
    root = mkdtempSync(join(tmpdir(), "workbench-files-"));
    const stateDir = join(root, ".workbench");
    const content = "export const doomed = 1;\n";
    writeFileSync(join(root, "doomed.ts"), content, "utf8");

    const access = createWriteAccess({ root, stateDir });
    await access.remove("doomed.ts");

    expect(readdirSync(root)).not.toContain("doomed.ts");
    const trashed = readdirSync(join(stateDir, "trash"));
    expect(trashed).toHaveLength(1);
    expect(trashed[0]).toEndWith("-doomed.ts");
    expect(readFileSync(join(stateDir, "trash", trashed[0] as string), "utf8")).toBe(content);
  });

  test("remove of a protected path is refused by the jail", async () => {
    root = mkdtempSync(join(tmpdir(), "workbench-files-"));
    const access = createWriteAccess({ root, stateDir: join(root, ".workbench") });
    expect(access.remove(".workbench/journal.db")).rejects.toMatchObject({
      code: "PROTECTED_PATH",
    });
  });
});
