/**
 * The plugin TCK (SDD §8.5, chunk:m4-tck). Pins the chunk's acceptance:
 *
 *   1. The TCK covers interface conformance, capability-violation probes and
 *      fuel/memory abuse — four typed chapters in one report.
 *   2. The language-pack chapter includes a deterministic fuzz-corpus run.
 *   3. The SDK's own example plugins (examples/plugins/) pass unmodified.
 *
 * Two chapters here: the kit's behaviour over a scripted fake invoke lane
 * (report shape, chapter applicability, every red case lands in the right
 * chapter), and the kit end to end over the real native binding — where the
 * example plugins must be green and the kit's own abuse fixtures must
 * actually die typed inside the sandbox.
 */

import { describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { defaultArtifactsDir, loadCore, nativeArtifactKey } from "../src/adapters/core/loader";
import type { PluginInvokeOutcome, TckDeps, TckReport } from "../src/tools";
import { runPluginTck, TCK_FUZZ_ITERATIONS_DEFAULT } from "../src/tools";

const GOOD_TOML = `
[plugin]
publisher = "acme"
pkg = "hello"
version = "0.1.0"

[component]
path = "guest.wasm"

[[tools]]
name = "greet"
description = "Greets the payload"

[limits]
fuel = 100000
memory_bytes = 1048576
`;

const LANG_PACK_TOML = GOOD_TOML.replace('name = "greet"', 'name = "classify-file"');

const DECLARES_OBSERVE_TOML = `${GOOD_TOML}
[capabilities]
imports = ["observe"]
`;

/** A throwaway plugin directory holding a manifest and a component file. */
function pluginDir(init: { toml?: string; component?: string | null }): string {
  const dir = mkdtempSync(path.join(tmpdir(), "workbench-tck-test-"));
  if (init.toml !== undefined) writeFileSync(path.join(dir, "plugin.toml"), init.toml);
  if (init.component !== null) {
    writeFileSync(path.join(dir, "guest.wasm"), init.component ?? "(component)");
  }
  return dir;
}

const OK: PluginInvokeOutcome = { outcome: "ok", detail: "", fuelUsed: 10, result: '{"a":1}' };

interface FakeCall {
  readonly absPath: string;
  readonly payload: string;
  readonly capabilities: readonly string[];
  readonly policy: { fuel: number; memoryCapBytes: number };
}

/**
 * A scripted invoke lane. The handler decides the outcome per call; the kit's
 * own abuse fixtures are recognized by file name and answer their expected
 * kills unless the script overrides them.
 */
function fakeLane(
  handler: (call: FakeCall) => PluginInvokeOutcome,
  opts?: { brokenSandbox?: boolean },
): { deps: TckDeps; calls: FakeCall[] } {
  const calls: FakeCall[] = [];
  return {
    calls,
    deps: {
      invokeGuest: (absPath, _exportName, payload, capabilities, policy) => {
        const call: FakeCall = { absPath, payload, capabilities, policy };
        calls.push(call);
        if (!opts?.brokenSandbox) {
          if (absPath.includes("tck-spin")) {
            return Promise.resolve({
              outcome: "out-of-fuel",
              detail: "",
              fuelUsed: 0,
              result: null,
            });
          }
          if (absPath.includes("tck-grow")) {
            return Promise.resolve({
              outcome: "memory-cap",
              detail: "",
              fuelUsed: 0,
              result: null,
            });
          }
        }
        return Promise.resolve(handler(call));
      },
    },
  };
}

function chapterById(report: TckReport, id: string) {
  const found = report.chapters.find((c) => c.id === id);
  if (found === undefined) throw new Error(`no chapter ${id}`);
  return found;
}

describe("TCK report shape and chapter applicability", () => {
  test("a conforming tool plugin is green in three chapters; lang-pack is inapplicable", async () => {
    const { deps } = fakeLane(() => OK);
    const report = await runPluginTck(pluginDir({ toml: GOOD_TOML }), deps);
    expect(report.plugin).toBe("acme.hello");
    expect(report.passed).toBe(true);
    expect(report.chapters.map((c) => c.id)).toEqual([
      "interface",
      "capabilities",
      "resources",
      "lang-pack-fuzz",
    ]);
    expect(chapterById(report, "lang-pack-fuzz").applicable).toBe(false);
    for (const id of ["interface", "capabilities", "resources"]) {
      expect(chapterById(report, id).applicable).toBe(true);
      expect(chapterById(report, id).passed).toBe(true);
    }
  });

  test("a missing manifest fails the interface chapter and leaves the rest inapplicable", async () => {
    const { deps, calls } = fakeLane(() => OK);
    const report = await runPluginTck(pluginDir({ component: "(component)" }), deps);
    expect(report.plugin).toBeNull();
    expect(report.passed).toBe(false);
    expect(chapterById(report, "interface").passed).toBe(false);
    expect(chapterById(report, "capabilities").applicable).toBe(false);
    expect(chapterById(report, "resources").applicable).toBe(false);
    expect(calls).toHaveLength(0);
  });

  test("a missing component fails interface without ever touching the invoke lane", async () => {
    const { deps, calls } = fakeLane(() => OK);
    const report = await runPluginTck(pluginDir({ toml: GOOD_TOML, component: null }), deps);
    expect(report.passed).toBe(false);
    const probes = chapterById(report, "interface").probes;
    expect(probes.find((p) => p.id === "component-present")?.passed).toBe(false);
    expect(calls).toHaveLength(0);
  });
});

describe("TCK capability-violation probes", () => {
  test("under-declaration: cap-denied under the full declared grant set is a conformance failure", async () => {
    // The component secretly needs "observe"; the manifest declares nothing.
    const { deps } = fakeLane((call) =>
      call.capabilities.includes("observe")
        ? OK
        : { outcome: "cap-denied", detail: "observe", fuelUsed: 0, result: null },
    );
    const report = await runPluginTck(pluginDir({ toml: GOOD_TOML }), deps);
    expect(report.passed).toBe(false);
    const capabilities = chapterById(report, "capabilities");
    expect(capabilities.passed).toBe(false);
    const sufficiency = capabilities.probes.find((p) => p.id === "declared-imports-suffice");
    expect(sufficiency?.passed).toBe(false);
    expect(sufficiency?.detail).toContain("does not declare");
  });

  test("over-declaration: declared imports the component never uses fail least-privilege", async () => {
    // Declares "observe" but runs happily with zero grants.
    const { deps } = fakeLane(() => OK);
    const report = await runPluginTck(pluginDir({ toml: DECLARES_OBSERVE_TOML }), deps);
    expect(report.passed).toBe(false);
    const probe = chapterById(report, "capabilities").probes.find(
      (p) => p.id === "least-privilege",
    );
    expect(probe?.passed).toBe(false);
    expect(probe?.detail).toContain("does not use");
  });

  test("honest declaration: needs exactly what it declares, denied only under zero grants", async () => {
    const { deps } = fakeLane((call) =>
      call.capabilities.includes("observe")
        ? OK
        : { outcome: "cap-denied", detail: "observe", fuelUsed: 0, result: null },
    );
    const report = await runPluginTck(pluginDir({ toml: DECLARES_OBSERVE_TOML }), deps);
    const capabilities = chapterById(report, "capabilities");
    expect(capabilities.passed).toBe(true);
    expect(capabilities.probes.find((p) => p.id === "least-privilege")?.detail).toContain(
      "denied before instantiation",
    );
  });
});

describe("TCK fuel/memory abuse chapter", () => {
  test("a trapping tool fails interface, and starvation probes accept only typed ends", async () => {
    const trap: PluginInvokeOutcome = {
      outcome: "guest-trap",
      detail: "unreachable",
      fuelUsed: 5,
      result: null,
    };
    const { deps } = fakeLane(() => trap);
    const report = await runPluginTck(pluginDir({ toml: GOOD_TOML }), deps);
    expect(report.passed).toBe(false);
    expect(chapterById(report, "interface").passed).toBe(false);
    const starvation = chapterById(report, "resources").probes.find(
      (p) => p.id === "tool-greet-starvation-fuel",
    );
    // Trapping under starvation is not in {ok, out-of-fuel}: red.
    expect(starvation?.passed).toBe(false);
  });

  test("the kit's own abuse fixtures must die typed — a sandbox that ignores policy is red", async () => {
    const { deps } = fakeLane(() => OK, { brokenSandbox: true });
    const report = await runPluginTck(pluginDir({ toml: GOOD_TOML }), deps);
    const resources = chapterById(report, "resources");
    expect(resources.passed).toBe(false);
    expect(resources.probes.find((p) => p.id === "environment-fuel-kill")?.passed).toBe(false);
    expect(resources.probes.find((p) => p.id === "environment-memory-kill")?.passed).toBe(false);
  });

  test("the abuse fixtures really reach the invoke lane, under kit-owned policy", async () => {
    const { deps, calls } = fakeLane(() => OK);
    await runPluginTck(pluginDir({ toml: GOOD_TOML }), deps);
    const spin = calls.find((c) => c.absPath.includes("tck-spin"));
    const grow = calls.find((c) => c.absPath.includes("tck-grow"));
    expect(spin?.policy.fuel).toBe(100_000);
    expect(grow?.policy.memoryCapBytes).toBe(4 * 64 * 1024);
  });
});

describe("TCK lang-pack fuzz chapter", () => {
  test("applicable for a classify-file tool, deterministic, and sized seeds+iterations", async () => {
    const first = fakeLane(() => OK);
    const report = await runPluginTck(pluginDir({ toml: LANG_PACK_TOML }), first.deps);
    const fuzz = chapterById(report, "lang-pack-fuzz");
    expect(fuzz.applicable).toBe(true);
    expect(fuzz.passed).toBe(true);
    expect(fuzz.probes[0]?.detail).toContain("all ok");

    // Deterministic: a second run sends byte-identical fuzz payloads.
    const second = fakeLane(() => OK);
    await runPluginTck(pluginDir({ toml: LANG_PACK_TOML }), second.deps);
    const fuzzPayloads = (calls: readonly FakeCall[]) =>
      calls.map((c) => c.payload).filter((p) => p.includes('"classify-file"'));
    expect(fuzzPayloads(second.calls)).toEqual(fuzzPayloads(first.calls));
    // Seeds + default iterations, plus the interface/capability/resource
    // probes that also address the tool by name.
    expect(fuzzPayloads(first.calls).length).toBeGreaterThanOrEqual(
      TCK_FUZZ_ITERATIONS_DEFAULT + 9,
    );
  });

  test("one trapping corpus entry fails the chapter and names the entry", async () => {
    let fuzzCalls = 0;
    const { deps } = fakeLane((call) => {
      if (!call.payload.includes('"classify-file"')) return OK;
      fuzzCalls++;
      // The interface/capability/resource probes use {} args; corpus entries
      // carry a path. Trap on the 5th corpus-shaped payload.
      return fuzzCalls === 20
        ? { outcome: "guest-trap", detail: "boom", fuelUsed: 1, result: null }
        : OK;
    });
    const report = await runPluginTck(pluginDir({ toml: LANG_PACK_TOML }), deps);
    const fuzz = chapterById(report, "lang-pack-fuzz");
    expect(fuzz.passed).toBe(false);
    expect(fuzz.probes[0]?.detail).toContain("corpus[");
    expect(report.passed).toBe(false);
  });

  test("iterations are tunable through deps.fuzz", async () => {
    const { deps, calls } = fakeLane(() => OK);
    await runPluginTck(pluginDir({ toml: LANG_PACK_TOML }), {
      ...deps,
      fuzz: { iterations: 5 },
    });
    const corpus = calls.filter(
      (c) => c.payload.includes('"classify-file"') && c.payload.includes('"path"'),
    );
    // 9 fixed seed cases + 5 mutations.
    expect(corpus).toHaveLength(14);
  });
});

// ---- End to end over the real native binding -------------------------------

const nativeArtifact = path.join(
  defaultArtifactsDir(),
  `workbench-core.${nativeArtifactKey()}.node`,
);

const EXAMPLES = path.join(import.meta.dir, "..", "examples", "plugins");

/** Requires an import no grant covers, and has no string-shaped export. */
const NEEDY_TOOL_WAT = `
(component
  (import "observe" (func $obs (param "n" u32) (result u32)))
  (core func $obs_core (canon lower (func $obs)))
  (core module $m
    (import "cap" "observe" (func $obs (param i32) (result i32)))
    (func (export "run") (result i32)
      (call $obs (i32.const 7))))
  (core instance $i (instantiate $m (with "cap" (instance (export "observe" (func $obs_core))))))
  (func (export "invoke") (result u32) (canon lift (core func $i "run")))
)
`;

describe.if(existsSync(nativeArtifact))("TCK over the real native binding", () => {
  const { binding } = loadCore({ force: "native" });
  const deps: TckDeps = {
    invokeGuest: async (absPath, exportName, payload, capabilities, policy) => {
      if (binding.sandboxInvokeComponent === undefined) {
        throw new Error("native binding lacks the sandbox host");
      }
      const bytes = new Uint8Array(await readFile(absPath));
      return binding.sandboxInvokeComponent(bytes, exportName, payload, [...capabilities], policy);
    },
  };

  // Acceptance 3 — the SDK's own examples pass the TCK unmodified.
  test("example plugin echo passes every applicable chapter", async () => {
    const report = await runPluginTck(path.join(EXAMPLES, "echo"), deps);
    expect(report.plugin).toBe("workbench.example-echo");
    expect(report.passed).toBe(true);
    expect(chapterById(report, "lang-pack-fuzz").applicable).toBe(false);
    // The environment probes really killed the kit's abusive guests.
    const resources = chapterById(report, "resources");
    expect(resources.probes.find((p) => p.id === "environment-fuel-kill")?.passed).toBe(true);
    expect(resources.probes.find((p) => p.id === "environment-memory-kill")?.passed).toBe(true);
  }, 30_000);

  // M5 gate criterion (chunk:m5-lsp-resolver-plugin, spec SC-002): the first
  // first-party plugin passes the TCK UNMODIFIED. Its subprocess import makes
  // both capability probes real: declared-sufficiency links the host-defined
  // interface, least-privilege is cap-denied before instantiation.
  test("bundled lsp-resolver plugin passes every applicable chapter unmodified", async () => {
    const report = await runPluginTck(
      path.join(EXAMPLES, "..", "..", "plugins", "lsp-resolver"),
      deps,
    );
    expect(report.plugin).toBe("workbench.lsp-resolver");
    expect(report.passed).toBe(true);
    expect(chapterById(report, "interface").passed).toBe(true);
    expect(chapterById(report, "capabilities").passed).toBe(true);
    expect(chapterById(report, "resources").passed).toBe(true);
    expect(chapterById(report, "lang-pack-fuzz").applicable).toBe(false);
  }, 60_000);

  // M5 gate criterion (chunk:m5-runner-adapter, chunk acceptance 2): the
  // bundled go runner adapter passes the TCK unmodified. Zero declared
  // imports make least-privilege trivially honest.
  test("bundled runner-go plugin passes every applicable chapter unmodified", async () => {
    const report = await runPluginTck(
      path.join(EXAMPLES, "..", "..", "plugins", "runner-go"),
      deps,
    );
    expect(report.plugin).toBe("workbench.runner-go");
    expect(report.passed).toBe(true);
    expect(chapterById(report, "interface").passed).toBe(true);
    expect(chapterById(report, "capabilities").passed).toBe(true);
    expect(chapterById(report, "resources").passed).toBe(true);
    expect(chapterById(report, "lang-pack-fuzz").applicable).toBe(false);
  }, 60_000);

  // M5 gate criterion (chunk:m5-lang-pack, spec SC-004): the bundled gleam
  // lang pack passes the TCK unmodified. Zero declared imports make
  // least-privilege trivially honest, and its `classify-file` tool makes the
  // lang-pack fuzz chapter applicable for the first time on a REAL pack.
  test("bundled lang-pack-gleam plugin passes every applicable chapter unmodified", async () => {
    const report = await runPluginTck(
      path.join(EXAMPLES, "..", "..", "plugins", "lang-pack-gleam"),
      deps,
    );
    expect(report.plugin).toBe("workbench.lang-pack-gleam");
    expect(report.passed).toBe(true);
    expect(chapterById(report, "interface").passed).toBe(true);
    expect(chapterById(report, "capabilities").passed).toBe(true);
    expect(chapterById(report, "resources").passed).toBe(true);
    const fuzz = chapterById(report, "lang-pack-fuzz");
    expect(fuzz.applicable).toBe(true);
    expect(fuzz.passed).toBe(true);
  }, 60_000);

  // M5 gate criterion (chunk:m5-ranking-shadow, spec SC-004): the bundled
  // visibility ranking signal passes the TCK unmodified. Zero declared
  // imports make least-privilege trivially honest on the tightest-leash
  // surface — a ranking signal is a pure function from per-hit facts to
  // named feature values.
  test("bundled ranking-visibility plugin passes every applicable chapter unmodified", async () => {
    const report = await runPluginTck(
      path.join(EXAMPLES, "..", "..", "plugins", "ranking-visibility"),
      deps,
    );
    expect(report.plugin).toBe("workbench.ranking-visibility");
    expect(report.passed).toBe(true);
    expect(chapterById(report, "interface").passed).toBe(true);
    expect(chapterById(report, "capabilities").passed).toBe(true);
    expect(chapterById(report, "resources").passed).toBe(true);
    expect(chapterById(report, "lang-pack-fuzz").applicable).toBe(false);
  }, 60_000);

  // Acceptance 2 — the lang-pack chapter's fuzz corpus runs for real.
  test("example lang pack passes, fuzz corpus included", async () => {
    const report = await runPluginTck(path.join(EXAMPLES, "lang-pack-example"), deps);
    expect(report.plugin).toBe("workbench.example-lang-pack");
    expect(report.passed).toBe(true);
    const fuzz = chapterById(report, "lang-pack-fuzz");
    expect(fuzz.applicable).toBe(true);
    expect(fuzz.passed).toBe(true);
    expect(fuzz.probes[0]?.detail).toContain("all ok");
  }, 60_000);

  // Acceptance 1 — a real capability violation lands in the right chapter.
  test("a component needing an undeclared import fails capabilities; an honest one passes them", async () => {
    const undeclared = mkdtempSync(path.join(tmpdir(), "workbench-tck-needy-"));
    mkdirSync(undeclared, { recursive: true });
    writeFileSync(
      path.join(undeclared, "plugin.toml"),
      GOOD_TOML.replace("guest.wasm", "guest.wat"),
    );
    writeFileSync(path.join(undeclared, "guest.wat"), NEEDY_TOOL_WAT);
    const red = await runPluginTck(undeclared, deps);
    expect(red.passed).toBe(false);
    expect(
      chapterById(red, "capabilities").probes.find((p) => p.id === "declared-imports-suffice")
        ?.passed,
    ).toBe(false);

    const declared = mkdtempSync(path.join(tmpdir(), "workbench-tck-honest-"));
    writeFileSync(
      path.join(declared, "plugin.toml"),
      `${GOOD_TOML.replace("guest.wasm", "guest.wat")}\n[capabilities]\nimports = ["observe"]\n`,
    );
    writeFileSync(path.join(declared, "guest.wat"), NEEDY_TOOL_WAT);
    const honest = await runPluginTck(declared, deps);
    // Capabilities are conformant (declared exactly what it needs) even
    // though its integer-shaped export fails the interface chapter.
    expect(chapterById(honest, "capabilities").passed).toBe(true);
    expect(chapterById(honest, "interface").passed).toBe(false);
    expect(honest.passed).toBe(false);
  }, 30_000);
});
