/**
 * MCP conformance on both v1 transports (SDD §5.2, M0 gate): the official SDK
 * client against a REAL spawned `bun src/main.ts` server — initialize
 * handshake, tools/list, tools/call, envelope-version negotiation via _meta,
 * schema validity of every returned envelope against the normative
 * `schemas/envelope.schema.json`, and the Tasks-extension flows (SDD §5.1,
 * chunk m1-tasks-integration): the first-start build registers as the
 * pollable `index/build` task, tasks/get polls it to terminal, tasks/list
 * names it, and an unknown id is refused.
 */

import { afterAll, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { StreamableHTTPClientTransport } from "@modelcontextprotocol/sdk/client/streamableHttp.js";
import type { Transport } from "@modelcontextprotocol/sdk/shared/transport.js";
import { GetTaskResultSchema, ListTasksResultSchema } from "@modelcontextprotocol/sdk/types.js";
import envelopeSchema from "../schemas/envelope.schema.json";
import type { ResponseEnvelope } from "../src/envelope";
import { envelopeValidatorFor } from "./support/ajv";

const REPO = path.resolve(import.meta.dir, "..");
const MAIN = path.join(REPO, "src", "main.ts");
const { validate, errorsText } = envelopeValidatorFor(envelopeSchema);

const roots: string[] = [];
function tmpWorkspace(): string {
  const root = mkdtempSync(path.join(tmpdir(), "workbench-conf-"));
  roots.push(root);
  return root;
}

afterAll(() => {
  for (const root of roots.splice(0)) {
    try {
      rmSync(root, { recursive: true, force: true });
    } catch {
      // Windows can hold the spawned server's SQLite handles (journal.db,
      // index.db) for a beat after process exit; leak the tempdir rather
      // than fail the suite on teardown.
    }
  }
});

/** Local-forced env so a network-mounted CI tmpdir cannot skew the suite. */
const LOCAL_ENV = { ...process.env, WORKBENCH_NETWORK_FS: "0" } as Record<string, string>;

function envelopeFrom(result: unknown): ResponseEnvelope {
  const content = (result as { content: Array<{ type: string; text: string }> }).content;
  expect(content).toHaveLength(1);
  const first = content[0];
  if (first === undefined) throw new Error("empty content");
  expect(first.type).toBe("text");
  return JSON.parse(first.text) as ResponseEnvelope;
}

function assertValidEnvelope(envelope: ResponseEnvelope): void {
  expect(validate(envelope) ? "" : errorsText()).toBe("");
}

/** The conformance assertions, transport-agnostic: run against a connected client. */
async function runConformance(client: Client): Promise<void> {
  // initialize happened inside connect(); the server identified itself.
  expect(client.getServerVersion()?.name).toBe("workbench-mcp");

  const tools = await client.listTools();
  const status = tools.tools.find((t) => t.name === "workbench_status");
  expect(status).toBeDefined();
  expect(tools.tools.find((t) => t.name === "search")).toBeDefined();
  expect(tools.tools.find((t) => t.name === "map")).toBeDefined();
  expect(tools.tools.find((t) => t.name === "read")).toBeDefined();

  // Plain call: latest envelope version, schema-valid, WAL on local disk.
  const plain = envelopeFrom(await client.callTool({ name: "workbench_status", arguments: {} }));
  assertValidEnvelope(plain);
  expect(plain.v).toBe(1);
  expect(plain.ok).toBe(true);
  if (plain.ok) {
    const [info] = plain.results as Array<{ journalMode: string; networkFs: boolean }>;
    expect(info?.journalMode).toBe("WAL");
    expect(info?.networkFs).toBe(false);
  }

  // Pinned to a future max: answered in the highest mutual version (1).
  const pinnedHigh = envelopeFrom(
    await client.callTool({
      name: "workbench_status",
      arguments: {},
      _meta: { "workbench/maxEnvelopeVersion": 99 },
    }),
  );
  assertValidEnvelope(pinnedHigh);
  expect(pinnedHigh.v).toBe(1);
  expect(pinnedHigh.ok).toBe(true);

  // Garbled pin: refused with the taxonomy code, not silently upgraded.
  const refused = envelopeFrom(
    await client.callTool({
      name: "workbench_status",
      arguments: {},
      _meta: { "workbench/maxEnvelopeVersion": "one" },
    }),
  );
  assertValidEnvelope(refused);
  expect(refused.ok).toBe(false);
  if (!refused.ok) {
    expect(refused.error.code).toBe("UNSUPPORTED_VERSION");
    expect(refused.error.recovery).toBe("abort");
  }

  // Tasks extension (SDD §5.1, chunk m1-tasks-integration): declared, and the
  // first-start index build registers as the pollable index/build task.
  expect(client.getServerCapabilities()?.tasks).toBeDefined();

  // The first search kicks the Tier-1 build (SDD §7.3.2, non-blocking).
  assertValidEnvelope(
    envelopeFrom(
      await client.callTool({ name: "search", arguments: { query: "x", mode: "lexical" } }),
    ),
  );

  // Poll tasks/get until the build lands terminal; every intermediate answer
  // must already be the index/build task in a legal status.
  const deadline = Date.now() + 15_000;
  let last: { status: string; statusMessage?: string | undefined } | undefined;
  for (;;) {
    const task = await client.request(
      { method: "tasks/get", params: { taskId: "index/build" } },
      GetTaskResultSchema,
    );
    expect(task.taskId).toBe("index/build");
    expect(["working", "completed", "failed"]).toContain(task.status);
    last = task;
    if (task.status !== "working" || Date.now() > deadline) break;
    await new Promise((resolve) => setTimeout(resolve, task.pollInterval ?? 100));
  }
  expect(last?.status).toBe("completed");
  expect(last?.statusMessage).toContain("index at tier 1");

  const listed = await client.request({ method: "tasks/list", params: {} }, ListTasksResultSchema);
  expect(listed.tasks.map((t) => t.taskId)).toContain("index/build");

  // An id the workspace never had is refused, not invented. (Asserted via
  // settle-and-inspect: bun's expect().rejects deadlocks on an in-flight
  // stdio request under `bun test`.)
  const refusal = await client
    .request({ method: "tasks/get", params: { taskId: "no-such" } }, GetTaskResultSchema)
    .then(
      () => "resolved",
      (error: unknown) => String(error),
    );
  expect(refusal).toContain("Task not found");
}

describe("stdio transport", () => {
  test("conformance: initialize, tools/list, tools/call, negotiation", async () => {
    const client = new Client({ name: "conformance-stdio", version: "0.0.0" });
    const transport = new StdioClientTransport({
      command: process.execPath,
      args: [MAIN, tmpWorkspace()],
      env: LOCAL_ENV,
      cwd: REPO,
    });
    await client.connect(transport);
    try {
      await runConformance(client);
    } finally {
      await client.close();
    }
  }, 30_000);
});

describe("streamable http transport", () => {
  test("conformance: initialize, tools/list, tools/call, negotiation; stateless across clients", async () => {
    const proc = Bun.spawn({
      cmd: [process.execPath, MAIN, tmpWorkspace(), "--http", "0"],
      cwd: REPO,
      env: LOCAL_ENV,
      stdout: "ignore",
      stderr: "pipe",
    });
    try {
      // The chosen port is announced on stderr.
      const reader = proc.stderr.getReader();
      let banner = "";
      let port: number | undefined;
      while (port === undefined) {
        const { value, done } = await reader.read();
        if (done) throw new Error(`server exited before announcing a port: ${banner}`);
        banner += new TextDecoder().decode(value);
        port = /listening on port (\d+)/.exec(banner)?.[1] as unknown as number | undefined;
        if (port !== undefined) port = Number(port);
      }
      reader.releaseLock();

      const url = new URL(`http://127.0.0.1:${port}/mcp`);
      // The SDK's own transport class narrows `sessionId` differently from its
      // Transport interface under exactOptionalPropertyTypes — cast at the seam.
      const first = new Client({ name: "conformance-http", version: "0.0.0" });
      await first.connect(new StreamableHTTPClientTransport(url) as unknown as Transport);
      try {
        await runConformance(first);
      } finally {
        await first.close();
      }

      // A second, unrelated client succeeds with no session carried over —
      // the stateless profile has no session to resume or to miss.
      const second = new Client({ name: "conformance-http-2", version: "0.0.0" });
      await second.connect(new StreamableHTTPClientTransport(url) as unknown as Transport);
      try {
        const envelope = envelopeFrom(
          await second.callTool({ name: "workbench_status", arguments: {} }),
        );
        assertValidEnvelope(envelope);
        expect(envelope.ok).toBe(true);
      } finally {
        await second.close();
      }
    } finally {
      proc.kill();
      await proc.exited;
    }
  }, 30_000);
});
