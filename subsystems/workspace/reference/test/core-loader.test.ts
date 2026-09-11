/**
 * workbench-core loader: selection logic (native → wasm fail-open) with
 * injected attempts, the self-test gate, and — when built artifacts are
 * present on this machine — the real bindings end to end under Bun.
 *
 * The real-artifact suites are per-platform proof and run wherever the
 * artifact exists (locally after `bun run build:core`, and in each CI matrix
 * lane). On lanes without artifacts they are skipped; the CI matrix is the
 * exhaustive cover.
 */

import { describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import {
  type CoreBinding,
  CoreLoadError,
  CoreSelfTestError,
  type CoreSelfTestReport,
  defaultArtifactsDir,
  loadCore,
  loadVerifiedCore,
  nativeArtifactKey,
  verifyCore,
} from "../src/adapters/core";

const greenReport: CoreSelfTestReport = {
  parseOk: true,
  searchOk: true,
  rewriteOk: true,
  allOk: true,
  details: [],
};

function fakeBinding(overrides: Partial<CoreBinding> = {}): CoreBinding {
  return {
    version: () => "0.0.0-fake",
    languages: () => ["typescript"],
    parse: () => ({
      language: "typescript",
      rootKind: "program",
      nodeCount: 1,
      errorCount: 0,
      hasError: false,
    }),
    search: () => [],
    structuralSearch: () => [],
    rewrite: () => ({ output: "", matchCount: 0 }),
    extract: () => Promise.resolve({ symbols: [], imports: [] }),
    scanWorkspace: () => Promise.resolve([]),
    selfTest: () => greenReport,
    ...overrides,
  };
}

describe("loadCore selection", () => {
  test("native success selects native and never attempts wasm", () => {
    let wasmAttempted = false;
    const loaded = loadCore({
      attempts: {
        tryNative: () => fakeBinding(),
        tryWasm: () => {
          wasmAttempted = true;
          return fakeBinding();
        },
      },
    });
    expect(loaded.flavor).toBe("native");
    expect(loaded.nativeError).toBeUndefined();
    expect(wasmAttempted).toBe(false);
  });

  test("native failure fails open to wasm and records the native error", () => {
    const loaded = loadCore({
      attempts: {
        tryNative: () => {
          throw new Error("ERR_DLOPEN_FAILED: not a valid Win32 application");
        },
        tryWasm: () => fakeBinding(),
      },
    });
    expect(loaded.flavor).toBe("wasm");
    expect(loaded.nativeError).toContain("ERR_DLOPEN_FAILED");
  });

  test("both failing throws CoreLoadError naming both causes", () => {
    expect(() =>
      loadCore({
        attempts: {
          tryNative: () => {
            throw new Error("no native");
          },
          tryWasm: () => {
            throw new Error("no wasm");
          },
        },
      }),
    ).toThrow(CoreLoadError);
    try {
      loadCore({
        attempts: {
          tryNative: () => {
            throw new Error("no native");
          },
          tryWasm: () => {
            throw new Error("no wasm");
          },
        },
      });
    } catch (error) {
      const e = error as CoreLoadError;
      expect(e.nativeError).toBe("no native");
      expect(e.wasmError).toBe("no wasm");
    }
  });

  test("force=wasm skips the native attempt entirely", () => {
    let nativeAttempted = false;
    const loaded = loadCore({
      force: "wasm",
      attempts: {
        tryNative: () => {
          nativeAttempted = true;
          return fakeBinding();
        },
        tryWasm: () => fakeBinding(),
      },
    });
    expect(loaded.flavor).toBe("wasm");
    expect(nativeAttempted).toBe(false);
  });

  test("force=native does not fall back on failure", () => {
    expect(() =>
      loadCore({
        force: "native",
        attempts: {
          tryNative: () => {
            throw new Error("boom");
          },
          tryWasm: () => fakeBinding(),
        },
      }),
    ).toThrow("boom");
  });
});

describe("self-test gate", () => {
  test("a green self-test passes through", () => {
    const loaded = loadVerifiedCore({
      attempts: { tryNative: () => fakeBinding(), tryWasm: () => fakeBinding() },
    });
    expect(loaded.flavor).toBe("native");
  });

  test("a red self-test refuses the binding", () => {
    const red: CoreSelfTestReport = {
      parseOk: true,
      searchOk: false,
      rewriteOk: true,
      allOk: false,
      details: ["search: broken"],
    };
    expect(() =>
      verifyCore({
        binding: fakeBinding({ selfTest: () => red }),
        flavor: "native",
      }),
    ).toThrow(CoreSelfTestError);
    try {
      verifyCore({ binding: fakeBinding({ selfTest: () => red }), flavor: "native" });
    } catch (error) {
      const e = error as CoreSelfTestError;
      expect(e.message).toContain("search=false");
      expect(e.message).toContain("search: broken");
    }
  });
});

describe("nativeArtifactKey", () => {
  test("maps the napi platform naming", () => {
    expect(nativeArtifactKey("win32", "x64")).toBe("win32-x64-msvc");
    expect(nativeArtifactKey("linux", "x64")).toBe("linux-x64-gnu");
    expect(nativeArtifactKey("linux", "arm64")).toBe("linux-arm64-gnu");
    expect(nativeArtifactKey("darwin", "arm64")).toBe("darwin-arm64");
  });
});

// ---- real artifacts, where present ----------------------------------------

const artifactsDir = defaultArtifactsDir();
const nativeArtifact = path.join(artifactsDir, `workbench-core.${nativeArtifactKey()}.node`);
const wasmArtifact = path.join(artifactsDir, "workbench-core.wasm32-wasi.wasm");

describe.if(existsSync(nativeArtifact))("real native binding", () => {
  test("loads under Bun and passes the full self-test", () => {
    const loaded = loadVerifiedCore({ force: "native" });
    expect(loaded.binding.languages().length).toBe(13);
    expect(loaded.binding.selfTest().allOk).toBe(true);
  });

  test("extract and scanWorkspace run on the async worker pool", async () => {
    const loaded = loadVerifiedCore({ force: "native" });
    const extraction = await loaded.binding.extract("rust", "pub fn native_probe() {}\n");
    expect(extraction.symbols.map((s) => s.name)).toContain("native_probe");

    const dir = mkdtempSync(path.join(tmpdir(), "wb-loader-scan-"));
    try {
      writeFileSync(path.join(dir, "a.rs"), "fn scanned() {}\n");
      const entries = await loaded.binding.scanWorkspace(dir);
      expect(entries.map((e) => e.path)).toContain("a.rs");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe.if(existsSync(wasmArtifact))("real wasm fallback", () => {
  test("is auto-selected when the native addon fails to load", () => {
    const loaded = loadVerifiedCore({
      attempts: {
        tryNative: () => {
          throw new Error("simulated native load failure");
        },
      },
    });
    expect(loaded.flavor).toBe("wasm");
    expect(loaded.nativeError).toContain("simulated");
    const rewritten = loaded.binding.rewrite("python", "print(1)\n", "print($A)", "log($A)");
    expect(rewritten.output).toBe("log(1)\n");
    expect(rewritten.matchCount).toBe(1);
  });

  test("extract works over the async pool; scanWorkspace settles", async () => {
    const loaded = loadVerifiedCore({ force: "wasm" });
    const extraction = await loaded.binding.extract(
      "typescript",
      "export function fromWasm(): void {}\n",
    );
    expect(extraction.symbols.map((s) => s.name)).toContain("fromWasm");

    // The wasm guest has no fs preopens, so the walk is expected to reject —
    // the engine child degrades to its TS walk on that rejection. The
    // contract tested here is that the call settles instead of hanging.
    const outcome = await loaded.binding.scanWorkspace(tmpdir()).then(
      () => "resolved",
      () => "rejected",
    );
    expect(["resolved", "rejected"]).toContain(outcome);
  });
});
