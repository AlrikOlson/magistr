/**
 * Supervision and chaos suite (SDD §6.2) against REAL engine children: real
 * `bun src/main.ts --engine` processes, real pipes, real kills. Only the
 * core binding is stubbed (WORKBENCH_ENGINE_STUB_CORE=1) so the suite runs
 * on machines and CI lanes without built artifacts.
 *
 * Kill points are planted with WORKBENCH_ENGINE_KILL_ON=<method|selftest>
 * plus a payload marker, and the chaos sweep enumerates the child's method
 * table — a method added later (e.g. the M1 indexer's) joins the sweep by
 * construction or fails the params-builder, never silently escapes it.
 * "mid-verify" is the parse kill point (parse IS the write path's
 * parse-verify primitive) plus the mid-startup self-test kill; "mid-index"
 * arrives with the M1 indexer's methods.
 */

import { Database } from "bun:sqlite";
import { afterEach, describe, expect, test } from "bun:test";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  ENGINE_METHODS,
  type EngineMethod,
  EngineSupervisor,
  type EngineSupervisorOptions,
  EngineUnavailableError,
  INDEX_METHODS,
  INDEX_SCHEMA_VERSION,
  type IndexMethod,
  InputQuarantinedError,
  IpcEngineAdapter,
  RequestCancelledError,
} from "../src/adapters/engine-ipc";

const MARKER = "WORKBENCH_CHAOS_MARKER";
const FAST_BACKOFF = [0, 10, 20] as const;

const supervisors: EngineSupervisor[] = [];

function makeSupervisor(
  env: Record<string, string> = {},
  extra: Partial<EngineSupervisorOptions> = {},
): EngineSupervisor {
  const supervisor = new EngineSupervisor({
    env: { WORKBENCH_ENGINE_STUB_CORE: "1", ...env },
    backoffMs: FAST_BACKOFF,
    readyTimeoutMs: 10_000,
    ...extra,
  });
  supervisors.push(supervisor);
  return supervisor;
}

afterEach(async () => {
  await Promise.all(supervisors.splice(0).map((s) => s.stop()));
});

/** Marker-carrying params for one method — throws on a method it does not
 * know, so a new child method cannot silently escape the chaos sweep. */
function chaosParams(method: EngineMethod, marker: string): unknown {
  switch (method) {
    case "parse":
      return { path: "chaos.ts", source: marker };
    case "search":
      return { pattern: "x", haystack: marker, path: "chaos.txt" };
    case "structural":
      return { pattern: "x($A)", source: marker, path: "chaos.ts" };
    case "rewrite":
      return { path: "chaos.ts", source: marker, edits: [] };
    case "ping":
      throw new Error("ping carries no payload to mark");
  }
}

/** Clean params for the same method, used to prove recovery after restart. */
function cleanParams(method: EngineMethod): unknown {
  switch (method) {
    case "parse":
      return { path: "clean.ts", source: "const a = 1" };
    case "search":
      return { pattern: "a", haystack: "a line", path: "clean.txt" };
    case "structural":
      return { pattern: "a($A)", source: "a(1)", path: "clean.ts" };
    case "rewrite":
      return { path: "clean.ts", source: "a", edits: [] };
    case "ping":
      return {};
  }
}

describe("round-trips", () => {
  test("ping and parse answer over real pipes, ready gated on the self-test", async () => {
    const supervisor = makeSupervisor();
    expect(await supervisor.call("ping", {})).toEqual({ pong: true });
    expect(await supervisor.call("parse", { path: "a.ts", source: "const a = [1" })).toMatchObject({
      ok: false,
    });
    expect(supervisor.status().state).toBe("ready");
    expect(supervisor.status().restarts).toBe(0);
  });
});

describe("engine-kill chaos: degraded-not-dead at every kill point", () => {
  const killableMethods = ENGINE_METHODS.filter((m): m is EngineMethod => m !== "ping");

  for (const method of killableMethods) {
    test(`kill mid-${method}: in-flight degrades, next request succeeds`, async () => {
      const supervisor = makeSupervisor({
        WORKBENCH_ENGINE_KILL_ON: method,
        WORKBENCH_ENGINE_KILL_MATCH: MARKER,
      });
      expect(await supervisor.call("ping", {})).toEqual({ pong: true });

      // The planted kill: the child dies with this request in flight.
      await expect(supervisor.call(method, chaosParams(method, MARKER))).rejects.toBeInstanceOf(
        EngineUnavailableError,
      );

      // Degraded, not dead: the same supervisor restarts and answers.
      await expect(supervisor.call(method, cleanParams(method))).resolves.toBeDefined();
      expect(supervisor.status().restarts).toBeGreaterThanOrEqual(1);
    });
  }

  test("kill mid-startup (self-test): calls reject with backoff, never hang", async () => {
    const supervisor = makeSupervisor({ WORKBENCH_ENGINE_KILL_ON: "selftest" });
    await expect(supervisor.call("ping", {})).rejects.toBeInstanceOf(EngineUnavailableError);
    await expect(supervisor.call("ping", {})).rejects.toBeInstanceOf(EngineUnavailableError);
    expect(supervisor.status().consecutiveCrashes).toBeGreaterThanOrEqual(2);
  });

  test("in-flight requests during a crash each get one degraded answer", async () => {
    const supervisor = makeSupervisor({
      WORKBENCH_ENGINE_KILL_ON: "parse",
      WORKBENCH_ENGINE_KILL_MATCH: MARKER,
    });
    await supervisor.call("ping", {});
    const results = await Promise.allSettled([
      supervisor.call("parse", { path: "one.ts", source: MARKER }),
      supervisor.call("parse", { path: "two.ts", source: "const b = 2" }),
    ]);
    // Every in-flight request settles — rejected with a typed error, no hangs.
    for (const r of results) {
      expect(r.status).toBe("rejected");
      if (r.status === "rejected") expect(r.reason).toBeInstanceOf(EngineUnavailableError);
    }
  });
});

describe("repeated crash on one input quarantines the file", () => {
  test("three strikes: quarantined, recorded for doctor, engine stays alive", async () => {
    const dir = mkdtempSync(path.join(tmpdir(), "wb-quarantine-"));
    const log = path.join(dir, "quarantine.jsonl");
    const notified: string[] = [];
    const supervisor = makeSupervisor(
      { WORKBENCH_ENGINE_KILL_ON: "parse", WORKBENCH_ENGINE_KILL_MATCH: "poison.ts" },
      {
        maxStrikes: 3,
        quarantineLog: log,
        onQuarantine: (entry: { path: string }) => notified.push(entry.path),
      },
    );
    try {
      await supervisor.call("ping", {});
      // Three crashes attributed to the same input path.
      for (let i = 0; i < 3; i += 1) {
        await expect(
          supervisor.call("parse", { path: "poison.ts", source: "boom" }),
        ).rejects.toBeInstanceOf(EngineUnavailableError);
      }
      const restartsAtQuarantine = supervisor.status().restarts;

      // The fourth request is refused locally — no crash-loop, no respawn —
      // with the taxonomy's INPUT_QUARANTINED (recovery escalate), not the
      // retryable ENGINE_UNAVAILABLE (§4.3 v1.3 amendment).
      const refusal = supervisor.call("parse", { path: "poison.ts", source: "boom" });
      await expect(refusal).rejects.toBeInstanceOf(InputQuarantinedError);
      await expect(refusal).rejects.toMatchObject({ code: "INPUT_QUARANTINED" });
      expect(supervisor.status().restarts).toBe(restartsAtQuarantine);

      // The engine still serves every other file.
      expect(await supervisor.call("parse", { path: "fine.ts", source: "const a = 1" })).toEqual({
        ok: true,
      });

      // Recorded for doctor: the in-memory report, the JSONL log, the callback.
      const entry = supervisor.quarantineReport().find((e) => e.path === "poison.ts");
      expect(entry?.quarantined).toBe(true);
      expect(entry?.strikes).toBe(3);
      expect(notified).toEqual(["poison.ts"]);
      expect(existsSync(log)).toBe(true);
      const logged = JSON.parse(readFileSync(log, "utf8").trim()) as { path: string };
      expect(logged.path).toBe("poison.ts");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("cancellation (§7.7)", () => {
  test("an aborted request rejects and the child keeps serving", async () => {
    const supervisor = makeSupervisor();
    await supervisor.call("ping", {});
    const controller = new AbortController();
    const pending = supervisor.call(
      "parse",
      { path: "slow.ts", source: "const a = 1" },
      { signal: controller.signal },
    );
    controller.abort();
    await expect(pending).rejects.toBeInstanceOf(RequestCancelledError);
    expect(await supervisor.call("ping", {})).toEqual({ pong: true });
  });
});

describe("the EnginePort bridge", () => {
  test("parse, search and rewrite flow through the port over IPC", async () => {
    const files = new Map([
      ["a.txt", "hay\nneedle here"],
      ["b.txt", "no match"],
    ]);
    const adapter = new IpcEngineAdapter(makeSupervisor(), {
      read: (p) => Promise.resolve(files.get(p) ?? ""),
      list: () => Promise.resolve([...files.keys()]),
    });
    expect(await adapter.parse({ path: "a.ts", content: "const a = [1]" })).toEqual({ ok: true });
    const matches = await adapter.search({ mode: "lexical", pattern: "needle", paths: [] });
    expect(matches).toEqual([
      { location: { path: "a.txt", line: 2, column: 1 }, excerpt: "needle here" },
    ]);
    expect(
      await adapter.rewrite({
        path: "a.md",
        content: "one two",
        edits: [{ find: "one", replace: "1" }],
      }),
    ).toEqual({ content: "1 two", changed: true });
  });
});

describe("engine-kill chaos: the stateful index/* methods", () => {
  const indexDirs: string[] = [];

  // Runs before the module-level supervisor cleanup: children must be dead
  // before Windows lets the fixture directories (holding index.db) go.
  afterEach(async () => {
    await Promise.all(supervisors.splice(0).map((s) => s.stop()));
    for (const dir of indexDirs.splice(0)) {
      rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 });
    }
  });

  /** A tiny indexable workspace the stub core can extract symbols from. */
  function makeIndexFixture(): string {
    const root = mkdtempSync(path.join(tmpdir(), "wb-chaos-index-"));
    indexDirs.push(root);
    mkdirSync(path.join(root, "src"));
    writeFileSync(path.join(root, "src", "alpha.ts"), "function alpha() {}\n");
    writeFileSync(path.join(root, "src", "gamma.ts"), "function gamma() {}\n");
    return root;
  }

  interface ChildStatus {
    readonly tier: 0 | 1;
    readonly building: boolean;
    readonly filesTotal: number;
    readonly filesIndexed: number;
    readonly error?: string;
  }

  /** Unmarked status polls (safe under every planted kill, which matches the
   * marker) until the background build settles. */
  async function settled(supervisor: EngineSupervisor): Promise<ChildStatus> {
    const deadline = Date.now() + 10_000;
    for (;;) {
      const status = (await supervisor.call("index/status", {})) as ChildStatus;
      if (!status.building) return status;
      if (Date.now() > deadline) throw new Error("index build did not settle");
      await Bun.sleep(20);
    }
  }

  /** Marker-carrying params for one index method. The extra `marker` field is
   * ignored by every handler but lands in the frame JSON the kill switch
   * matches; unmarked prelude calls (build, status polls) survive. A method
   * added to INDEX_METHODS fails the `satisfies never` here — it cannot
   * silently escape the sweep. */
  function chaosIndexParams(method: IndexMethod, root: string): unknown {
    switch (method) {
      case "index/build":
        return { root, marker: MARKER };
      case "index/status":
        return { marker: MARKER };
      case "index/symbols":
        return { name: "alpha", marker: MARKER };
      case "index/outline":
        return { paths: ["src/alpha.ts"], marker: MARKER };
      case "index/references":
        return { name: "alpha", marker: MARKER };
      case "index/importGraph":
        return { limit: 5, marker: MARKER };
      case "index/upsert":
        return { path: "src/alpha.ts", marker: MARKER };
      case "index/remove":
        return { path: "src/gamma.ts", marker: MARKER };
      case "index/testRanges":
        return { paths: ["src/alpha.ts"], marker: MARKER };
      default:
        throw new Error(`no chaos params for index method ${method satisfies never}`);
    }
  }

  /** Clean params for the same method, to prove recovery after restart. */
  function cleanIndexParams(method: IndexMethod, root: string): unknown {
    switch (method) {
      case "index/build":
        return { root };
      case "index/status":
        return {};
      case "index/symbols":
        return { name: "alpha" };
      case "index/outline":
        return { paths: ["src/alpha.ts"] };
      case "index/references":
        return { name: "alpha" };
      case "index/importGraph":
        return { limit: 5 };
      case "index/upsert":
        return { path: "src/alpha.ts" };
      case "index/remove":
        return { path: "src/gamma.ts" };
      case "index/testRanges":
        return { paths: ["src/alpha.ts"] };
      default:
        throw new Error(`no clean params for index method ${method satisfies never}`);
    }
  }

  for (const method of INDEX_METHODS) {
    test(`kill mid-${method}: in-flight degrades, next request succeeds`, async () => {
      const root = makeIndexFixture();
      const supervisor = makeSupervisor({
        WORKBENCH_ENGINE_KILL_ON: method,
        WORKBENCH_ENGINE_KILL_MATCH: MARKER,
      });
      expect(await supervisor.call("ping", {})).toEqual({ pong: true });

      // Unmarked prelude: build and settle so every method has an index.
      await supervisor.call("index/build", { root });
      await settled(supervisor);

      // The planted kill: the marked frame dies with the request in flight.
      await expect(supervisor.call(method, chaosIndexParams(method, root))).rejects.toBeInstanceOf(
        EngineUnavailableError,
      );

      // Degraded, not dead: the respawned child holds no indexer (that state
      // died with it), so recovery is a fresh cache-class rebuild — then a
      // clean answer from the same method that was killed. Await-then-assert
      // rather than expect(promise).resolves: under bun test the .resolves
      // matcher starves an already-running Bun.spawn stdout iterator, so the
      // supervisor's read loop never delivers the response (bun 1.4.0).
      await supervisor.call("index/build", { root });
      await settled(supervisor);
      const clean = await supervisor.call(method, cleanIndexParams(method, root));
      expect(clean).toBeDefined();
      expect(supervisor.status().restarts).toBeGreaterThanOrEqual(1);
    }, 20_000);
  }

  test("a kill mid-build leaves cache-class index.db rebuildable, never quarantined", async () => {
    const root = makeIndexFixture();
    // Enough files that the interrupted rebuild is genuinely in flight when
    // the marked status frame lands (partial batches may have committed —
    // the recovery contract below holds for empty, partial, and complete).
    for (let i = 0; i < 40; i += 1) {
      writeFileSync(path.join(root, "src", `extra${i}.ts`), `function extra${i}() {}\n`);
    }
    const supervisor = makeSupervisor({
      WORKBENCH_ENGINE_KILL_ON: "index/status",
      WORKBENCH_ENGINE_KILL_MATCH: MARKER,
    });

    // Generation 1: a complete, warm index.
    await supervisor.call("index/build", { root });
    await settled(supervisor);
    const warm = (await supervisor.call("index/symbols", { name: "gamma" })) as {
      symbols: unknown[];
    };
    expect(warm.symbols).toHaveLength(1);

    // Mutate the workspace, kick a rebuild, and kill the child mid-build:
    // the marked status frame dies at receipt with the build in flight.
    rmSync(path.join(root, "src", "gamma.ts"));
    writeFileSync(path.join(root, "src", "delta.ts"), "function delta() {}\n");
    await supervisor.call("index/build", { root });
    await expect(supervisor.call("index/status", { marker: MARKER })).rejects.toBeInstanceOf(
      EngineUnavailableError,
    );

    // The abandoned database still carries its version stamp: the next open
    // re-runs the migration check and finds nothing to migrate.
    const dbPath = path.join(root, ".workbench", "index.db");
    const raw = new Database(dbPath, { readonly: true });
    const stamped = (raw.query("PRAGMA user_version").get() as { user_version: number })
      .user_version;
    raw.close();
    expect(stamped).toBe(INDEX_SCHEMA_VERSION);

    // Recovery is rebuild: the respawned child re-opens the same file and
    // converges — stale rows dropped (idempotent re-index), new file
    // present, tier 1 again.
    await supervisor.call("index/build", { root });
    const status = await settled(supervisor);
    expect(status.tier).toBe(1);
    expect(status.error).toBeUndefined();
    expect(status.filesIndexed).toBe(status.filesTotal);
    const gone = (await supervisor.call("index/symbols", { name: "gamma" })) as {
      symbols: unknown[];
    };
    expect(gone.symbols).toHaveLength(0);
    const added = (await supervisor.call("index/symbols", { name: "delta" })) as {
      symbols: unknown[];
    };
    expect(added.symbols).toHaveLength(1);

    // Cache-class recovery is rebuild, never quarantine (m1-migrations):
    // no index.db.quarantined* artifact may exist after the round trip.
    const artifacts = readdirSync(path.join(root, ".workbench"));
    expect(artifacts.some((name) => name.includes(".quarantined"))).toBe(false);

    const after = new Database(dbPath, { readonly: true });
    const stampedAfter = (after.query("PRAGMA user_version").get() as { user_version: number })
      .user_version;
    after.close();
    expect(stampedAfter).toBe(INDEX_SCHEMA_VERSION);
  }, 20_000);
});
