/**
 * The workspace write lock (SDD §7.7) — chunk:m2-write-lock's acceptance
 * criteria: two clients with interleaved mutations produce no interleaved
 * corruption (the journal is the serialization witness); a cancelled request
 * actually stops burning engine cycles (the child's token checks); and
 * bounded queues answer ENGINE_BUSY with a suggested backoff, never a hang.
 */

import { afterEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  ChildIndexer,
  EngineBusyError,
  EngineSupervisor,
  IndexMethodError,
  stubCoreBinding,
} from "../src/adapters/engine-ipc";
import { createInProcessPorts, type InProcessPorts } from "../src/adapters/inprocess";
import { WorkspaceLockBusyError, WorkspaceWriteLock } from "../src/adapters/workspace-lock";
import type { ResponseEnvelope } from "../src/envelope";
import {
  runWriteBatch,
  type WriteEngineDeps,
  type WriteReport,
  type WriteWorkspaceAccess,
} from "../src/tools/write-engine";

const tempDirs: string[] = [];
const supervisors: EngineSupervisor[] = [];

function tmpRoot(): string {
  const dir = mkdtempSync(join(tmpdir(), "wb-write-lock-"));
  tempDirs.push(dir);
  return dir;
}

afterEach(async () => {
  await Promise.all(supervisors.splice(0).map((s) => s.stop()));
  for (const dir of tempDirs.splice(0)) {
    rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 });
  }
});

function enoent(path: string): NodeJS.ErrnoException {
  const error = new Error(`ENOENT: no such file: ${path}`) as NodeJS.ErrnoException;
  error.code = "ENOENT";
  return error;
}

/** In-memory workspace access whose write yields first — the interleaving
 * window a missing lock would corrupt through. */
function accessOf(files: Map<string, string>): WriteWorkspaceAccess {
  return {
    read: (path) =>
      files.has(path) ? Promise.resolve(files.get(path) as string) : Promise.reject(enoent(path)),
    write: async (path, content) => {
      await Bun.sleep(0);
      files.set(path, content);
    },
    remove: async (path) => {
      await Bun.sleep(0);
      files.delete(path);
    },
  };
}

/** One shared workspace + journal, one WriteEngineDeps per "client", each
 * client with its own lock instance over the same lock root — the same
 * shape as two processes contending for one workspace. */
function makeClients(root: string): {
  ports: InProcessPorts;
  clientA: WriteEngineDeps;
  clientB: WriteEngineDeps;
} {
  const ports = createInProcessPorts();
  const access = accessOf(ports.workspace.files);
  const base = { engine: ports.engine, formatter: ports.formatter, journal: ports.journal };
  const clientA: WriteEngineDeps = {
    ...base,
    workspace: access,
    lock: new WorkspaceWriteLock(root, { pollMs: 5 }),
  };
  const clientB: WriteEngineDeps = {
    ...base,
    workspace: access,
    lock: new WorkspaceWriteLock(root, { pollMs: 5 }),
  };
  return { ports, clientA, clientB };
}

function ok(envelope: ResponseEnvelope<WriteReport>): readonly WriteReport[] {
  if (!envelope.ok) throw new Error(`expected ok envelope, got ${envelope.error.code}`);
  return envelope.results;
}

describe("two clients, interleaved mutations (§7.7 acceptance)", () => {
  test("batches serialize: paired files never tear, journal pairs stay contiguous", async () => {
    const root = tmpRoot();
    const { ports, clientA, clientB } = makeClients(root);
    ports.workspace.files.set("pair/a.txt", "seed\n");
    ports.workspace.files.set("pair/b.txt", "seed\n");

    // Invariant under serialization: every batch writes the SAME value to
    // both files, so a.txt === b.txt after any prefix of complete batches.
    const ROUNDS = 12;
    const client = async (deps: WriteEngineDeps, tag: string): Promise<void> => {
      for (let i = 0; i < ROUNDS; i += 1) {
        const value = `${tag}-${i}\n`;
        const envelope = await runWriteBatch(
          deps,
          {
            writes: [
              { path: "pair/a.txt", content: value },
              { path: "pair/b.txt", content: value },
            ],
          },
          1,
        );
        ok(envelope);
      }
    };
    await Promise.all([client(clientA, "A"), client(clientB, "B")]);

    expect(ports.workspace.files.get("pair/a.txt")).toBe(ports.workspace.files.get("pair/b.txt"));

    // The journal is the serialization witness: one op per batch, and the
    // pair inside each op carries one batch's value — never a mix.
    const journal = await ports.journal.list(0);
    expect(journal).toHaveLength(ROUNDS * 2);
    for (const op of journal) {
      const [first, second] = op.files;
      expect(first?.after).toBe(second?.after);
      expect(first?.path).toBe("pair/a.txt");
      expect(second?.path).toBe("pair/b.txt");
    }
  });

  test("one instance serves its own callers in FIFO order", async () => {
    const lock = new WorkspaceWriteLock(tmpRoot(), { pollMs: 5 });
    const order: number[] = [];
    await Promise.all(
      [1, 2, 3].map((n) =>
        lock.withLock(async () => {
          await Bun.sleep(5);
          order.push(n);
        }),
      ),
    );
    expect(order).toEqual([1, 2, 3]);
    // An earlier holder's failure must not poison the queue.
    await expect(lock.withLock(() => Promise.reject(new Error("boom")))).rejects.toThrow("boom");
    await expect(lock.withLock(() => Promise.resolve("after"))).resolves.toBe("after");
  });
});

describe("the advisory lock file", () => {
  test("a dead holder's lock is taken over; a live one is waited out", async () => {
    const root = tmpRoot();
    mkdirSync(join(root, ".workbench"), { recursive: true });
    writeFileSync(
      join(root, ".workbench", "write.lock"),
      JSON.stringify({ pid: 99_999_999, acquiredAtMs: Date.now() }),
    );
    const lock = new WorkspaceWriteLock(root, {
      pollMs: 5,
      acquireTimeoutMs: 500,
      pidAlive: () => false, // the probe says: corpse
    });
    await expect(lock.withLock(() => Promise.resolve("took over"))).resolves.toBe("took over");
  });

  test("an unparseable lock file is a corpse by definition", async () => {
    const root = tmpRoot();
    mkdirSync(join(root, ".workbench"), { recursive: true });
    writeFileSync(join(root, ".workbench", "write.lock"), "not json at all");
    const lock = new WorkspaceWriteLock(root, { pollMs: 5, acquireTimeoutMs: 500 });
    await expect(lock.withLock(() => Promise.resolve("ok"))).resolves.toBe("ok");
  });

  test("our own pid in the file is live contention, not a corpse", async () => {
    const root = tmpRoot();
    mkdirSync(join(root, ".workbench"), { recursive: true });
    writeFileSync(
      join(root, ".workbench", "write.lock"),
      JSON.stringify({ pid: process.pid, acquiredAtMs: Date.now() }),
    );
    const lock = new WorkspaceWriteLock(root, { pollMs: 5, acquireTimeoutMs: 60 });
    await expect(lock.withLock(() => Promise.resolve("never"))).rejects.toBeInstanceOf(
      WorkspaceLockBusyError,
    );
  });

  test("an over-age lock is stale even when its pid reads alive", async () => {
    const root = tmpRoot();
    mkdirSync(join(root, ".workbench"), { recursive: true });
    writeFileSync(
      join(root, ".workbench", "write.lock"),
      JSON.stringify({ pid: process.pid, acquiredAtMs: Date.now() - 60_000 }),
    );
    const lock = new WorkspaceWriteLock(root, {
      pollMs: 5,
      acquireTimeoutMs: 500,
      staleMs: 1_000,
    });
    await expect(lock.withLock(() => Promise.resolve("reclaimed"))).resolves.toBe("reclaimed");
  });

  test("bounded wait: a held lock answers ENGINE_BUSY with backoff, never hangs", async () => {
    const root = tmpRoot();
    const holder = new WorkspaceWriteLock(root, { pollMs: 5 });
    const contender = new WorkspaceWriteLock(root, { pollMs: 10, acquireTimeoutMs: 80 });
    let releaseHolder!: () => void;
    const held = new Promise<void>((resolve) => {
      releaseHolder = resolve;
    });
    const holding = holder.withLock(() => held);

    const refusal = contender.withLock(() => Promise.resolve("never")) as Promise<unknown>;
    const error = (await refusal.then(
      () => null,
      (e: unknown) => e,
    )) as WorkspaceLockBusyError | null;
    expect(error).toBeInstanceOf(WorkspaceLockBusyError);
    expect(error?.code).toBe("ENGINE_BUSY");
    expect(error?.backoffMs).toBeGreaterThan(0);

    releaseHolder();
    await holding;
    // The §4.3 retry recovery is honest: after the holder yields, retry wins.
    await expect(contender.withLock(() => Promise.resolve("retried"))).resolves.toBe("retried");
  });
});

describe("typed backpressure through the tool envelope", () => {
  test("a busy lock surfaces as taxonomy ENGINE_BUSY and applies nothing", async () => {
    const root = tmpRoot();
    const { ports, clientA, clientB } = makeClients(root);
    ports.workspace.files.set("f.txt", "before\n");

    // Client A parks inside the §7.5 pipeline while B's bounded wait expires.
    const slowB: WriteEngineDeps = {
      ...clientB,
      lock: new WorkspaceWriteLock(root, { pollMs: 10, acquireTimeoutMs: 60 }),
    };
    let releaseA!: () => void;
    const gate = new Promise<void>((resolve) => {
      releaseA = resolve;
    });
    const slowAccess: WriteWorkspaceAccess = {
      ...clientA.workspace,
      write: async (path, content) => {
        await gate;
        ports.workspace.files.set(path, content);
      },
    };
    const holding = runWriteBatch(
      { ...clientA, workspace: slowAccess },
      { writes: [{ path: "f.txt", content: "from A\n" }] },
      1,
    );
    await Bun.sleep(10); // let A take the file lock

    const envelope = await runWriteBatch(
      slowB,
      { writes: [{ path: "f.txt", content: "from B\n" }] },
      1,
    );
    if (envelope.ok) throw new Error("expected ENGINE_BUSY, got ok");
    expect(envelope.error.code).toBe("ENGINE_BUSY");
    expect(envelope.error.recovery).toBe("retry");
    expect((envelope.error as { backoffMs?: number }).backoffMs).toBeGreaterThan(0);
    expect(ports.workspace.files.get("f.txt")).toBe("before\n");

    releaseA();
    ok(await holding);
    expect(ports.workspace.files.get("f.txt")).toBe("from A\n");
  });

  test("the supervisor's bounded queue rejects overflow with EngineBusyError", async () => {
    const supervisor = new EngineSupervisor({
      env: { WORKBENCH_ENGINE_STUB_CORE: "1" },
      backoffMs: [0, 10, 20],
      readyTimeoutMs: 10_000,
      maxPending: 1,
    });
    supervisors.push(supervisor);

    // The first call occupies the whole queue while the child boots; the
    // second gets the §7.7 typed answer immediately instead of queueing.
    const first = supervisor.call("ping", {});
    const second = supervisor.call("ping", {});
    const error = (await second.then(
      () => null,
      (e: unknown) => e,
    )) as EngineBusyError | null;
    expect(error).toBeInstanceOf(EngineBusyError);
    expect(error?.code).toBe("ENGINE_BUSY");
    expect(error?.backoffMs).toBeGreaterThan(0);
    expect(await first).toEqual({ pong: true });
    // Capacity freed: the retry the error suggests actually works.
    expect(await supervisor.call("ping", {})).toEqual({ pong: true });
  });
});

describe("cancellation stops engine cycles (§7.7)", () => {
  test("a cancelled reference query stops between scan units", async () => {
    const root = tmpRoot();
    for (let i = 0; i < 6; i += 1) {
      writeFileSync(join(root, `mod${i}.ts`), `export function alpha${i}() { return alpha() }\n`);
    }
    writeFileSync(join(root, "alpha.ts"), "export function alpha() { return 1 }\n");
    const indexer = new ChildIndexer(root, stubCoreBinding());
    try {
      indexer.build();
      await indexer.settled();

      let checks = 0;
      const cancelAfterTwo = (): boolean => {
        checks += 1;
        return checks > 2;
      };
      const error = (await indexer.queryReferencesVerified("alpha", cancelAfterTwo).then(
        () => null,
        (e: unknown) => e,
      )) as IndexMethodError | null;
      expect(error).toBeInstanceOf(IndexMethodError);
      expect(error?.code).toBe("REQUEST_CANCELLED");
      // The token was consulted per unit and honoured at the third check —
      // the remaining files were never scanned.
      expect(checks).toBe(3);

      // An uncancelled run over the same corpus proves there WAS more work.
      const full = await indexer.queryReferencesVerified("alpha");
      expect(full.edges.length).toBeGreaterThan(3);
    } finally {
      indexer.close();
    }
  });
});
