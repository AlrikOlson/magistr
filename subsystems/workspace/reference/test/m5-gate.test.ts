/**
 * The M5 exit gate, structural (chunk:m5-gate, SDD §13 — non-negotiable).
 *
 * The gate's three criteria are pinned by SWEEPS, not lists, so they hold by
 * construction as the plugin population grows:
 *
 *   G1 — every committed first-party plugin passes the TCK unmodified. The
 *        plugin set is DISCOVERED from plugins/ (any dir carrying
 *        plugin.toml); a fifth plugin cannot dodge by not being enumerated.
 *   G2 — no first-party plugin relies on an interface a third party could
 *        not use: every vendored guest/wit/deps file is byte-equal to its
 *        public top-level wit/ original, and every WIT package a guest
 *        world references is blessed in wit/baseline.json.
 *   G3 — the blessing/promotion path has been exercised end to end: the two
 *        promotion mechanisms' suites (execution-lane tier flip, ranking
 *        application promotion) are present in the test tree and their state
 *        surfaces exist — the running evidence is those suites passing in
 *        this same `bun test` invocation.
 *
 * The per-plugin TCK tests in plugin-tck.test.ts are deliberately retained:
 * they carry chunk-specific assertions this sweep does not.
 */

import { describe, expect, test } from "bun:test";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { defaultArtifactsDir, loadCore, nativeArtifactKey } from "../src/adapters/core/loader";
import type { TckDeps } from "../src/tools";
import { runPluginTck } from "../src/tools";

const REPO = path.resolve(import.meta.dir, "..");
const PLUGINS_DIR = path.join(REPO, "plugins");
const WIT_DIR = path.join(REPO, "wit");
const nativeArtifact = path.join(
  defaultArtifactsDir(),
  `workbench-core.${nativeArtifactKey()}.node`,
);

/** Discovery, not enumeration: any plugins/ dir with a plugin.toml. */
function discoverPlugins(): string[] {
  return readdirSync(PLUGINS_DIR)
    .map((name) => path.join(PLUGINS_DIR, name))
    .filter((dir) => statSync(dir).isDirectory() && existsSync(path.join(dir, "plugin.toml")));
}

/** Every .wit file under a directory, recursively. */
function witFilesUnder(dir: string): string[] {
  if (!existsSync(dir)) return [];
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...witFilesUnder(full));
    else if (entry.name.endsWith(".wit")) out.push(full);
  }
  return out;
}

describe("M5 gate — G1: every first-party plugin passes the TCK unmodified (discovered)", () => {
  const plugins = discoverPlugins();

  test("discovery is not vacuous: the four shipped M5 plugins are present", () => {
    expect(plugins.length).toBeGreaterThanOrEqual(4);
    const names = plugins.map((p) => path.basename(p));
    for (const shipped of ["lsp-resolver", "runner-go", "lang-pack-gleam", "ranking-visibility"]) {
      expect(names).toContain(shipped);
    }
  });

  describe.if(existsSync(nativeArtifact))("over the real native binding", () => {
    const { binding } = loadCore({ force: "native" });
    const deps: TckDeps = {
      invokeGuest: async (absPath, exportName, payload, capabilities, policy) => {
        if (binding.sandboxInvokeComponent === undefined) {
          throw new Error("native binding lacks the sandbox host");
        }
        const bytes = new Uint8Array(await readFile(absPath));
        return binding.sandboxInvokeComponent(
          bytes,
          exportName,
          payload,
          [...capabilities],
          policy,
        );
      },
    };

    for (const dir of plugins) {
      test(`${path.basename(dir)} passes every applicable TCK chapter`, async () => {
        const report = await runPluginTck(dir, deps);
        expect(report.passed).toBe(true);
      }, 60_000);
    }
  });
});

describe("M5 gate — G2: no first-party plugin relies on a non-public interface", () => {
  test("every vendored wit/deps file is byte-equal to its public wit/ original", () => {
    let checked = 0;
    for (const dir of discoverPlugins()) {
      for (const vendored of witFilesUnder(path.join(dir, "guest", "wit", "deps"))) {
        const original = path.join(WIT_DIR, path.basename(vendored));
        expect(existsSync(original)).toBe(true);
        expect(readFileSync(vendored, "utf8")).toBe(readFileSync(original, "utf8"));
        checked += 1;
      }
    }
    // Non-vacuity: the four shipped plugins vendor at least one dep each.
    expect(checked).toBeGreaterThanOrEqual(4);
  });

  test("every WIT package a guest world references is blessed in the baseline", () => {
    const baseline = JSON.parse(
      readFileSync(path.join(WIT_DIR, "baseline.json"), "utf8"),
    ) as Record<string, unknown>;
    const blessed = new Set(Object.keys(baseline));
    for (const dir of discoverPlugins()) {
      for (const witFile of witFilesUnder(path.join(dir, "guest", "wit"))) {
        const source = readFileSync(witFile, "utf8");
        // Every cross-package reference (`workbench:<pkg>/...`) must name a
        // blessed package; the guest's own package id is exempt by shape.
        for (const match of source.matchAll(/workbench:([a-z][a-z0-9-]*)\//g)) {
          expect(blessed.has(`workbench:${match[1]}`)).toBe(true);
        }
      }
    }
  });
});

describe("M5 gate — G3: the blessing/promotion path is exercised end to end", () => {
  test("both promotion mechanisms' suites are in the test tree", () => {
    // Execution-lane promotion: blessed lang pack flips WasmStore -> native,
    // extraction byte-identical across lanes.
    expect(existsSync(path.join(REPO, "test", "lang-pack-promotion.test.ts"))).toBe(true);
    // Application promotion: ranking signal shadow -> promoted -> demoted,
    // componentHash-bound, restart-surviving.
    expect(existsSync(path.join(REPO, "test", "ranking-shadow.test.ts"))).toBe(true);
  });

  test("the promotion state surfaces exist where the suites pin them", () => {
    // The ranking promotion store and the tier-selecting boot lanes are the
    // named seams; a rename would silently orphan the evidence above.
    const rankingPlugin = readFileSync(
      path.join(REPO, "src", "protocol", "ranking-plugin.ts"),
      "utf8",
    );
    expect(rankingPlugin).toContain("ranking-promotions.json");
    const langPackPlugin = readFileSync(
      path.join(REPO, "src", "protocol", "lang-pack-plugin.ts"),
      "utf8",
    );
    expect(langPackPlugin).toContain('tier === "blessed"');
  });
});
