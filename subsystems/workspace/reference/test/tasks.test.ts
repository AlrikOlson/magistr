/**
 * The Tasks extension (SDD §5.1, §7.3.2), chunk m1-tasks-integration:
 *
 *  - `indexBuildTask` derives the one `index/build` task purely from
 *    IndexPort.status() — no task store, nothing to leak between requests.
 *  - The served surface: the server declares the `tasks` capability and
 *    answers `tasks/list` + `tasks/get` over linked in-memory transports,
 *    with LIVE progress — the same client sees the statusMessage advance as
 *    the (fake) build commits batches, which is the acceptance criterion
 *    "tasks/get returns live progress during a build" made deterministic.
 *
 * The end-to-end flows against a real spawned server on both transports live
 * in mcp-conformance.test.ts.
 */

import { describe, expect, test } from "bun:test";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { GetTaskResultSchema, ListTasksResultSchema } from "@modelcontextprotocol/sdk/types.js";
import { defaultResolvedConfig } from "../src/config";
import type {
  ImportGraphResult,
  IndexPort,
  IndexStatus,
  OutlineQueryResult,
  ReferenceQueryResult,
  SymbolQueryResult,
} from "../src/ports";
import { PORT_VERSIONS } from "../src/ports";
import {
  buildServer,
  INDEX_BUILD_TASK_ID,
  indexBuildTask,
  type WorkspaceRuntime,
} from "../src/protocol";
import type { SearchDeps } from "../src/tools";

const T0 = 1_755_750_000_000; // fixed epoch ms; the derivation is pure in it
const ISO_T0 = new Date(T0).toISOString();

describe("indexBuildTask derivation", () => {
  test("no status or no build kicked: no task", () => {
    expect(indexBuildTask(null)).toBeNull();
    expect(indexBuildTask({ tier: 1, generation: 3, building: false })).toBeNull();
  });

  test("building: working with X/Y progress and a poll interval", () => {
    const task = indexBuildTask({
      tier: 0,
      generation: 0,
      building: true,
      filesTotal: 120,
      filesIndexed: 3,
      buildStartedAtMs: T0,
      buildUpdatedAtMs: T0 + 500,
    });
    expect(task).not.toBeNull();
    expect(task?.taskId).toBe(INDEX_BUILD_TASK_ID);
    expect(task?.status).toBe("working");
    expect(task?.statusMessage).toBe("index build in progress: 3/120 files");
    expect(task?.pollInterval).toBeGreaterThan(0);
    expect(task?.ttl).toBeNull();
    expect(task?.createdAt).toBe(ISO_T0);
    expect(task?.lastUpdatedAt).toBe(new Date(T0 + 500).toISOString());
  });

  test("failed build: failed, error verbatim in the statusMessage", () => {
    const task = indexBuildTask({
      tier: 0,
      generation: 0,
      building: false,
      buildStartedAtMs: T0,
      buildUpdatedAtMs: T0 + 900,
      lastError: "scan blew up",
    });
    expect(task?.status).toBe("failed");
    expect(task?.statusMessage).toBe("index build failed: scan blew up");
  });

  test("warm index after a build: completed with generation and file count", () => {
    const task = indexBuildTask({
      tier: 1,
      generation: 4,
      building: false,
      filesTotal: 120,
      filesIndexed: 120,
      buildStartedAtMs: T0,
      buildUpdatedAtMs: T0 + 900,
    });
    expect(task?.status).toBe("completed");
    expect(task?.statusMessage).toBe("index at tier 1, generation 4 (120 files)");
  });

  test("settle race (kicked, tier 0, no error, not building): still working", () => {
    const task = indexBuildTask({
      tier: 0,
      generation: 0,
      building: false,
      filesTotal: 120,
      filesIndexed: 120,
      buildStartedAtMs: T0,
    });
    expect(task?.status).toBe("working");
    // No buildUpdatedAtMs: lastUpdatedAt falls back to createdAt.
    expect(task?.lastUpdatedAt).toBe(ISO_T0);
  });
});

/** A search-deps fake whose index status the test mutates mid-flight. */
function fakeSearch(statusRef: { current: IndexStatus }): SearchDeps {
  const index: IndexPort = {
    version: PORT_VERSIONS.index,
    status: (): Promise<IndexStatus> => Promise.resolve(statusRef.current),
    build: () => Promise.resolve(statusRef.current),
    querySymbols: (): Promise<SymbolQueryResult> =>
      Promise.resolve({ symbols: [], freshness: "warm" }),
    queryReferences: (): Promise<ReferenceQueryResult> =>
      Promise.resolve({ edges: [], freshness: "warm" }),
    queryOutline: (): Promise<OutlineQueryResult> =>
      Promise.resolve({ files: [], freshness: "warm" }),
    queryImportGraph: (): Promise<ImportGraphResult> =>
      Promise.resolve({ fanIn: [], fanOut: [], freshness: "warm" }),
    upsertFile: () => Promise.resolve(),
    refreshFile: () => Promise.resolve(),
    removeFile: () => Promise.resolve(),
  };
  return {
    engine: {
      version: PORT_VERSIONS.engine,
      parse: () => Promise.reject(new Error("not under test")),
      search: () => Promise.resolve([]),
      rewrite: () => Promise.reject(new Error("not under test")),
    },
    index,
    workspace: {
      list: () => Promise.resolve([]),
      read: () => Promise.reject(new Error("not under test")),
    },
  };
}

const WORKSPACE: WorkspaceRuntime = {
  root: "/w",
  stateDir: "/w/.workbench",
  detection: { networkFs: false, method: "default-local" },
  journalMode: "WAL",
  appliedJournalMode: "wal",
  config: defaultResolvedConfig(),
  notes: [],
  close: () => {},
};

async function connectedClient(statusRef: { current: IndexStatus }): Promise<Client> {
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  const server = buildServer(WORKSPACE, fakeSearch(statusRef));
  const client = new Client({ name: "tasks-test", version: "0.0.0" });
  await server.connect(serverTransport);
  await client.connect(clientTransport);
  return client;
}

describe("tasks/* over the server", () => {
  test("capability declared; list and get serve live progress during a build", async () => {
    const statusRef: { current: IndexStatus } = {
      current: {
        tier: 0,
        generation: 0,
        building: true,
        filesTotal: 120,
        filesIndexed: 3,
        buildStartedAtMs: T0,
        buildUpdatedAtMs: T0 + 100,
      },
    };
    const client = await connectedClient(statusRef);
    try {
      expect(client.getServerCapabilities()?.tasks).toBeDefined();
      expect(client.getServerCapabilities()?.tasks?.list).toBeDefined();

      const listed = await client.request(
        { method: "tasks/list", params: {} },
        ListTasksResultSchema,
      );
      expect(listed.tasks).toHaveLength(1);
      expect(listed.tasks[0]?.taskId).toBe(INDEX_BUILD_TASK_ID);
      expect(listed.tasks[0]?.status).toBe("working");

      const early = await client.request(
        { method: "tasks/get", params: { taskId: INDEX_BUILD_TASK_ID } },
        GetTaskResultSchema,
      );
      expect(early.statusMessage).toBe("index build in progress: 3/120 files");

      // The build commits more batches; the SAME task id now reports them.
      statusRef.current = { ...statusRef.current, filesIndexed: 100, buildUpdatedAtMs: T0 + 800 };
      const later = await client.request(
        { method: "tasks/get", params: { taskId: INDEX_BUILD_TASK_ID } },
        GetTaskResultSchema,
      );
      expect(later.status).toBe("working");
      expect(later.statusMessage).toBe("index build in progress: 100/120 files");

      // The build settles warm; the task completes.
      statusRef.current = {
        tier: 1,
        generation: 1,
        building: false,
        filesTotal: 120,
        filesIndexed: 120,
        buildStartedAtMs: T0,
        buildUpdatedAtMs: T0 + 900,
      };
      const done = await client.request(
        { method: "tasks/get", params: { taskId: INDEX_BUILD_TASK_ID } },
        GetTaskResultSchema,
      );
      expect(done.status).toBe("completed");
      expect(done.statusMessage).toBe("index at tier 1, generation 1 (120 files)");
    } finally {
      await client.close();
    }
  });

  test("unknown task id and never-kicked build both refuse cleanly", async () => {
    const statusRef: { current: IndexStatus } = {
      current: { tier: 1, generation: 2, building: false },
    };
    const client = await connectedClient(statusRef);
    try {
      // No build ran in this engine child's lifetime: nothing to poll.
      const listed = await client.request(
        { method: "tasks/list", params: {} },
        ListTasksResultSchema,
      );
      expect(listed.tasks).toHaveLength(0);
      await expect(
        client.request(
          { method: "tasks/get", params: { taskId: INDEX_BUILD_TASK_ID } },
          GetTaskResultSchema,
        ),
      ).rejects.toThrow("Task not found");
      await expect(
        client.request(
          { method: "tasks/get", params: { taskId: "no-such-task" } },
          GetTaskResultSchema,
        ),
      ).rejects.toThrow("Task not found");
    } finally {
      await client.close();
    }
  });
});
