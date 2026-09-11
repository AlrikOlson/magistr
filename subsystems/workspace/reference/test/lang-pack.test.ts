/**
 * The gleam lang pack on the public SDK (chunk:m5-lang-pack,
 * specs/002-wasm-lang-pack — US1 + degradation edges, SC-001/SC-005).
 *
 * Chapter A runs the LIVE lane at the FFI boundary the engine child uses
 * (skip when the native artifact is absent, like sandbox-host): the real
 * carrier component read in the real sandbox, the real WasmStore grammar,
 * the real tag-query extraction, edit parse-verify through the child's own
 * handler, and Tier-1 indexing of a gleam fixture workspace. Chapter B pins
 * the boot lane's degradation ladder over scripted deps. Chapter C pins the
 * contracts: WIT drift and the zero-capability manifest.
 */

import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { loadCore, nativeArtifactKey } from "../src/adapters/core";
import {
  ChildIndexer,
  clearPackLanguages,
  handleRequest,
  handleSandboxLangPack,
  handleSandboxLangPackClassify,
  languageForPath,
  registerPackLanguage,
} from "../src/adapters/engine-ipc";
import {
  bootLangPacks,
  GLEAM_PACK_NAME,
  type LangPackBootDeps,
} from "../src/protocol/lang-pack-plugin";
import { type InstalledPluginRecord, parsePluginManifest } from "../src/tools/plugins";

const ROOT = path.join(import.meta.dir, "..");
const PLUGIN_DIR = path.join(ROOT, "plugins", "lang-pack-gleam");
const GUEST = path.join(PLUGIN_DIR, "guest.wasm");
const POLICY = { fuel: 200_000_000, memoryCapBytes: 32 * 1024 * 1024 };

const FIXTURE = [
  "import gleam/io",
  "",
  "pub fn main() {",
  '  io.println(greet("world"))',
  "}",
  "",
  "fn greet(name: String) -> String {",
  '  "hello"',
  "}",
  "",
  "pub type Season {",
  "  Spring",
  "  Summer",
  "}",
  "",
  "pub const answer = 42",
  "",
].join("\n");

const nativeArtifact = path.join(
  ROOT,
  "crates",
  "workbench-core",
  "artifacts",
  `workbench-core.${nativeArtifactKey()}.node`,
);

describe.if(existsSync(nativeArtifact))("live lane: registry + child handlers", () => {
  const { binding } = loadCore({ force: "native" });

  const cleanup = (): void => {
    binding.langPackClear?.();
    clearPackLanguages();
  };
  afterEach(cleanup);

  const boot = async (blessed = false) => {
    const outcome = await handleSandboxLangPack(binding, {
      wasmPath: GUEST,
      blessed,
      capabilities: [],
      ...POLICY,
    });
    expect(outcome.outcome).toBe("ok");
    const registered = outcome.registered;
    if (registered === undefined || registered === null) {
      throw new Error(`pack did not register: ${outcome.detail}`);
    }
    return registered;
  };

  test("the carrier registers gleam on the wasm lane with zero capabilities", async () => {
    const registered = await boot();
    expect(registered.id).toBe("gleam");
    expect(registered.extensions).toEqual(["gleam"]);
    expect(registered.lane).toBe("wasm");
    expect(registered.queriesOk).toBe(true);
    expect(binding.languages()).toContain("gleam");
    expect(languageForPath("src/a.gleam")).toBe("gleam");
  });

  test("parse-verify: a syntax-breaking gleam edit is rejected in-call (FR-004)", async () => {
    await boot();
    const broken = handleRequest(binding, "parse", {
      path: "src/broken.gleam",
      source: "pub fn broken( {\n",
    }) as { ok: boolean; parserMessage?: string };
    expect(broken.ok).toBe(false);
    expect(broken.parserMessage).toContain("gleam");

    const valid = handleRequest(binding, "parse", {
      path: "src/ok.gleam",
      source: FIXTURE,
    }) as { ok: boolean };
    expect(valid.ok).toBe(true);
  });

  test("tag-query extraction answers gleam symbols and imports (R4)", async () => {
    await boot();
    const extraction = await binding.extract("gleam", FIXTURE);
    const names = extraction.symbols.map((s) => [s.name, s.kind]);
    expect(names).toContainEqual(["main", "function"]);
    expect(names).toContainEqual(["greet", "function"]);
    expect(names).toContainEqual(["Season", "type"]);
    expect(names).toContainEqual(["answer", "const"]);
    expect(extraction.imports).toContain("gleam/io");
  });

  test("a gleam fixture workspace is Tier-1 symbol-searchable (SC-001)", async () => {
    await boot();
    const root = mkdtempSync(path.join(tmpdir(), "wb-langpack-"));
    try {
      mkdirSync(path.join(root, "src"), { recursive: true });
      writeFileSync(path.join(root, "src", "main.gleam"), FIXTURE);
      const indexer = new ChildIndexer(root, binding, "TRUNCATE");
      try {
        indexer.build();
        await indexer.settled();
        const { symbols } = await indexer.querySymbolsVerified("greet");
        expect(symbols.length).toBe(1);
        expect(symbols[0]?.kind).toBe("function");
        expect(symbols[0]?.path).toBe("src/main.gleam");
        expect(symbols[0]?.start_line).toBe(7);
      } finally {
        indexer.close();
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("pack absence degrades to lexical: no errors, no parse-verify claims (FR-009)", () => {
    // No boot: .gleam is simply not a known language.
    expect(languageForPath("src/a.gleam")).toBeUndefined();
    const verdict = handleRequest(binding, "parse", {
      path: "src/a.gleam",
      source: "pub fn broken( {\n",
    }) as { ok: boolean };
    // Nothing to parse-verify — the write path still works, lexically.
    expect(verdict.ok).toBe(true);
  });

  test("classify-file: source/test/vendor verdicts with none deferring (batched lane)", async () => {
    const outcome = await handleSandboxLangPackClassify(binding, {
      wasmPath: GUEST,
      paths: [
        "src/main.gleam",
        "test/main_test.gleam",
        "src/thing_test.gleam",
        "build/packages/dep/src/dep.gleam",
        "README.md",
      ],
      capabilities: [],
      ...POLICY,
    });
    expect(outcome.outcome).toBe("ok");
    expect(outcome.classes).toEqual(["source", "test", "test", "vendor", null]);
  });

  test("re-registration is idempotent; clear forgets the language", async () => {
    await boot();
    await boot();
    expect(binding.languages().filter((l) => l === "gleam").length).toBe(1);
    binding.langPackClear?.();
    expect(binding.languages()).not.toContain("gleam");
  });
});

describe("boot lane: degradation ladder over scripted deps", () => {
  afterEach(() => {
    clearPackLanguages();
  });

  const record = (): InstalledPluginRecord => ({
    name: GLEAM_PACK_NAME,
    source: "registry",
    tier: "verified",
    dir: "plugins/lang-pack-gleam",
  });

  const MANIFEST = `
[plugin]
publisher = "workbench"
pkg = "lang-pack-gleam"
version = "0.1.0"

[component]
path = "guest.wasm"

[[tools]]
name = "classify-file"
description = "x"
`;

  const okDeps = (overrides: Partial<LangPackBootDeps> = {}): LangPackBootDeps => ({
    inventory: () => Promise.resolve([record()]),
    readManifest: () => Promise.resolve(MANIFEST),
    componentInfo: () => Promise.resolve({ absPath: "/abs/guest.wasm", sha256: "aa" }),
    loadPack: () =>
      Promise.resolve({
        outcome: "ok" as const,
        detail: "",
        fuelUsed: 1,
        registered: {
          id: "gleam",
          extensions: ["gleam"],
          lane: "wasm",
          queriesOk: true,
          notes: [],
        },
      }),
    health: { read: () => Promise.resolve([]), append: () => Promise.resolve() },
    now: () => "2026-08-22T00:00:00Z",
    ...overrides,
  });

  test("a healthy pack boots with lane provenance (FR-007 host half)", async () => {
    const statuses = await bootLangPacks(okDeps());
    expect(statuses.length).toBe(1);
    expect(statuses[0]?.ok).toBe(true);
    expect(statuses[0]?.language).toBe("gleam");
    expect(statuses[0]?.lane).toBe("wasm");
  });

  test("non-lang-pack plugins are not touched", async () => {
    let loads = 0;
    const statuses = await bootLangPacks(
      okDeps({
        inventory: () =>
          Promise.resolve([{ name: "workbench.lsp-resolver", source: "registry" as const }]),
        loadPack: () => {
          loads += 1;
          throw new Error("must not load");
        },
      }),
    );
    expect(statuses).toEqual([]);
    expect(loads).toBe(0);
  });

  test("an unreadable manifest is a status row, never a throw", async () => {
    const statuses = await bootLangPacks(
      okDeps({ readManifest: () => Promise.reject(new Error("EACCES")) }),
    );
    expect(statuses[0]?.ok).toBe(false);
    expect(statuses[0]?.outcome).toBe("manifest-unreadable");
  });

  test("a missing component is a status row", async () => {
    const statuses = await bootLangPacks(okDeps({ componentInfo: () => Promise.resolve(null) }));
    expect(statuses[0]?.ok).toBe(false);
    expect(statuses[0]?.outcome).toBe("missing-component");
  });

  test("a failing pack breaches the breaker; a breached pack stays quiet", async () => {
    const appended: string[] = [];
    const failing = okDeps({
      loadPack: () =>
        Promise.resolve({
          outcome: "guest-trap" as const,
          detail: "boom",
          fuelUsed: 1,
          registered: null,
        }),
      health: {
        read: () => Promise.resolve([]),
        append: (r) => {
          appended.push(r.reason);
          return Promise.resolve();
        },
      },
    });
    const first = await bootLangPacks(failing);
    expect(first[0]?.ok).toBe(false);
    expect(appended).toEqual(["boom"]);

    const breakered = await bootLangPacks(
      okDeps({
        health: {
          read: () =>
            Promise.resolve([
              {
                plugin: GLEAM_PACK_NAME,
                componentHash: "aa",
                reason: "boom",
                at: "2026-08-22T00:00:00Z",
              },
            ]),
          append: () => Promise.resolve(),
        },
        loadPack: () => {
          throw new Error("a breached pack must not be re-loaded");
        },
      }),
    );
    expect(breakered[0]?.outcome).toBe("breakered");
  });

  test("a registration refusal (bundled shadowing) is a status row and a breach", async () => {
    const statuses = await bootLangPacks(
      okDeps({
        loadPack: () =>
          Promise.reject(new Error("pack language id `rust` shadows a bundled grammar; refused")),
      }),
    );
    expect(statuses[0]?.ok).toBe(false);
    expect(statuses[0]?.outcome).toBe("registration-refused");
    expect(statuses[0]?.detail).toContain("shadows a bundled grammar");
  });
});

describe("contracts", () => {
  test("the guest's deps copy of lang-pack.wit is byte-equal to the blessed wit/", async () => {
    const blessed = await readFile(path.join(ROOT, "wit", "lang-pack.wit"), "utf8");
    const shipped = await readFile(
      path.join(PLUGIN_DIR, "guest", "wit", "deps", "lang-pack", "lang-pack.wit"),
      "utf8",
    );
    expect(shipped).toBe(blessed);
  });

  test("the pack's manifest declares zero capabilities (FR-010)", async () => {
    const parsed = parsePluginManifest(
      await readFile(path.join(PLUGIN_DIR, "plugin.toml"), "utf8"),
    );
    if (!parsed.ok) throw new Error(parsed.errors.join("; "));
    expect(parsed.manifest.imports).toEqual([]);
    expect(parsed.manifest.publisher).toBe("workbench");
    expect(parsed.manifest.pkg).toBe("lang-pack-gleam");
  });

  test("registerPackLanguage never shadows a bundled extension (FR-005, TS half)", () => {
    registerPackLanguage("gloom", ["rs", "gloom"]);
    expect(languageForPath("a.rs")).toBe("rust");
    expect(languageForPath("a.gloom")).toBe("gloom");
    clearPackLanguages();
    expect(languageForPath("a.gloom")).toBeUndefined();
  });
});
