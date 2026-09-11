/**
 * Language-server session pool — engine-child endings (specs/007-resolver-
 * server-pool US2 / SC-002, contract §7). Real engine children, the real
 * native binding, the real resolver guest, and a fake language server that
 * records its pid (and its parent's — the engine child) and, in `ignore-eof`
 * mode, deliberately outlives its stdin so only an OS-level kill can end it.
 *
 * Chapters:
 *   belt       a server that exits on stdin EOF dies with the engine child's
 *              deliberate stop — every platform
 *   shutdown   the engine child's NORMAL exit (stdin closed) runs the explicit
 *              pool shutdown: even an ignore-eof server dies — every platform
 *   kill group an ignore-eof server dies when the engine child is stopped or
 *              hard-killed — Windows (job object) and Linux (PDEATHSIG); on
 *              darwin the chapter is skipped and says why (research R5).
 */

import { describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { nativeArtifactKey } from "../src/adapters/core";
import {
  EngineSupervisor,
  encodeFrame,
  FrameDecoder,
  IPC_VERSION,
  SANDBOX_RESOLVER_METHOD,
} from "../src/adapters/engine-ipc";
import { SUBPROCESS_IMPORT } from "../src/tools/plugins";

const ROOT = path.join(import.meta.dir, "..");
const MAIN = path.join(ROOT, "src", "main.ts");
const GUEST = path.join(ROOT, "plugins", "lsp-resolver", "guest.wasm");
const FAKE = path.join(ROOT, "test", "support", "fake-lsp.ts");
const nativeArtifact = path.join(
  ROOT,
  "crates",
  "workbench-core",
  "artifacts",
  `workbench-core.${nativeArtifactKey()}.node`,
);
const OS_KILL_GROUP = process.platform === "win32" || process.platform === "linux";

function tempWorkspace(): string {
  const root = mkdtempSync(path.join(tmpdir(), "workbench-pool-"));
  mkdirSync(path.join(root, "src"), { recursive: true });
  writeFileSync(path.join(root, "src", "greet.ts"), "export function greet() {}\n");
  writeFileSync(path.join(root, "src", "use.ts"), "\n\ngreet();\n");
  return root;
}

function isAlive(pid: number): boolean {
  if (process.platform === "win32") {
    const out = Bun.spawnSync(["tasklist", "/FI", `PID eq ${pid}`, "/NH", "/FO", "CSV"]);
    return new TextDecoder().decode(out.stdout).includes(`"${pid}"`);
  }
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

async function waitGone(pid: number, ms: number): Promise<boolean> {
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    if (!isAlive(pid)) return true;
    await Bun.sleep(100);
  }
  return !isAlive(pid);
}

function readPids(file: string): { pid: number; ppid: number } {
  const [pid, ppid] = readFileSync(file, "utf8").trim().split(" ").map(Number);
  if (pid === undefined || ppid === undefined || Number.isNaN(pid) || Number.isNaN(ppid)) {
    throw new Error(`pid file malformed: ${readFileSync(file, "utf8")}`);
  }
  return { pid, ppid };
}

function resolverParams(root: string, pidFile: string, modes: string[]) {
  return {
    wasmPath: GUEST,
    symbol: "greet",
    capabilities: [SUBPROCESS_IMPORT],
    fuel: 500_000_000,
    memoryCapBytes: 64 * 1024 * 1024,
    subprocessSpecs: [
      { languageId: "fake", bin: process.execPath, args: [FAKE, ...modes, `pid:${pidFile}`] },
    ],
    subprocessRoot: root,
  };
}

interface ResolverResult {
  readonly outcome: string;
  readonly locations: readonly unknown[];
  readonly sessionsOpened: number;
  readonly sessionsReused: number;
}

describe.if(existsSync(nativeArtifact))("server pool: engine-child endings", () => {
  test("belt: a stdin-EOF-exiting server dies with the supervisor's stop (every platform)", async () => {
    const root = tempWorkspace();
    const pidFile = path.join(root, "fake.pid");
    const supervisor = new EngineSupervisor({ readyTimeoutMs: 30_000 });
    try {
      const result = (await supervisor.call(
        SANDBOX_RESOLVER_METHOD,
        resolverParams(root, pidFile, []),
      )) as ResolverResult;
      expect(result.outcome).toBe("ok");
      expect(result.locations).toHaveLength(2);
      expect(result.sessionsOpened).toBe(1);
      const { pid } = readPids(pidFile);
      expect(isAlive(pid)).toBe(true);
    } finally {
      await supervisor.stop();
    }
    const { pid } = readPids(pidFile);
    expect(await waitGone(pid, 10_000)).toBe(true);
  }, 60_000);

  test("warm reuse across two engine-child requests: the second invoke reuses, no respawn", async () => {
    const root = tempWorkspace();
    const pidFile = path.join(root, "fake.pid");
    const supervisor = new EngineSupervisor({ readyTimeoutMs: 30_000 });
    try {
      const first = (await supervisor.call(
        SANDBOX_RESOLVER_METHOD,
        resolverParams(root, pidFile, []),
      )) as ResolverResult;
      const pidAfterFirst = readPids(pidFile).pid;
      const second = (await supervisor.call(
        SANDBOX_RESOLVER_METHOD,
        resolverParams(root, pidFile, []),
      )) as ResolverResult;
      expect([first.sessionsOpened, first.sessionsReused]).toEqual([1, 0]);
      expect([second.sessionsOpened, second.sessionsReused]).toEqual([0, 1]);
      expect(second.locations).toHaveLength(2);
      // Same process answered both: the pid file was written once.
      expect(readPids(pidFile).pid).toBe(pidAfterFirst);
    } finally {
      await supervisor.stop();
    }
  }, 60_000);

  test("shutdown: the engine child's normal exit (stdin closed) kills an ignore-eof server (every platform)", async () => {
    const root = tempWorkspace();
    const pidFile = path.join(root, "fake.pid");
    const proc = Bun.spawn([process.execPath, MAIN, "--engine"], {
      stdin: "pipe",
      stdout: "pipe",
      stderr: "inherit",
      cwd: ROOT,
    });
    const decoder = new FrameDecoder();
    const reader = proc.stdout.getReader();
    const frames: unknown[] = [];
    async function nextFrame(
      pred: (f: Record<string, unknown>) => boolean,
    ): Promise<Record<string, unknown>> {
      for (;;) {
        const hit = frames.find((f) => pred(f as Record<string, unknown>));
        if (hit !== undefined) {
          frames.splice(frames.indexOf(hit), 1);
          return hit as Record<string, unknown>;
        }
        const { value, done } = await reader.read();
        if (done) throw new Error("engine child stdout ended");
        for (const decoded of decoder.push(value)) {
          if (decoded.ok) frames.push(decoded.frame);
        }
      }
    }
    await nextFrame((f) => f["kind"] === "ready");
    proc.stdin.write(
      encodeFrame({
        v: IPC_VERSION,
        kind: "req",
        id: 1,
        method: SANDBOX_RESOLVER_METHOD,
        params: resolverParams(root, pidFile, ["ignore-eof"]),
      }),
    );
    await proc.stdin.flush();
    const res = await nextFrame((f) => f["kind"] === "res" && f["id"] === 1);
    const result = res["result"] as ResolverResult;
    expect(result.outcome).toBe("ok");
    expect(result.sessionsOpened).toBe(1);
    const { pid, ppid } = readPids(pidFile);
    expect(ppid).toBe(proc.pid);
    expect(isAlive(pid)).toBe(true);
    // Close stdin: the frame loop ends, in-flight work settles, the child runs
    // the explicit pool shutdown and exits normally.
    proc.stdin.end();
    const exitCode = await proc.exited;
    expect(exitCode).toBe(0);
    expect(await waitGone(pid, 10_000)).toBe(true);
  }, 60_000);
});

describe.if(existsSync(nativeArtifact) && OS_KILL_GROUP)(
  "server pool: OS kill group (Windows job object / Linux PDEATHSIG)",
  () => {
    test("an ignore-eof server dies when the supervisor hard-stops the engine child", async () => {
      const root = tempWorkspace();
      const pidFile = path.join(root, "fake.pid");
      const supervisor = new EngineSupervisor({ readyTimeoutMs: 30_000 });
      const result = (await supervisor.call(
        SANDBOX_RESOLVER_METHOD,
        resolverParams(root, pidFile, ["ignore-eof"]),
      )) as ResolverResult;
      expect(result.outcome).toBe("ok");
      const { pid } = readPids(pidFile);
      expect(isAlive(pid)).toBe(true);
      await supervisor.stop(); // proc.kill(): no destructor ever runs
      expect(await waitGone(pid, 10_000)).toBe(true);
    }, 60_000);

    test("an ignore-eof server dies when the engine child is killed outright (SIGKILL)", async () => {
      const root = tempWorkspace();
      const pidFile = path.join(root, "fake.pid");
      const supervisor = new EngineSupervisor({ readyTimeoutMs: 30_000, backoffMs: [0, 5_000] });
      try {
        const result = (await supervisor.call(
          SANDBOX_RESOLVER_METHOD,
          resolverParams(root, pidFile, ["ignore-eof"]),
        )) as ResolverResult;
        expect(result.outcome).toBe("ok");
        const { pid, ppid } = readPids(pidFile);
        expect(isAlive(pid)).toBe(true);
        process.kill(ppid, "SIGKILL");
        expect(await waitGone(pid, 10_000)).toBe(true);
      } finally {
        await supervisor.stop();
      }
    }, 60_000);
  },
);

test.skipIf(OS_KILL_GROUP)(
  "server pool: OS kill group chapter skipped — no kill-on-parent-death primitive on this platform (darwin relies on the server's stdin-EOF exit; specs/007 research R5)",
  () => {},
);
