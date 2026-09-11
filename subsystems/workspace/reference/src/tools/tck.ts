/**
 * The plugin conformance kit — the TCK (SDD §8.5, chunk:m4-tck).
 *
 * One programmatic entry point, `runPluginTck(pluginDir, deps)`, produces a
 * typed chapter report. Registry admission runs this kit (§8.4, M6), the M4
 * gate requires the SDK's own examples to pass it unmodified, and the M5 gate
 * requires the same of every first-party plugin — so the kit is a library,
 * not a pile of test cases, and its report is data a CLI can render.
 *
 * Four chapters, per §8.5 verbatim:
 *
 *   interface       plugin.toml parses under the REAL manifest validator, the
 *                   component exists inside the plugin directory, and every
 *                   declared tool answers the canonical `{v, tool, args}`
 *                   payload with an ok outcome and JSON-parseable result.
 *   capabilities    capability-violation probes. Declared-sufficiency: under
 *                   grants = declared imports the run is never cap-denied (a
 *                   denial here means the component needs more than the
 *                   manifest admits — no grant can fix it). Least-privilege:
 *                   under ZERO grants a plugin declaring imports MUST be
 *                   cap-denied before instantiation; success proves it
 *                   declares imports it does not use.
 *   resources       fuel/memory abuse. The plugin's tools answer TYPED
 *                   outcomes under starvation fuel and a tiny memory cap —
 *                   never a hang or a host error. Kit-owned abuse fixtures
 *                   (a spin loop, a memory-grow loop) run through the same
 *                   invoke lane and must die typed (out-of-fuel, memory-cap),
 *                   proving the enforcing sandbox is live, not assumed.
 *   lang-pack-fuzz  applicable when the manifest declares a tool named
 *                   `classify-file` (the lang-pack convention). A
 *                   deterministic seeded fuzz corpus — edge-case seeds plus
 *                   xorshift-mutated variants — goes through the tool
 *                   surface; every entry must answer ok with JSON. The real
 *                   lang-pack world's `classify-file(string) -> option<file-class>`
 *                   is not lowerable by the live `(string) -> string` invoke
 *                   lane; when M5 lands that lowering, this chapter re-points
 *                   at the world export with the same corpus discipline.
 *
 * The kit reads the plugin directory with plain `node:fs`, deliberately: it
 * runs against a directory on the operator's machine (registry admission,
 * SDK development), outside the server's §7.1 jail, and it is never imported
 * by the served surface. Guest execution is injected (`invokeGuest`), so the
 * kit runs identically over the real engine-child lane, a direct native
 * binding, or a scripted fake in tests.
 */

import { mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  PLUGIN_FUEL_CAP,
  PLUGIN_MEMORY_CAP,
  type PluginHostDeps,
  type PluginInvokeOutcome,
  type PluginManifest,
  parsePluginManifest,
} from "./plugins";

/** Manifest tool name that marks a plugin as a language pack (D6, think:97). */
export const TCK_LANG_PACK_TOOL = "classify-file";

/** Starvation fuel for the resource chapter's plugin-side probe. */
export const TCK_STARVATION_FUEL = 10;

/** Tiny memory cap (bytes) for the resource chapter's plugin-side probe. */
export const TCK_TINY_MEMORY_CAP = 32 * 1024;

/** Default fuzz-corpus mutation count (on top of the fixed seed cases). */
export const TCK_FUZZ_ITERATIONS_DEFAULT = 64;

/** Default xorshift32 seed — any fixed value; stability is what matters. */
export const TCK_FUZZ_SEED_DEFAULT = 0x54434b31; // "TCK1"

const TYPED_OUTCOMES: readonly PluginInvokeOutcome["outcome"][] = [
  "ok",
  "out-of-fuel",
  "memory-cap",
  "cap-denied",
  "guest-trap",
  "guest-exit",
  "invalid-guest",
];

/** Kit-owned abuse fixture: an invoke export that never returns — the fuel
 * meter is the only thing that ends it. Canonical-ABI (string) -> string. */
export const TCK_SPIN_WAT = `
(component
  (core module $m
    (memory (export "mem") 1)
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      (i32.const 1024))
    (func (export "spin") (param i32 i32) (result i32)
      (loop $l (br $l))
      (i32.const 16)))
  (core instance $i (instantiate $m))
  (func (export "invoke") (param "payload" string) (result string)
    (canon lift (core func $i "spin") (memory $i "mem") (realloc (func $i "realloc"))))
)
`;

/** Kit-owned abuse fixture: grows memory until the resource limiter kills it
 * (trap_on_grow_failure makes the breach deterministic and typed). */
export const TCK_GROW_WAT = `
(component
  (core module $m
    (memory (export "mem") 1)
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      (i32.const 1024))
    (func (export "grow") (param i32 i32) (result i32)
      (loop $l
        (drop (memory.grow (i32.const 1)))
        (br $l))
      (i32.const 16)))
  (core instance $i (instantiate $m))
  (func (export "invoke") (param "payload" string) (result string)
    (canon lift (core func $i "grow") (memory $i "mem") (realloc (func $i "realloc"))))
)
`;

export interface TckDeps {
  /** One guest invoke — same contract as the plugin host's lane. */
  readonly invokeGuest: PluginHostDeps["invokeGuest"];
  /** Fuzz-corpus tuning; defaults are deterministic and CI-sized. */
  readonly fuzz?: {
    readonly iterations?: number;
    readonly seed?: number;
  };
}

export interface TckProbe {
  readonly id: string;
  readonly passed: boolean;
  readonly detail: string;
}

export type TckChapterId = "interface" | "capabilities" | "resources" | "lang-pack-fuzz";

export interface TckChapter {
  readonly id: TckChapterId;
  /** False when the chapter does not apply to this plugin (e.g. the
   * lang-pack chapter for a plain tool plugin). Inapplicable never fails. */
  readonly applicable: boolean;
  readonly passed: boolean;
  readonly probes: readonly TckProbe[];
}

export interface TckReport {
  readonly pluginDir: string;
  /** `publisher.pkg` once the manifest parses; null when it never did. */
  readonly plugin: string | null;
  readonly passed: boolean;
  readonly chapters: readonly TckChapter[];
  readonly notes: readonly string[];
}

/** Deterministic xorshift32 — the corpus must be identical run to run. */
function xorshift32(seed: number): () => number {
  let state = seed >>> 0 || 1;
  return () => {
    state ^= state << 13;
    state >>>= 0;
    state ^= state >> 17;
    state ^= state << 5;
    state >>>= 0;
    return state;
  };
}

/** Fixed edge-case seeds every corpus starts from. */
const FUZZ_SEED_CASES: readonly string[] = [
  "",
  "src/main.ts",
  'fn main() { println!("hello"); }',
  "päth/withõ/ünicode.gleam",
  "null\u0000byte.txt",
  "a".repeat(4096),
  `${"[".repeat(256)}deep${"]".repeat(256)}`,
  "\t\r\n\u001b[31mansi\u001b[0m\u0007",
  "🦀🧩\u{10FFFF}",
];

const FUZZ_MUTATION_CHARS = " abcdefghijklmnopqrstuvwxyz/._-{}()[]\"'\\\u0000é世";

/** One seeded mutation of one base case: truncate, splice, repeat or inject. */
function mutate(base: string, rand: () => number): string {
  switch (rand() % 4) {
    case 0:
      return base.slice(0, rand() % (base.length + 1));
    case 1: {
      const at = rand() % (base.length + 1);
      const char = FUZZ_MUTATION_CHARS[rand() % FUZZ_MUTATION_CHARS.length] ?? "x";
      return base.slice(0, at) + char.repeat((rand() % 16) + 1) + base.slice(at);
    }
    case 2:
      return base + base.slice(0, rand() % (base.length + 1));
    default:
      return base.split("").reverse().join("");
  }
}

function chapter(id: TckChapterId, applicable: boolean, probes: readonly TckProbe[]): TckChapter {
  return {
    id,
    applicable,
    passed: !applicable || probes.every((p) => p.passed),
    probes,
  };
}

/** The clamped standard policy the plugin host would run this manifest under. */
function standardPolicy(manifest: PluginManifest): { fuel: number; memoryCapBytes: number } {
  return {
    fuel: Math.min(manifest.limits.fuel ?? PLUGIN_FUEL_CAP, PLUGIN_FUEL_CAP),
    memoryCapBytes: Math.min(
      manifest.limits.memoryCapBytes ?? PLUGIN_MEMORY_CAP,
      PLUGIN_MEMORY_CAP,
    ),
  };
}

interface InvokeProbeResult {
  readonly outcome: PluginInvokeOutcome | null;
  /** Message when the invoke lane itself threw (child down, degraded). */
  readonly threw: string | null;
}

async function tryInvoke(
  deps: TckDeps,
  absPath: string,
  exportName: string,
  payload: string,
  capabilities: readonly string[],
  policy: { fuel: number; memoryCapBytes: number },
): Promise<InvokeProbeResult> {
  try {
    const outcome = await deps.invokeGuest(absPath, exportName, payload, capabilities, policy);
    return { outcome, threw: null };
  } catch (error) {
    return { outcome: null, threw: error instanceof Error ? error.message : String(error) };
  }
}

function toolPayload(tool: string, args: Record<string, unknown>): string {
  return JSON.stringify({ v: 1, tool, args });
}

/**
 * Run the full conformance kit against one plugin directory.
 *
 * Never throws for a non-conforming plugin — every failure is a failed probe
 * in the report. Throws only on kit-level breakage (e.g. the temp directory
 * for abuse fixtures cannot be created).
 */
export async function runPluginTck(pluginDir: string, deps: TckDeps): Promise<TckReport> {
  const notes: string[] = [];
  const chapters: TckChapter[] = [];

  // ---- Chapter: interface ---------------------------------------------------

  const interfaceProbes: TckProbe[] = [];
  let manifest: PluginManifest | null = null;
  let componentAbs: string | null = null;

  let manifestSource: string | null = null;
  try {
    manifestSource = await readFile(path.join(pluginDir, "plugin.toml"), "utf8");
  } catch (error) {
    interfaceProbes.push({
      id: "manifest-readable",
      passed: false,
      detail: `plugin.toml unreadable: ${error instanceof Error ? error.message : String(error)}`,
    });
  }
  if (manifestSource !== null) {
    const parsed = parsePluginManifest(manifestSource);
    if (parsed.ok) {
      manifest = parsed.manifest;
      notes.push(...parsed.warnings);
      interfaceProbes.push({
        id: "manifest-valid",
        passed: true,
        detail: `plugin ${manifest.publisher}.${manifest.pkg}@${manifest.version}, ${manifest.tools.length} tool(s)`,
      });
    } else {
      interfaceProbes.push({
        id: "manifest-valid",
        passed: false,
        detail: parsed.errors.join("; "),
      });
    }
  }

  if (manifest !== null) {
    const candidate = path.resolve(pluginDir, manifest.componentPath);
    try {
      const info = await stat(candidate);
      if (info.isFile()) {
        componentAbs = candidate;
        interfaceProbes.push({
          id: "component-present",
          passed: true,
          detail: `${manifest.componentPath} (${info.size} bytes)`,
        });
      } else {
        interfaceProbes.push({
          id: "component-present",
          passed: false,
          detail: `${manifest.componentPath} is not a regular file`,
        });
      }
    } catch {
      interfaceProbes.push({
        id: "component-present",
        passed: false,
        detail: `${manifest.componentPath} is missing`,
      });
    }
  }

  const declaredGrants = manifest?.imports ?? [];
  const policy = manifest === null ? null : standardPolicy(manifest);

  if (manifest !== null && componentAbs !== null && policy !== null) {
    for (const tool of manifest.tools) {
      const run = await tryInvoke(
        deps,
        componentAbs,
        manifest.invokeExport,
        toolPayload(tool.name, {}),
        declaredGrants,
        policy,
      );
      if (run.threw !== null) {
        interfaceProbes.push({
          id: `tool-${tool.name}-invokes`,
          passed: false,
          detail: `invoke lane threw: ${run.threw}`,
        });
        continue;
      }
      const outcome = run.outcome as PluginInvokeOutcome;
      if (outcome.outcome !== "ok") {
        interfaceProbes.push({
          id: `tool-${tool.name}-invokes`,
          passed: false,
          detail: `expected ok, got ${outcome.outcome}${outcome.detail === "" ? "" : `: ${outcome.detail}`}`,
        });
        continue;
      }
      let jsonOk = true;
      try {
        JSON.parse(outcome.result ?? "");
      } catch {
        jsonOk = false;
      }
      interfaceProbes.push({
        id: `tool-${tool.name}-invokes`,
        passed: jsonOk,
        detail: jsonOk
          ? `ok, ${outcome.fuelUsed} fuel`
          : "result is not JSON — a plugin tool's result must be JSON text (wit/tool.wit)",
      });
    }
  }
  chapters.push(chapter("interface", true, interfaceProbes));

  // ---- Chapter: capabilities ------------------------------------------------

  const capabilityProbes: TckProbe[] = [];
  if (manifest !== null && componentAbs !== null && policy !== null) {
    const firstTool = manifest.tools[0]?.name ?? "probe";
    const payload = toolPayload(firstTool, {});

    const declared = await tryInvoke(
      deps,
      componentAbs,
      manifest.invokeExport,
      payload,
      declaredGrants,
      policy,
    );
    capabilityProbes.push({
      id: "declared-imports-suffice",
      passed: declared.threw === null && declared.outcome?.outcome !== "cap-denied",
      detail:
        declared.threw !== null
          ? `invoke lane threw: ${declared.threw}`
          : declared.outcome?.outcome === "cap-denied"
            ? `cap-denied under the full declared grant set (needs ${JSON.stringify(declared.outcome.detail)}) — ` +
              "the component requires an import the manifest does not declare; no grant can fix this"
            : `outcome ${declared.outcome?.outcome} under grants=[${declaredGrants.join(", ")}]`,
    });

    const zeroGrant = await tryInvoke(
      deps,
      componentAbs,
      manifest.invokeExport,
      payload,
      [],
      policy,
    );
    if (zeroGrant.threw !== null) {
      capabilityProbes.push({
        id: "least-privilege",
        passed: false,
        detail: `invoke lane threw: ${zeroGrant.threw}`,
      });
    } else if (declaredGrants.length === 0) {
      const outcome = zeroGrant.outcome as PluginInvokeOutcome;
      capabilityProbes.push({
        id: "least-privilege",
        passed: outcome.outcome !== "cap-denied",
        detail:
          outcome.outcome === "cap-denied"
            ? `component imports ${JSON.stringify(outcome.detail)} but the manifest declares no imports`
            : `import-free plugin runs under zero grants (outcome ${outcome.outcome})`,
      });
    } else {
      const outcome = zeroGrant.outcome as PluginInvokeOutcome;
      capabilityProbes.push({
        id: "least-privilege",
        passed: outcome.outcome === "cap-denied",
        detail:
          outcome.outcome === "cap-denied"
            ? `zero grants denied before instantiation (${JSON.stringify(outcome.detail)})`
            : `declares imports [${declaredGrants.join(", ")}] yet ran with zero grants ` +
              `(outcome ${outcome.outcome}) — it declares capabilities it does not use`,
      });
    }
  }
  chapters.push(
    chapter("capabilities", manifest !== null && componentAbs !== null, capabilityProbes),
  );

  // ---- Chapter: resources ---------------------------------------------------

  const resourceProbes: TckProbe[] = [];
  if (manifest !== null && componentAbs !== null && policy !== null) {
    for (const tool of manifest.tools) {
      const starved = await tryInvoke(
        deps,
        componentAbs,
        manifest.invokeExport,
        toolPayload(tool.name, {}),
        declaredGrants,
        { fuel: TCK_STARVATION_FUEL, memoryCapBytes: policy.memoryCapBytes },
      );
      capOk(resourceProbes, `tool-${tool.name}-starvation-fuel`, starved, ["ok", "out-of-fuel"]);

      const squeezed = await tryInvoke(
        deps,
        componentAbs,
        manifest.invokeExport,
        toolPayload(tool.name, {}),
        declaredGrants,
        { fuel: policy.fuel, memoryCapBytes: TCK_TINY_MEMORY_CAP },
      );
      capOk(resourceProbes, `tool-${tool.name}-tiny-memory`, squeezed, TYPED_OUTCOMES);
    }

    // Environment probes: the kit's own abusive guests must die typed. A
    // sandbox that lets either survive is not the sandbox the report claims.
    const fixtures = await mkdtemp(path.join(tmpdir(), "workbench-tck-"));
    try {
      const spinPath = path.join(fixtures, "tck-spin.wat");
      await writeFile(spinPath, TCK_SPIN_WAT);
      const spun = await tryInvoke(deps, spinPath, "invoke", toolPayload("spin", {}), [], {
        fuel: 100_000,
        memoryCapBytes: PLUGIN_MEMORY_CAP,
      });
      capOk(resourceProbes, "environment-fuel-kill", spun, ["out-of-fuel"]);

      const growPath = path.join(fixtures, "tck-grow.wat");
      await writeFile(growPath, TCK_GROW_WAT);
      // Bounded fuel so a limiter that fails to trap still ends the probe
      // quickly — as out-of-fuel, which the assertion rightly rejects.
      const grown = await tryInvoke(deps, growPath, "invoke", toolPayload("grow", {}), [], {
        fuel: 10_000_000,
        memoryCapBytes: 4 * 64 * 1024,
      });
      capOk(resourceProbes, "environment-memory-kill", grown, ["memory-cap"]);
    } finally {
      await rm(fixtures, { recursive: true, force: true });
    }
  }
  chapters.push(chapter("resources", manifest !== null && componentAbs !== null, resourceProbes));

  // ---- Chapter: lang-pack-fuzz ----------------------------------------------

  const fuzzProbes: TckProbe[] = [];
  const langPackTool = manifest?.tools.find((t) => t.name === TCK_LANG_PACK_TOOL);
  const fuzzApplicable =
    manifest !== null && componentAbs !== null && policy !== null && langPackTool !== undefined;
  if (manifest !== null && componentAbs !== null && policy !== null && langPackTool !== undefined) {
    const iterations = deps.fuzz?.iterations ?? TCK_FUZZ_ITERATIONS_DEFAULT;
    const seed = deps.fuzz?.seed ?? TCK_FUZZ_SEED_DEFAULT;
    const rand = xorshift32(seed);
    const corpus: string[] = [...FUZZ_SEED_CASES];
    for (let i = 0; i < iterations; i++) {
      const base = FUZZ_SEED_CASES[rand() % FUZZ_SEED_CASES.length] ?? "";
      corpus.push(mutate(base, rand));
    }
    let failures = 0;
    let firstFailure = "";
    for (const [index, input] of corpus.entries()) {
      const run = await tryInvoke(
        deps,
        componentAbs,
        manifest.invokeExport,
        toolPayload(TCK_LANG_PACK_TOOL, { path: input }),
        declaredGrants,
        policy,
      );
      const ok =
        run.threw === null &&
        run.outcome?.outcome === "ok" &&
        (() => {
          try {
            JSON.parse(run.outcome.result ?? "");
            return true;
          } catch {
            return false;
          }
        })();
      if (!ok) {
        failures++;
        if (firstFailure === "") {
          firstFailure =
            `corpus[${index}] (${JSON.stringify(input.slice(0, 40))}${input.length > 40 ? "…" : ""}): ` +
            (run.threw ?? `outcome ${run.outcome?.outcome}`);
        }
      }
    }
    fuzzProbes.push({
      id: "fuzz-corpus",
      passed: failures === 0,
      detail:
        failures === 0
          ? `${corpus.length} corpus entries (seed ${seed >>> 0}), all ok with JSON results`
          : `${failures}/${corpus.length} corpus entries failed; first: ${firstFailure}`,
    });
  }
  chapters.push(chapter("lang-pack-fuzz", fuzzApplicable, fuzzProbes));

  return {
    pluginDir,
    plugin: manifest === null ? null : `${manifest.publisher}.${manifest.pkg}`,
    passed: chapters.every((c) => c.passed),
    chapters,
    notes,
  };
}

/** Record one resource probe: the run must not throw, and the outcome must be
 * one of `allowed` — "died typed" is the whole assertion. */
function capOk(
  probes: TckProbe[],
  id: string,
  run: InvokeProbeResult,
  allowed: readonly PluginInvokeOutcome["outcome"][],
): void {
  if (run.threw !== null) {
    probes.push({ id, passed: false, detail: `invoke lane threw: ${run.threw}` });
    return;
  }
  const outcome = run.outcome as PluginInvokeOutcome;
  const passed = allowed.includes(outcome.outcome);
  probes.push({
    id,
    passed,
    detail: passed
      ? `typed outcome ${outcome.outcome}`
      : `expected one of [${allowed.join(", ")}], got ${outcome.outcome}` +
        (outcome.detail === "" ? "" : `: ${outcome.detail}`),
  });
}
