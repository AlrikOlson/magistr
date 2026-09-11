/**
 * Wasmtime extension host through the real native binding
 * (SDD §8.1, chunk:m4-wasmtime-host).
 *
 * Pins the chunk's three acceptance criteria at the FFI boundary the engine
 * child actually uses:
 *
 *   1. Guests have no ambient filesystem or network access; imports resolve
 *      only through explicitly granted capability objects (default deny).
 *   2. Fuel exhaustion and memory-cap breach both terminate the guest
 *      deterministically — same outcome, same fuel accounting, every run.
 *   3. A trapped guest never takes down the engine child: every guest
 *      failure is a typed outcome value, and the process keeps serving.
 *
 * Skips (like test/core-loader.test.ts) when the native artifact is absent;
 * the wasm fallback lane deliberately lacks the host, and that absence is
 * itself pinned here.
 */

import { describe, expect, test } from "bun:test";
import { existsSync } from "node:fs";
import path from "node:path";

import {
  type CoreBinding,
  type CoreSandboxPolicy,
  defaultArtifactsDir,
  loadCore,
  nativeArtifactKey,
} from "../src/adapters/core/loader";

const nativeArtifact = path.join(
  defaultArtifactsDir(),
  `workbench-core.${nativeArtifactKey()}.node`,
);

const POLICY: CoreSandboxPolicy = { fuel: 1_000_000, memoryCapBytes: 10 * 65536 };

const ADD_WAT = `
(component
  (core module $m
    (memory (export "mem") 1)
    (func (export "add") (param i32 i32) (result i32)
      local.get 0
      local.get 1
      i32.add))
  (core instance $i (instantiate $m))
  (func (export "add") (param "a" u32) (param "b" u32) (result u32)
    (canon lift (core func $i "add")))
)
`;

const SPIN_WAT = `
(component
  (core module $m
    (func (export "spin") (loop $l br $l)))
  (core instance $i (instantiate $m))
  (func (export "spin") (canon lift (core func $i "spin")))
)
`;

const HOG_WAT = `
(component
  (core module $m
    (memory 1)
    (func (export "hog") (result i32)
      (loop $l
        (drop (memory.grow (i32.const 1)))
        (br $l))
      (i32.const 0))
  )
  (core instance $i (instantiate $m))
  (func (export "hog") (result u32) (canon lift (core func $i "hog")))
)
`;

const CRASH_WAT = `
(component
  (core module $m
    (func (export "crash") unreachable))
  (core instance $i (instantiate $m))
  (func (export "crash") (canon lift (core func $i "crash")))
)
`;

const OBSERVE_WAT = `
(component
  (import "observe" (func $obs (param "n" u32) (result u32)))
  (core func $obs_core (canon lower (func $obs)))
  (core module $m
    (import "cap" "observe" (func $obs (param i32) (result i32)))
    (func (export "run") (result i32)
      (drop (call $obs (i32.const 7)))
      (drop (call $obs (i32.const 11)))
      (call $obs (i32.const 13))))
  (core instance $i (instantiate $m (with "cap" (instance (export "observe" (func $obs_core))))))
  (func (export "run") (result u32) (canon lift (core func $i "run")))
)
`;

/** Probes fd 3 for a preopen and exits with the errno (8 = EBADF = none). */
const PREOPEN_PROBE_WAT = `
(module
  (import "wasi_snapshot_preview1" "fd_prestat_get"
    (func $prestat (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (call $exit (call $prestat (i32.const 3) (i32.const 0)))
    unreachable)
)
`;

const HELLO_WASI_WAT = `
(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 8) "jailed")
  (func (export "_start")
    (i32.store (i32.const 0) (i32.const 8))
    (i32.store (i32.const 4) (i32.const 6))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 16))))
)
`;

const wat = (text: string): Uint8Array => new TextEncoder().encode(text);

describe.if(existsSync(nativeArtifact))("sandbox host (real native binding)", () => {
  const { binding } = loadCore({ force: "native" });

  const runComponent = (
    guest: string,
    exportName: string,
    args: number[],
    capabilities: string[],
  ) => {
    if (!binding.sandboxRunComponent) throw new Error("native binding lacks the sandbox host");
    return binding.sandboxRunComponent(wat(guest), exportName, args, capabilities, POLICY);
  };

  const runWasi = (guest: string, argv: string[]) => {
    if (!binding.sandboxRunWasiModule) throw new Error("native binding lacks the sandbox host");
    return binding.sandboxRunWasiModule(wat(guest), argv, POLICY);
  };

  test("the host advertises its capability objects", () => {
    // v0 counter probe + the blessed-only subprocess capability
    // (chunk:m5-lsp-resolver-plugin), at its current stamp and the 0.1.0
    // stamp the host still serves per-invoke (specs/007-resolver-server-pool).
    expect(binding.sandboxCapabilities?.()).toEqual([
      "observe",
      "workbench:subprocess/subprocess@0.2.0",
      "workbench:subprocess/subprocess@0.1.0",
    ]);
  });

  test("a component runs under the policy and returns typed results", async () => {
    const outcome = await runComponent(ADD_WAT, "add", [19, 23], []);
    expect(outcome.outcome).toBe("ok");
    expect(outcome.results).toEqual(["U32(42)"]);
    expect(outcome.fuelUsed).toBeGreaterThan(0);
  });

  // Acceptance 1 — capability objects only, default deny.
  test("an ungranted import is denied by name before any guest code runs", async () => {
    const outcome = await runComponent(OBSERVE_WAT, "run", [], []);
    expect(outcome.outcome).toBe("cap-denied");
    expect(outcome.detail).toBe("observe");
    expect(outcome.fuelUsed).toBe(0);
  });

  test("a granted capability is the only door, and the host observes every call", async () => {
    const outcome = await runComponent(OBSERVE_WAT, "run", [], ["observe"]);
    expect(outcome.outcome).toBe("ok");
    expect(outcome.observations).toEqual([7, 11, 13]);
    expect(outcome.results).toEqual(["U32(3)"]);
  });

  test("granting a capability the host does not define is refused", async () => {
    const outcome = await runComponent(ADD_WAT, "add", [1, 2], ["filesystem"]);
    expect(outcome.outcome).toBe("invalid-guest");
    expect(outcome.detail).toContain("filesystem");
  });

  // Acceptance 1 — no ambient filesystem or network in the wasi lane either.
  test("a wasi guest sees zero preopened directories (errno 8 on the probe)", async () => {
    const outcome = await runWasi(PREOPEN_PROBE_WAT, ["probe"]);
    expect(outcome.outcome).toBe("guest-exit");
    expect(outcome.exitCode).toBe(8);
  });

  test("wasi stdout is captured into the outcome, never inherited", async () => {
    const outcome = await runWasi(HELLO_WASI_WAT, ["hello"]);
    expect(outcome.outcome).toBe("ok");
    expect(outcome.stdout).toBe("jailed");
    expect(outcome.exitCode).toBe(0);
  });

  // Acceptance 2 — deterministic kills.
  test("fuel exhaustion kills deterministically, identical accounting across runs", async () => {
    const first = await runComponent(SPIN_WAT, "spin", [], []);
    const second = await runComponent(SPIN_WAT, "spin", [], []);
    expect(first.outcome).toBe("out-of-fuel");
    expect(second.outcome).toBe("out-of-fuel");
    expect(first.fuelUsed).toBe(second.fuelUsed);
  });

  test("a memory-cap breach kills deterministically, identical accounting across runs", async () => {
    const first = await runComponent(HOG_WAT, "hog", [], []);
    const second = await runComponent(HOG_WAT, "hog", [], []);
    expect(first.outcome).toBe("memory-cap");
    expect(second.outcome).toBe("memory-cap");
    expect(first.fuelUsed).toBe(second.fuelUsed);
  });

  // Acceptance 3 — a trapped guest never takes down the engine child.
  test("every kill is a value, and the process keeps serving afterwards", async () => {
    expect((await runComponent(SPIN_WAT, "spin", [], [])).outcome).toBe("out-of-fuel");
    expect((await runComponent(HOG_WAT, "hog", [], [])).outcome).toBe("memory-cap");
    expect((await runComponent(CRASH_WAT, "crash", [], [])).outcome).toBe("guest-trap");
    expect((await runWasi(PREOPEN_PROBE_WAT, ["probe"])).outcome).toBe("guest-exit");

    const alive = await runComponent(ADD_WAT, "add", [2, 2], []);
    expect(alive.outcome).toBe("ok");
    expect(alive.results).toEqual(["U32(4)"]);

    // The rest of the core binding is untouched by the carnage.
    expect(binding.selfTest().allOk).toBe(true);
  });

  test("garbage bytes and a missing export are typed refusals, not exceptions", async () => {
    const garbage = await binding.sandboxRunComponent?.(
      new TextEncoder().encode("not wasm"),
      "x",
      [],
      [],
      POLICY,
    );
    expect(garbage?.outcome).toBe("invalid-guest");

    const missing = await runComponent(ADD_WAT, "nope", [], []);
    expect(missing.outcome).toBe("invalid-guest");
    expect(missing.detail).toContain("nope");
  });
});

const wasmArtifact = path.join(defaultArtifactsDir(), "workbench-core.wasm32-wasi.wasm");

describe.if(existsSync(wasmArtifact))("sandbox host (wasm fallback lane)", () => {
  test("the degraded lane honestly lacks the extension host", () => {
    const { binding } = loadCore({ force: "wasm" }) as { binding: CoreBinding };
    expect(binding.sandboxRunComponent).toBeUndefined();
    expect(binding.sandboxRunWasiModule).toBeUndefined();
  });
});
