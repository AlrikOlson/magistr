/**
 * The `script` tool (SDD §9.7, chunk:m4-script-sandbox) — typed Code Mode.
 *
 * Two layers:
 *
 *  1. A protocol/unit chapter over a FAKE guest runner: host-call servicing,
 *     error mapping, and the degraded-lane answer, runnable on any machine.
 *  2. An integration chapter over the REAL QuickJS-ng guest (vendor/qjs)
 *     through the real native binding — the chunk's three acceptance
 *     criteria live here: the 10-one-liner expressiveness check, typed
 *     sandbox kills under abuse (fuel, memory, capability), and redaction
 *     applying to script return values because they ride sealEnvelope.
 *     Skips (core-loader idiom) when the native artifact is absent.
 */

import { describe, expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { loadCore } from "../src/adapters/core/loader";
import { handleSandboxWasi, stubCoreBinding } from "../src/adapters/engine-ipc/engine-child";
import { createInProcessPorts } from "../src/adapters/inprocess";
import { type ResponseEnvelope, sealEnvelope } from "../src/envelope";
import { checkConfirmGrant, KEY_LENGTH, mintConfirmGrant } from "../src/jail";
import type {
  EngineMatch,
  EnginePort,
  EngineSearchRequest,
  ImportGraphResult,
  IndexPort,
  IndexStatus,
  OutlineQueryResult,
  ReferenceQueryResult,
  SymbolQueryResult,
} from "../src/ports";
import { PORT_VERSIONS } from "../src/ports";
import {
  runScript,
  SCRIPT_FUEL_DEFAULT,
  SCRIPT_HOST_CALLS_MAX,
  SCRIPT_MEMORY_CAP_DEFAULT,
  type ScriptDeps,
  type ScriptGuestOutcome,
  type ScriptGuestPolicy,
  type ScriptInput,
  type ScriptResult,
} from "../src/tools/script";
import type { WriteEngineDeps, WriteReport } from "../src/tools/write-engine";

const nativeArtifactsDir = path.resolve(import.meta.dirname, "../crates/workbench-core/artifacts");
const hasNative = existsSync(nativeArtifactsDir) ? readdirHasNode(nativeArtifactsDir) : false;

function readdirHasNode(dir: string): boolean {
  try {
    return require("node:fs")
      .readdirSync(dir)
      .some((f: string) => f.endsWith(".node"));
  } catch {
    return false;
  }
}

const QJS_WASM = path.resolve(import.meta.dirname, "../vendor/qjs/qjs-wasi.wasm");

function enoent(p: string): NodeJS.ErrnoException {
  const error = new Error(`ENOENT: no such file: ${p}`) as NodeJS.ErrnoException;
  error.code = "ENOENT";
  return error;
}

/** In-memory rig: fixture files, fake search/read/write deps, a guest seam. */
function makeRig(
  seed: Record<string, string>,
  runGuest: ScriptDeps["runGuest"],
): { deps: ScriptDeps; files: Map<string, string> } {
  const files = new Map(Object.entries(seed));
  const engine: EnginePort = {
    version: PORT_VERSIONS.engine,
    parse: () => Promise.resolve({ ok: true }),
    search: (request: EngineSearchRequest): Promise<readonly EngineMatch[]> => {
      const matches: EngineMatch[] = [];
      for (const p of request.paths) {
        const content = files.get(p);
        if (content === undefined) continue;
        content.split("\n").forEach((lineText, i) => {
          const start = lineText.indexOf(request.pattern);
          if (start >= 0) {
            matches.push({
              location: { path: p, line: i + 1, column: start + 1 },
              excerpt: lineText,
            });
          }
        });
      }
      return Promise.resolve(matches);
    },
    rewrite: () => Promise.reject(new Error("not under test")),
  };
  const index: IndexPort = {
    version: PORT_VERSIONS.index,
    status: (): Promise<IndexStatus> =>
      Promise.resolve({ tier: 1, generation: 1, building: false }),
    build: () => Promise.reject(new Error("not under test")),
    querySymbols: (): Promise<SymbolQueryResult> =>
      Promise.resolve({ symbols: [], freshness: "warm" }),
    queryReferences: (): Promise<ReferenceQueryResult> =>
      Promise.resolve({ edges: [], freshness: "warm" }),
    queryOutline: (): Promise<OutlineQueryResult> =>
      Promise.resolve({ files: [], freshness: "warm" }),
    queryImportGraph: (): Promise<ImportGraphResult> =>
      Promise.resolve({ fanIn: [], fanOut: [], freshness: "warm" }),
    upsertFile: () => Promise.resolve(),
    refreshFile: () => Promise.resolve(),
    removeFile: () => Promise.resolve(),
  };
  const list = () => Promise.resolve([...files.keys()].map((p) => ({ path: p, junk: false })));
  const ports = createInProcessPorts();
  const write: WriteEngineDeps = {
    engine,
    formatter: ports.formatter,
    journal: ports.journal,
    workspace: {
      read: (p) =>
        p.startsWith("..") || path.isAbsolute(p)
          ? // Mirror the live createWriteAccess throw shape: a §7.1 refusal is
            // a coded error — script.ts classifies by code, not message.
            Promise.reject(
              Object.assign(new Error(`path escapes the workspace jail (§7.1): ${p}`), {
                code: "PATH_OUTSIDE_WORKSPACE",
              }),
            )
          : files.has(p)
            ? Promise.resolve(files.get(p) as string)
            : Promise.reject(enoent(p)),
      write: (p, content) => {
        files.set(p, content);
        return Promise.resolve();
      },
      remove: (p) => {
        files.delete(p);
        return Promise.resolve();
      },
    },
  };
  const key = Buffer.alloc(KEY_LENGTH, 9);
  const deps: ScriptDeps = {
    search: {
      engine,
      index,
      workspace: { list, read: (p) => Promise.resolve(files.get(p) ?? "") },
    },
    write,
    confirm: {
      mint: (diffHash, contentHash) => mintConfirmGrant(key, diffHash, contentHash),
      check: (token) => checkConfirmGrant(key, token),
    },
    index,
    openRead: (p) => {
      if (p.startsWith("..") || path.isAbsolute(p)) {
        return Promise.resolve({
          ok: false as const,
          reason: "path escapes the workspace jail (§7.1)",
        });
      }
      return Promise.resolve({
        ok: true as const,
        access: {
          read: () =>
            files.has(p) ? Promise.resolve(files.get(p) as string) : Promise.reject(enoent(p)),
          stat: () =>
            files.has(p)
              ? Promise.resolve({ mtimeMs: 1000, size: Buffer.byteLength(files.get(p) as string) })
              : Promise.reject(enoent(p)),
        },
      });
    },
    runGuest,
    list,
  };
  return { deps, files };
}

function run(
  deps: ScriptDeps,
  input: ScriptInput,
): Promise<ResponseEnvelope<ScriptResult | WriteReport>> {
  return runScript(deps, input, 1, "suite");
}

function scriptRow(envelope: ResponseEnvelope<ScriptResult | WriteReport>): ScriptResult {
  if (!envelope.ok) throw new Error(`expected ok envelope, got ${envelope.error.code}`);
  const row = envelope.results[0];
  if (row === undefined || row.kind !== "script") throw new Error("expected a script row first");
  return row;
}

function refused(envelope: ResponseEnvelope<ScriptResult | WriteReport>) {
  if (envelope.ok) throw new Error("expected an error envelope, got ok");
  return envelope.error as typeof envelope.error & Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Chapter 1: protocol and error mapping over a fake guest (no artifact).
// ---------------------------------------------------------------------------

const FIXTURE = {
  "src/a.ts": 'import path from "node:path";\n// TODO: refactor\nexport const a = 1;\n',
  "src/b.ts":
    'import path from "node:path";\nexport const b = 2; // TODO: rename\n// TODO: split\n',
  "src/deep/c.ts": "export const c = 3;\n",
  "notes.md": "# notes\nTODO in prose\n",
  "config.json": '{"name":"fixture","count":42}\n',
};

function guestOutcome(partial: Partial<ScriptGuestOutcome>): ScriptGuestOutcome {
  return {
    outcome: "ok",
    detail: "",
    fuelUsed: 10,
    exitCode: 0,
    stdout: "",
    stderr: "",
    ...partial,
  };
}

describe("script protocol over a fake guest", () => {
  test("a rejected guest run answers ENGINE_UNAVAILABLE, typed", async () => {
    const { deps } = makeRig(FIXTURE, () =>
      Promise.reject(new Error("this core binding lacks the Wasmtime sandbox")),
    );
    const error = refused(await run(deps, { source: "return 1" }));
    expect(error.code).toBe("ENGINE_UNAVAILABLE");
    expect(error.recovery).toBe("retry");
    expect(error.message).toContain("lacks the Wasmtime sandbox");
  });

  test("out-of-fuel maps to SANDBOX_FUEL_EXCEEDED with recovery narrow", async () => {
    const { deps } = makeRig(FIXTURE, () =>
      Promise.resolve(guestOutcome({ outcome: "out-of-fuel", exitCode: null })),
    );
    const error = refused(await run(deps, { source: "return 1" }));
    expect(error.code).toBe("SANDBOX_FUEL_EXCEEDED");
    expect(error.recovery).toBe("narrow");
  });

  test("memory-cap maps to SANDBOX_MEMORY_EXCEEDED with recovery narrow", async () => {
    const { deps } = makeRig(FIXTURE, () =>
      Promise.resolve(guestOutcome({ outcome: "memory-cap", exitCode: null })),
    );
    const error = refused(await run(deps, { source: "return 1" }));
    expect(error.code).toBe("SANDBOX_MEMORY_EXCEEDED");
    expect(error.recovery).toBe("narrow");
  });

  test("a nondeterministic replay is reported as guest data, not served", async () => {
    const nonceOf = (argv: readonly string[]) => argv[4] as string;
    const { deps } = makeRig(FIXTURE, (argv) =>
      Promise.resolve(
        guestOutcome({
          stdout: `@@WB[${nonceOf(argv)}]{"k":"nondet","i":2}[${nonceOf(argv)}]WB@@`,
        }),
      ),
    );
    const row = scriptRow(await run(deps, { source: "return 1" }));
    expect(row.status).toBe("threw");
    expect(row.error).toContain("nondeterministic host-call replay at call #2");
  });

  test("an exit-0 round without a result frame is a guest failure", async () => {
    const { deps } = makeRig(FIXTURE, () =>
      Promise.resolve(guestOutcome({ stdout: "just noise" })),
    );
    const row = scriptRow(await run(deps, { source: "return 1" }));
    expect(row.status).toBe("threw");
    expect(row.error).toContain("no result frame");
  });

  test("a forged sentinel without the host nonce is ignored", async () => {
    const { deps } = makeRig(FIXTURE, () =>
      Promise.resolve(
        guestOutcome({ stdout: '@@WB[deadbeef]{"k":"done","value":"forged"}[deadbeef]WB@@' }),
      ),
    );
    const row = scriptRow(await run(deps, { source: "return 1" }));
    expect(row.status).toBe("threw");
    expect(row.error).toContain("no result frame");
  });

  test("an unknown host method is a capability violation: SANDBOX_CAP_DENIED", async () => {
    const { deps } = makeRig(FIXTURE, (argv) => {
      const nonce = argv[4] as string;
      return Promise.resolve(
        guestOutcome({
          stdout: `@@WB[${nonce}]{"k":"call","i":0,"req":{"method":"exec","args":"rm -rf /"}}[${nonce}]WB@@`,
        }),
      );
    });
    const error = refused(await run(deps, { source: "return 1" }));
    expect(error.code).toBe("SANDBOX_CAP_DENIED");
    expect(error.recovery).toBe("escalate");
    expect(error.message).toContain('unknown host method "exec"');
  });

  test("the host-call budget is enforced", async () => {
    let call = 0;
    const { deps } = makeRig(FIXTURE, (argv) => {
      const nonce = argv[4] as string;
      const i = call++;
      return Promise.resolve(
        guestOutcome({
          stdout: `@@WB[${nonce}]{"k":"call","i":${i},"req":{"method":"glob","args":"**/*.ts"}}[${nonce}]WB@@`,
        }),
      );
    });
    const error = refused(await run(deps, { source: "return 1" }));
    expect(error.code).toBe("SANDBOX_FUEL_EXCEEDED");
    expect(error.message).toContain(`${SCRIPT_HOST_CALLS_MAX} workspace calls`);
  });

  test("degraded-lane child answer (stub core) is the typed SANDBOX_UNAVAILABLE", async () => {
    const stub = stubCoreBinding();
    expect(stub.sandboxRunWasiModule).toBeUndefined();
    await expect(
      handleSandboxWasi(stub, {
        wasmPath: QJS_WASM,
        argv: ["qjs", "-e", "1"],
        fuel: 1000,
        memoryCapBytes: 65536,
      }),
    ).rejects.toMatchObject({ code: "SANDBOX_UNAVAILABLE" });
  });

  test("sandbox/wasi validates its params before touching the binding", async () => {
    const stub = stubCoreBinding();
    await expect(
      handleSandboxWasi(stub, { argv: [], fuel: 1, memoryCapBytes: 1 }),
    ).rejects.toMatchObject({ code: "MALFORMED_FRAME" });
    await expect(
      handleSandboxWasi(stub, { wasmPath: "x", argv: "no", fuel: 1, memoryCapBytes: 1 }),
    ).rejects.toMatchObject({ code: "MALFORMED_FRAME" });
    await expect(
      handleSandboxWasi(stub, { wasmPath: "x", argv: [], fuel: 0, memoryCapBytes: 1 }),
    ).rejects.toMatchObject({ code: "MALFORMED_FRAME" });
  });
});

// ---------------------------------------------------------------------------
// Chapter 2: the real QuickJS-ng guest through the real native binding.
// ---------------------------------------------------------------------------

describe.if(hasNative && existsSync(QJS_WASM))("script over the real QuickJS guest", () => {
  const { binding } = loadCore();
  const wasmBytes = new Uint8Array(readFileSync(QJS_WASM));
  const runGuest = (argv: readonly string[], policy: ScriptGuestPolicy) => {
    const runner = binding.sandboxRunWasiModule;
    if (runner === undefined) throw new Error("native binding lacks the sandbox");
    return runner(wasmBytes, [...argv], policy);
  };

  const rig = () => makeRig(FIXTURE, runGuest);

  // ---- acceptance #1: the 10-one-liner expressiveness check ---------------

  test("one-liner 1: count TODO hits across the workspace (search)", async () => {
    const { deps } = rig();
    const row = scriptRow(
      await run(deps, {
        source:
          'const r = workspace.search({query: "TODO", mode: "lexical"});\n' +
          "return r.results.reduce((n, g) => n + g.totalHits, 0);",
      }),
    );
    expect(row.status).toBe("ok");
    expect(row.value).toBe(4);
  });

  test("one-liner 2: list every TypeScript file (glob)", async () => {
    const { deps } = rig();
    const row = scriptRow(await run(deps, { source: 'return workspace.glob("**/*.ts");' }));
    expect(row.value).toEqual(["src/a.ts", "src/b.ts", "src/deep/c.ts"]);
  });

  test("one-liner 3: parse a JSON config field (read + JSON)", async () => {
    const { deps } = rig();
    const row = scriptRow(
      await run(deps, {
        source:
          'const r = workspace.read({file: "config.json"});\n' +
          'const text = r.results.filter(u => u.kind === "lines").map(u => u.text).join("");\n' +
          "return JSON.parse(text).count;",
      }),
    );
    expect(row.value).toBe(42);
  });

  test("one-liner 4: total line count across ts files (glob + read loop)", async () => {
    const { deps } = rig();
    const row = scriptRow(
      await run(deps, {
        source:
          'let total = 0;\nfor (const f of workspace.glob("**/*.ts")) {\n' +
          "  const r = workspace.read({file: f});\n" +
          '  total += r.results.filter(u => u.kind === "lines")\n' +
          '    .map(u => u.text).join("").split("\\n").filter(l => l !== "").length;\n' +
          "}\nreturn total;",
      }),
    );
    expect(row.value).toBe(7);
    expect(scriptRowRounds(row)).toBeGreaterThan(1);
  });

  test("one-liner 5: files per extension (glob + reduce)", async () => {
    const { deps } = rig();
    const row = scriptRow(
      await run(deps, {
        source:
          'const byExt = {};\nfor (const f of workspace.glob("**/*")) {\n' +
          '  const ext = f.split(".").pop();\n  byExt[ext] = (byExt[ext] ?? 0) + 1;\n}\n' +
          "return byExt;",
      }),
    );
    expect(row.value).toEqual({ ts: 3, md: 1, json: 1 });
  });

  test("one-liner 6: which files import node:path (search + map)", async () => {
    const { deps } = rig();
    const row = scriptRow(
      await run(deps, {
        source:
          'return workspace.search({query: "node:path", mode: "lexical"}).results.map(g => g.file).sort();',
      }),
    );
    expect(row.value).toEqual(["src/a.ts", "src/b.ts"]);
  });

  test("one-liner 7: hit lines for a needle, grouped (search hit detail)", async () => {
    const { deps } = rig();
    const row = scriptRow(
      await run(deps, {
        source:
          'const g = workspace.search({query: "export const", mode: "lexical"}).results;\n' +
          "return g.map(x => [x.file, x.hits.map(h => h.line)]);",
      }),
    );
    expect(row.value).toEqual([
      ["src/a.ts", [3]],
      ["src/b.ts", [2]],
      ["src/deep/c.ts", [1]],
    ]);
  });

  test("one-liner 8: conditional batch edit queue over search hits (dry-run diffs)", async () => {
    const { deps } = rig();
    const envelope = await run(deps, {
      source:
        'for (const g of workspace.search({query: "TODO", mode: "lexical"}).results) {\n' +
        '  if (g.file.endsWith(".ts")) workspace.edit.queue({file: g.file, old: "TODO", new: "DONE"});\n' +
        "}\nreturn true;",
    });
    const row = scriptRow(envelope);
    expect(row.status).toBe("ok");
    expect(row.queuedWrites).toBe(2);
    if (!envelope.ok) throw new Error("expected ok");
    const writes = envelope.results.filter((r): r is WriteReport => r.kind === "write");
    expect(writes.map((w) => w.path).sort()).toEqual(["src/a.ts", "src/b.ts"]);
    for (const w of writes) {
      expect(w.applied).toBe(false); // dryRun default never applies (§9.7)
      expect(w.diff).toContain("DONE");
    }
  });

  test("one-liner 9: plan new files (files.queue create + mkdir preview)", async () => {
    const { deps, files } = rig();
    const envelope = await run(deps, {
      source:
        'workspace.files.queue({op: "create", to: "src/generated/marker.txt"});\n' +
        'workspace.files.queue({op: "mkdir", to: "src/new-area"});\n' +
        'return "planned";',
    });
    const row = scriptRow(envelope);
    expect(row.value).toBe("planned");
    if (!envelope.ok) throw new Error("expected ok");
    const paths = envelope.results
      .filter((r) => r.kind === "write")
      .map((r) => (r as WriteReport).path);
    expect(paths.sort()).toEqual(["src/generated/marker.txt", "src/new-area/.gitkeep"]);
    expect(files.has("src/generated/marker.txt")).toBe(false); // preview only
  });

  test("one-liner 10: read-then-compute join across two files", async () => {
    const { deps } = rig();
    const row = scriptRow(
      await run(deps, {
        source:
          'const names = workspace.glob("**/*.ts").map(f => {\n' +
          "  const r = workspace.read({file: f});\n" +
          '  const text = r.results.filter(u => u.kind === "lines").map(u => u.text).join("");\n' +
          "  const m = text.match(/export const (\\w+)/);\n  return m ? m[1] : null;\n" +
          "}).filter(Boolean);\n" +
          'return names.join(",");',
      }),
    );
    expect(row.value).toBe("a,b,c");
  });

  // ---- acceptance #2: typed kills under abuse -----------------------------

  test("abuse: a CPU spin dies typed with SANDBOX_FUEL_EXCEEDED", async () => {
    const { deps } = rig();
    const error = refused(await run(deps, { source: "for(;;){}", fuel: 50_000_000 }));
    expect(error.code).toBe("SANDBOX_FUEL_EXCEEDED");
    expect(error.recovery).toBe("narrow");
  });

  test("abuse: a memory bomb dies typed with SANDBOX_MEMORY_EXCEEDED", async () => {
    const { deps } = rig();
    const error = refused(
      await run(deps, {
        source: "const a = []; for(;;) a.push(new ArrayBuffer(1 << 20));",
        memoryCapBytes: 64 * 1024 * 1024,
      }),
    );
    expect(error.code).toBe("SANDBOX_MEMORY_EXCEEDED");
    expect(error.recovery).toBe("narrow");
  });

  test("abuse: a jail-escaping read is SANDBOX_CAP_DENIED", async () => {
    const { deps } = rig();
    const error = refused(
      await run(deps, { source: 'return workspace.read({file: "../../etc/passwd"});' }),
    );
    expect(error.code).toBe("SANDBOX_CAP_DENIED");
    expect(error.recovery).toBe("escalate");
    expect(error.message).toContain("jail");
  });

  test("abuse: a jail-escaping queued write is SANDBOX_CAP_DENIED", async () => {
    const { deps } = rig();
    const error = refused(
      await run(deps, {
        source: 'workspace.edit.queue({file: "../outside.txt", old: "a", new: "b"}); return 1;',
      }),
    );
    expect(error.code).toBe("SANDBOX_CAP_DENIED");
  });

  test("a coded jail refusal whose message avoids jail words is still SANDBOX_CAP_DENIED", async () => {
    // Pins chunk:backlog-script-load-jail-regex: resolveQueue's pre-write read
    // classifies refusals by error code, so a coded refusal whose message
    // contains none of jail/outside/escap cannot fall through to FILE_NOT_FOUND.
    const { deps } = rig();
    const patched: ScriptDeps = {
      ...deps,
      write: {
        ...deps.write,
        workspace: {
          ...deps.write.workspace,
          read: () =>
            Promise.reject(
              Object.assign(new Error("write access refused for src/app.ts"), {
                code: "PROTECTED_PATH",
              }),
            ),
        },
      },
    };
    const error = refused(
      await run(patched, {
        source: 'workspace.edit.queue({file: "src/app.ts", old: "a", new: "b"}); return 1;',
      }),
    );
    expect(error.code).toBe("SANDBOX_CAP_DENIED");
    expect(error.message).toContain("write access refused");
  });

  test("caps clamp downward only: an inflated request still runs under defaults", async () => {
    const { deps } = rig();
    const row = scriptRow(
      await run(deps, {
        source: "return 7;",
        fuel: SCRIPT_FUEL_DEFAULT * 1000,
        memoryCapBytes: SCRIPT_MEMORY_CAP_DEFAULT * 1000,
      }),
    );
    expect(row.value).toBe(7);
    expect(row.fuelUsed).toBeLessThan(SCRIPT_FUEL_DEFAULT);
  });

  // ---- acceptance #3: redaction applies to script return values ----------

  test("a secret-bearing return value is redacted at the envelope seal", async () => {
    const { deps } = rig();
    const envelope = await run(deps, {
      source: 'return {key: "AKIA" + "IOSFODNN7EXAMPLE", note: "leaked"};',
    });
    const row = scriptRow(envelope);
    expect(JSON.stringify(row.value)).toContain("AKIAIOSFODNN7EXAMPLE"); // pre-seal
    const sealed = sealEnvelope(envelope);
    expect(sealed.json).not.toContain("AKIAIOSFODNN7EXAMPLE");
    expect(sealed.json).toContain("[REDACTED:");
  });

  // ---- guest failure is data; the write path is exactly edit's ------------

  test("a thrown script is data (status threw), and its queue is discarded", async () => {
    const { deps, files } = rig();
    const envelope = await run(deps, {
      source:
        'workspace.edit.queue({file: "src/a.ts", old: "TODO", new: "GONE"});\n' +
        'throw new Error("halfway failure");',
    });
    const row = scriptRow(envelope);
    expect(row.status).toBe("threw");
    expect(row.error).toContain("halfway failure");
    expect(files.get("src/a.ts")).toContain("TODO");
    expect(envelope.ok && envelope.results.some((r) => r.kind === "write")).toBe(false);
  });

  test("console output is captured and returned, sentinel-free", async () => {
    const { deps } = rig();
    const row = scriptRow(await run(deps, { source: 'print("progress line"); return "done";' }));
    expect(row.stdout).toContain("progress line");
    expect(row.stdout).not.toContain("@@WB[");
  });

  test("applying is always confirm-gated; the token flow applies the queue", async () => {
    const { deps, files } = rig();
    const source =
      'workspace.edit.queue({file: "src/a.ts", old: "TODO: refactor", new: "DONE: refactored"});\n' +
      "return 'queued';";
    const gated = await run(deps, { source, dryRun: false });
    const error = refused(gated);
    expect(error.code).toBe("CONFIRM_REQUIRED");
    expect(typeof error["confirmToken"]).toBe("string");
    expect((gated.ok ? [] : (gated.notes ?? [])).join("\n")).toContain("script return value");

    const applied = await run(deps, {
      source,
      dryRun: false,
      confirm: error["confirmToken"] as string,
    });
    const row = scriptRow(applied);
    expect(row.value).toBe("queued");
    if (!applied.ok) throw new Error("expected ok");
    const write = applied.results.find((r) => r.kind === "write") as WriteReport;
    expect(write.applied).toBe(true);
    expect(files.get("src/a.ts")).toContain("DONE: refactored");
  });

  test("a stale confirm fails closed with STALE_ANCHOR", async () => {
    const { deps, files } = rig();
    const source = 'workspace.edit.queue({file: "src/a.ts", old: "TODO", new: "DONE"}); return 1;';
    const gated = refused(await run(deps, { source, dryRun: false }));
    files.set("src/a.ts", "changed underneath\nTODO\n");
    const stale = refused(
      await run(deps, { source, dryRun: false, confirm: gated["confirmToken"] as string }),
    );
    expect(stale.code).toBe("STALE_ANCHOR");
  });

  test("delete queues resolve to journal-backed removals (preview shows deletion)", async () => {
    const { deps } = rig();
    const envelope = await run(deps, {
      source: 'workspace.files.queue({op: "delete", from: "notes.md"}); return "ok";',
    });
    if (!envelope.ok) throw new Error("expected ok");
    const write = envelope.results.find((r) => r.kind === "write") as WriteReport;
    expect(write.path).toBe("notes.md");
    expect(write.action).toBe("deleted");
    expect(write.applied).toBe(false);
  });
});

function scriptRowRounds(row: ScriptResult): number {
  return row.rounds;
}
