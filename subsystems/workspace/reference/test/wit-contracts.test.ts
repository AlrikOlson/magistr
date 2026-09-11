/**
 * The 1:1 crosswalk between the public WIT vocabulary (wit/, SDD §8.2) and
 * the live internal vocabulary it re-exports (§6.4) — chunk:m4-wit-interfaces.
 *
 * The other half of the contract lives in
 * crates/workbench-core/tests/wit_contracts.rs, which parses the WIT with the
 * real component-model parser and enforces additive evolution against the
 * blessed wit/baseline.json. This suite proves the WIT names still mean what
 * the internal code means: enum cases against the live ranking tables, record
 * fields against the port types, surfaces against the SDD's own words.
 */

import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { RANKING_WEIGHTS } from "../src/tools/ranking";

const ROOT = join(import.meta.dir, "..");
const WIT_DIR = join(ROOT, "wit");

const EXPECTED_PACKAGES = [
  "workbench:lang-pack",
  "workbench:resolver",
  "workbench:runner",
  "workbench:tool",
  "workbench:ranking",
  // The narrow host capability reserved to blessed plugins (SDD §8.3
  // Resolvers; specs/001-lsp-resolver-plugin), stamped independently.
  "workbench:subprocess",
] as const;

const witSources = new Map(
  readdirSync(WIT_DIR)
    .filter((name) => name.endsWith(".wit"))
    .map((name) => [name, readFileSync(join(WIT_DIR, name), "utf8")] as const),
);

/** `package workbench:<name>@<semver>;` headers across all .wit files. */
function packageStamps(): Map<string, string> {
  const stamps = new Map<string, string>();
  for (const source of witSources.values()) {
    for (const match of source.matchAll(/^package\s+(workbench:[a-z-]+)@(\d+\.\d+\.\d+);/gm)) {
      const name = match[1] as string;
      const version = match[2] as string;
      stamps.set(name, version);
    }
  }
  return stamps;
}

/** Case names of `enum <name> { ... }` in one .wit file, %-escapes stripped. */
function enumCases(file: string, enumName: string): string[] {
  const source = witSources.get(file);
  if (source === undefined) throw new Error(`missing wit file ${file}`);
  const match = source.match(new RegExp(`enum ${enumName} \\{([^}]*)\\}`));
  if (match?.[1] === undefined) throw new Error(`enum ${enumName} not found in ${file}`);
  return match[1]
    .split(",")
    .map((c) => c.trim().replace(/^%/, ""))
    .filter((c) => c.length > 0);
}

/** Field names of `record <name> { ... }` in one .wit file. */
function recordFields(file: string, recordName: string): string[] {
  const source = witSources.get(file);
  if (source === undefined) throw new Error(`missing wit file ${file}`);
  const match = source.match(new RegExp(`record ${recordName} \\{([^}]*)\\}`));
  if (match?.[1] === undefined) throw new Error(`record ${recordName} not found in ${file}`);
  return match[1]
    .split("\n")
    .map((line) => line.replace(/\/\/.*$/, ""))
    .map((line) => /^\s*([%\w-]+):/.exec(line)?.[1]?.replace(/^%/, ""))
    .filter((f): f is string => f !== undefined);
}

describe("five packages, independently stamped", () => {
  test("every §8.2 package exists with its own semver stamp", () => {
    const stamps = packageStamps();
    for (const name of EXPECTED_PACKAGES) {
      expect(stamps.get(name)).toMatch(/^\d+\.\d+\.\d+$/);
    }
    expect([...stamps.keys()].sort()).toEqual([...EXPECTED_PACKAGES].sort());
  });

  test("the blessed baseline agrees with the live stamps", () => {
    const baseline = JSON.parse(readFileSync(join(WIT_DIR, "baseline.json"), "utf8")) as Record<
      string,
      { version: string }
    >;
    const stamps = packageStamps();
    expect(Object.keys(baseline).sort()).toEqual([...EXPECTED_PACKAGES].sort());
    for (const [name, pkg] of Object.entries(baseline)) {
      expect(stamps.get(name)).toBe(pkg.version);
    }
  });
});

describe("1:1 with the internal vocabulary", () => {
  test("ranking kind-class cases equal the live RANKING_WEIGHTS.kind keys", () => {
    // KindClass is a type; RANKING_WEIGHTS.kind is its runtime shadow — the
    // ONE tunable table scores exactly these classes (SDD §7.4).
    expect(enumCases("ranking.wit", "kind-class").sort()).toEqual(
      Object.keys(RANKING_WEIGHTS.kind).sort(),
    );
  });

  test("file-class cases equal the live RANKING_WEIGHTS.fileClass keys, in both packages", () => {
    const expected = Object.keys(RANKING_WEIGHTS.fileClass).sort();
    expect(enumCases("ranking.wit", "file-class").sort()).toEqual(expected);
    expect(enumCases("lang-pack.wit", "file-class").sort()).toEqual(expected);
  });

  test("resolver source-location fields equal SourceLocation (src/ports/types.ts)", () => {
    const portSource = readFileSync(join(ROOT, "src/ports/types.ts"), "utf8");
    const match = portSource.match(/export interface SourceLocation \{([^}]*)\}/);
    if (match?.[1] === undefined) throw new Error("SourceLocation not found");
    const fields = [...match[1].matchAll(/readonly (\w+):/g)].map((m) => m[1]);
    expect(recordFields("resolver.wit", "source-location")).toEqual(fields as string[]);
  });

  test("resolver exports exactly the §8.2 surface", () => {
    const source = witSources.get("resolver.wit") ?? "";
    for (const fn of ["resolve-references", "rename", "diagnostics"]) {
      expect(source).toMatch(new RegExp(`^  ${fn}: func\\(`, "m"));
    }
  });

  test("runner structured-failure is §9.6's quartet verbatim", () => {
    expect(recordFields("runner.wit", "structured-failure")).toEqual([
      "test-name",
      "file",
      "assertion",
      "trace",
    ]);
    // The SDD sentence this re-exports, so a rewording there is caught here.
    const sdd = readFileSync(join(ROOT, "workbench-mcp-sdd.md"), "utf8");
    expect(sdd).toContain("failure extraction (test name, file, assertion, trimmed trace)");
  });

  test("tool world imports the host capability interface and exports guest", () => {
    const source = witSources.get("tool.wit") ?? "";
    expect(source).toMatch(/world tool-provider \{\s*import host;\s*export guest;\s*\}/);
    // The queued-write lane exists and nothing in the interface applies writes.
    expect(source).toContain("queue-write: func(op-json: string);");
  });

  test("every port in the internal registry is spoken for by the public surfaces", async () => {
    // The §6.4 re-export is per-surface, not per-port; this pins the review
    // that mapped them so a NEW port cannot appear without a public-surface
    // decision (extend a WIT package, or record why it stays internal).
    const { PORT_VERSIONS } = await import("../src/ports/versions");
    const decided: Record<keyof typeof PORT_VERSIONS, string> = {
      engine: "lang-pack (grammar/tag-queries feed parse+index); search/read host imports (tool)",
      index: "resolver (Tier-2 upgrade of index-backed references); search host import (tool)",
      journal: "internal by design: plugins reach writes only via queue-write into §7.5",
      watcher: "internal by design: freshness is a host property, never delegated",
      sandbox: "the substrate itself (§8.1): worlds/imports are what its grants name",
      formatter: "internal by design: §7.5 format step stays first-party",
      resolver: "resolver.wit — resolve-references/rename/diagnostics",
    };
    expect(Object.keys(decided).sort()).toEqual(Object.keys(PORT_VERSIONS).sort());
  });
});
