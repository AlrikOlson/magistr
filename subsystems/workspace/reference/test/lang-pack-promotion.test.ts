/**
 * The blessing/promotion path, exercised end to end (chunk:m5-lang-pack
 * acceptance 3, specs/002-wasm-lang-pack US2, SC-003).
 *
 * The tier flip is the promotion: the same pack registered unblessed runs
 * its grammar in the WasmStore (the documented 2–5x lane); blessed, the
 * engine child promotes it to native execution via the tree-sitter-gleam
 * crate it carries for exactly this (research R5). The suite pins that the
 * lane REALLY changes (observable provenance, FR-007) and that extraction
 * is identical across lanes (FR-008) — so promotion is a cost decision,
 * never a semantics decision. The blessed-without-native-grammar edge is
 * pinned in the Rust registry suite
 * (lang_registry::tests::blessing_without_a_native_grammar_stays_on_the_wasm_lane_with_a_note).
 */

import { afterEach, describe, expect, test } from "bun:test";
import { existsSync } from "node:fs";
import path from "node:path";
import { loadCore, nativeArtifactKey } from "../src/adapters/core";
import { clearPackLanguages, handleSandboxLangPack } from "../src/adapters/engine-ipc";

const ROOT = path.join(import.meta.dir, "..");
const GUEST = path.join(ROOT, "plugins", "lang-pack-gleam", "guest.wasm");
const POLICY = { fuel: 200_000_000, memoryCapBytes: 32 * 1024 * 1024 };

const FIXTURE = [
  "import gleam/io",
  "import gleam/list",
  "",
  "pub fn main() {",
  '  io.println(greet("world"))',
  "}",
  "",
  "fn greet(name: String) -> String {",
  '  "hello " <> name',
  "}",
  "",
  "pub type Shape {",
  "  Circle(radius: Float)",
  "  Square(side: Float)",
  "}",
  "",
  "pub type Alias = List(Int)",
  "",
  "pub const limit = 10",
  "",
  "// a syntax-recovering tail",
  "pub fn partial(",
  "",
].join("\n");

const nativeArtifact = path.join(
  ROOT,
  "crates",
  "workbench-core",
  "artifacts",
  `workbench-core.${nativeArtifactKey()}.node`,
);

describe.if(existsSync(nativeArtifact))("promotion: the tier flip selects the lane", () => {
  const { binding } = loadCore({ force: "native" });

  afterEach(() => {
    binding.langPackClear?.();
    clearPackLanguages();
  });

  const bootAt = async (blessed: boolean) => {
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

  test("verified/sideload → wasm lane; blessed → native lane (FR-007)", async () => {
    const unblessed = await bootAt(false);
    expect(unblessed.lane).toBe("wasm");

    const blessed = await bootAt(true);
    expect(blessed.lane).toBe("native");

    // Observable provenance survives past registration (map inventory seam).
    const snapshot = binding.langPackSnapshot?.() ?? [];
    expect(snapshot.find((r) => r.id === "gleam")?.lane).toBe("native");
  });

  test("extraction is identical across lanes over the fixture corpus (SC-003, FR-008)", async () => {
    await bootAt(false);
    const viaWasm = await binding.extract("gleam", FIXTURE);
    const wasmParse = binding.parse("gleam", FIXTURE);

    await bootAt(true);
    const viaNative = await binding.extract("gleam", FIXTURE);
    const nativeParse = binding.parse("gleam", FIXTURE);

    expect(viaWasm.symbols.length).toBeGreaterThan(3);
    expect(viaNative).toEqual(viaWasm);
    expect(nativeParse.hasError).toBe(wasmParse.hasError);
    expect(nativeParse.nodeCount).toBe(wasmParse.nodeCount);
    expect(nativeParse.errorCount).toBe(wasmParse.errorCount);
  });

  test("the flip is reversible: un-blessing returns to the wasm lane", async () => {
    await bootAt(true);
    const demoted = await bootAt(false);
    expect(demoted.lane).toBe("wasm");
    const snapshot = binding.langPackSnapshot?.() ?? [];
    expect(snapshot.find((r) => r.id === "gleam")?.lane).toBe("wasm");
  });
});
