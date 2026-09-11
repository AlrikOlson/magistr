/**
 * The local tracing ring buffer (feature 005-tracing-ring-buffer, SDD §11.2):
 *
 *  - US1 emission: every served request — ok or failing — produces one
 *    `tool:` root line in `.workbench/trace.jsonl` with duration and
 *    taxonomy status, over a fresh unconfigured workspace (SC-001).
 *  - US2 linkage + misuse extraction against a REAL engine child: `engine:`
 *    children share the root's traceId and carry parentSpanId; the 5x
 *    INDEX_UNAVAILABLE retry loop is extracted from the file alone (SC-002).
 *  - US3 immunity: an unwritable destination changes no envelope byte
 *    (SC-004); the cap evicts oldest-first and torn lines are tolerated; the
 *    recorded traced-vs-noop latency comparison (SC-005, Constitution VI).
 *  - US4 privacy: a sentinel planted in tool arguments never reaches the
 *    record (SC-003), and the tracing modules carry no network capability —
 *    pinned by source sweep, m5-gate style (research R6).
 *  - Contract conformance: every emitted line validates against
 *    contracts/trace-record.md with reject-unknown on fields and attributes.
 *  - Composition: the REAL server writes the file by default and writes
 *    nothing under `--config tracing.enabled=false` (FR-005).
 */

import { afterAll, describe, expect, test } from "bun:test";
import {
  appendFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { EngineSupervisor, IpcIndexAdapter } from "../src/adapters/engine-ipc";
import { defaultResolvedConfig } from "../src/config";
import { isErrorCode, recoveryFor } from "../src/envelope";
import type {
  ImportGraphResult,
  IndexPort,
  IndexStatus,
  OutlineQueryResult,
  ReferenceQueryResult,
  SymbolQueryResult,
} from "../src/ports";
import { noopTracer, PORT_VERSIONS, type Tracer } from "../src/ports";
import { buildServer, type WorkspaceRuntime } from "../src/protocol";
import { JsonlTracer, TRACE_LOG_CAP_BYTES, TRACE_RECORD_VERSION } from "../src/protocol/tracing";
import type { SearchDeps } from "../src/tools";

const REPO = path.resolve(import.meta.dir, "..");
const MAIN = path.join(REPO, "src", "main.ts");
const SENTINEL = "SENTINEL-trace-9c41";

const roots: string[] = [];
function tmpWorkspace(): string {
  const root = mkdtempSync(path.join(tmpdir(), "workbench-trace-"));
  roots.push(root);
  return root;
}

afterAll(() => {
  for (const root of roots.splice(0)) {
    try {
      rmSync(root, { recursive: true, force: true });
    } catch {
      // Windows can hold spawned-server SQLite handles briefly; leak the
      // tempdir rather than fail teardown.
    }
  }
});

function workspaceAt(root: string): WorkspaceRuntime {
  return {
    root,
    stateDir: path.join(root, ".workbench"),
    detection: { networkFs: false, method: "default-local" },
    journalMode: "WAL",
    appliedJournalMode: "wal",
    config: defaultResolvedConfig(),
    notes: [],
    close: () => {},
  };
}

/** Cold fake index: tier 0, so references-mode search refuses with INDEX_UNAVAILABLE. */
function coldFakeSearch(): SearchDeps {
  const status: IndexStatus = { tier: 0, generation: 0, building: false };
  const index: IndexPort = {
    version: PORT_VERSIONS.index,
    status: (): Promise<IndexStatus> => Promise.resolve(status),
    build: () => Promise.resolve(status),
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

async function connectedClient(server: ReturnType<typeof buildServer>): Promise<Client> {
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  const client = new Client({ name: "tracing-test", version: "0.0.0" });
  await server.connect(serverTransport);
  await client.connect(clientTransport);
  return client;
}

interface TraceLine {
  readonly v: number;
  readonly traceId: string;
  readonly spanId: string;
  readonly parentSpanId?: string;
  readonly name: string;
  readonly startUnixMs: number;
  readonly durationMs: number;
  readonly status: { ok: true } | { ok: false; code: string; recovery: string };
  readonly attributes: Record<string, unknown>;
}

/** Readers tolerate torn/malformed lines (quarantine-log precedent). */
function readTrace(tracePath: string): TraceLine[] {
  if (!existsSync(tracePath)) return [];
  const records: TraceLine[] = [];
  for (const line of readFileSync(tracePath, "utf8").split("\n")) {
    if (line.length === 0) continue;
    try {
      records.push(JSON.parse(line) as TraceLine);
    } catch {
      // torn tail — skipped by contract
    }
  }
  return records;
}

const RECORD_KEYS = new Set([
  "v",
  "traceId",
  "spanId",
  "parentSpanId",
  "name",
  "startUnixMs",
  "durationMs",
  "status",
  "attributes",
]);
const ROOT_ATTR_KEYS = new Set(["tool", "envelopeV", "notesCount", "truncated"]);
const CHILD_ATTR_KEYS = new Set(["method"]);

/** contracts/trace-record.md conformance, reject-unknown (T014). */
function assertContract(records: readonly TraceLine[]): void {
  expect(records.length).toBeGreaterThan(0);
  for (const record of records) {
    for (const key of Object.keys(record)) {
      expect(RECORD_KEYS.has(key) ? "" : `unknown field ${key}`).toBe("");
    }
    expect(record.v).toBe(TRACE_RECORD_VERSION);
    expect(/^[0-9a-f]{32}$/.test(record.traceId)).toBe(true);
    expect(/^[0-9a-f]{16}$/.test(record.spanId)).toBe(true);
    expect(/^(tool|engine):/.test(record.name)).toBe(true);
    expect(typeof record.startUnixMs).toBe("number");
    expect(record.durationMs).toBeGreaterThanOrEqual(0);
    if (record.status.ok !== true) {
      expect(isErrorCode(record.status.code) ? "" : `unknown code ${record.status.code}`).toBe("");
      if (isErrorCode(record.status.code)) {
        expect(record.status.recovery).toBe(recoveryFor(record.status.code));
      }
    }
    const allow = record.name.startsWith("tool:") ? ROOT_ATTR_KEYS : CHILD_ATTR_KEYS;
    for (const key of Object.keys(record.attributes)) {
      expect(allow.has(key) ? "" : `attribute ${key} outside allowlist`).toBe("");
    }
    if (record.name.startsWith("tool:")) {
      expect(record.parentSpanId).toBeUndefined();
      expect(record.attributes["tool"]).toBe(record.name.slice("tool:".length));
    } else {
      expect(record.attributes["method"]).toBe(record.name.slice("engine:".length));
    }
  }
}

describe("US1: flight recorder on by default (SC-001)", () => {
  test("one root line per served request; failures carry taxonomy code + recovery", async () => {
    const root = tmpWorkspace();
    const ws = workspaceAt(root);
    mkdirSync(ws.stateDir, { recursive: true });
    const tracePath = path.join(ws.stateDir, "trace.jsonl");
    const tracer = new JsonlTracer(tracePath);
    const server = buildServer(
      ws,
      coldFakeSearch(),
      undefined,
      undefined,
      undefined,
      undefined,
      tracer,
    );
    const client = await connectedClient(server);
    try {
      await client.callTool({ name: "workbench_status", arguments: {} });
      await client.callTool({ name: "search", arguments: { query: "hello" } });
      await client.callTool({
        name: "search",
        arguments: { query: "someSymbol", mode: "references" },
      });
    } finally {
      await client.close();
    }
    await tracer.settle();

    const records = readTrace(tracePath);
    assertContract(records);
    const tools = records.filter((r) => r.name.startsWith("tool:"));
    expect(tools.map((r) => r.name)).toEqual([
      "tool:workbench_status",
      "tool:search",
      "tool:search",
    ]);
    expect(tools[0]?.status.ok).toBe(true);
    expect(tools[1]?.status.ok).toBe(true);
    const failing = tools[2];
    expect(failing?.status).toEqual({ ok: false, code: "INDEX_UNAVAILABLE", recovery: "retry" });
    for (const record of tools) {
      expect(Math.abs(Date.now() - record.startUnixMs)).toBeLessThan(60_000);
      expect(record.attributes["envelopeV"]).toBe(1);
    }
  });
});

describe("US2: misuse patterns + linkage over a REAL engine child (SC-002)", () => {
  test("5x retry loop extractable; engine children linked to their roots", async () => {
    const root = tmpWorkspace();
    const ws = workspaceAt(root);
    mkdirSync(ws.stateDir, { recursive: true });
    const tracePath = path.join(ws.stateDir, "trace.jsonl");
    const tracer = new JsonlTracer(tracePath);
    const supervisor = new EngineSupervisor({
      env: { WORKBENCH_ENGINE_STUB_CORE: "1" },
      backoffMs: [0, 10, 20],
      readyTimeoutMs: 10_000,
      tracer,
    });
    try {
      const cold = coldFakeSearch();
      const search: SearchDeps = {
        ...cold,
        index: new IpcIndexAdapter(supervisor, root, { journalMode: "WAL" }),
      };
      const server = buildServer(ws, search, undefined, undefined, undefined, undefined, tracer);
      const client = await connectedClient(server);
      try {
        // The deliberate misuse pattern: the same failing call, five times.
        for (let i = 0; i < 5; i++) {
          await client.callTool({
            name: "search",
            arguments: { query: "someSymbol", mode: "references" },
          });
        }
      } finally {
        await client.close();
      }
      await tracer.settle();

      const records = readTrace(tracePath);
      assertContract(records);

      // SC-002: extracted from the file alone — five same-tool, same-code
      // roots in sequence.
      const retryLoop = records.filter(
        (r) =>
          r.name === "tool:search" &&
          r.status.ok === false &&
          r.status.code === "INDEX_UNAVAILABLE",
      );
      expect(retryLoop).toHaveLength(5);

      // FR-002: reconstruct one request's tree — root + engine children
      // sharing traceId, children carrying parentSpanId = root spanId.
      const anyRoot = retryLoop[0];
      if (anyRoot === undefined) throw new Error("unreachable");
      const children = records.filter((r) => r.parentSpanId === anyRoot.spanId);
      expect(children.length).toBeGreaterThan(0);
      for (const child of children) {
        expect(child.traceId).toBe(anyRoot.traceId);
        expect(child.name.startsWith("engine:")).toBe(true);
        expect(typeof child.attributes["method"]).toBe("string");
      }
    } finally {
      await supervisor.stop();
    }
  }, 30_000);
});

describe("US3: tracing can never hurt (SC-004, SC-005)", () => {
  test("unwritable destination: every envelope byte-identical to the noop run", async () => {
    const root = tmpWorkspace();
    const ws = workspaceAt(root);
    const brokenDir = path.join(root, "no-such-dir", "deeper");
    const broken = new JsonlTracer(path.join(brokenDir, "trace.jsonl"));

    const calls = [
      { name: "workbench_status", arguments: {} },
      { name: "search", arguments: { query: "hello" } },
      { name: "search", arguments: { query: "someSymbol", mode: "references" } },
    ];
    const run = async (tracer: Tracer): Promise<string[]> => {
      const server = buildServer(
        ws,
        coldFakeSearch(),
        undefined,
        undefined,
        undefined,
        undefined,
        tracer,
      );
      const client = await connectedClient(server);
      const envelopes: string[] = [];
      try {
        for (const call of calls) {
          const result = await client.callTool(call);
          const content = (result as { content: Array<{ text: string }> }).content;
          envelopes.push(content[0]?.text ?? "");
        }
      } finally {
        await client.close();
      }
      return envelopes;
    };

    const untraced = await run(noopTracer);
    const traced = await run(broken);
    expect(traced).toEqual(untraced);
    await broken.settle();
    expect(existsSync(brokenDir)).toBe(false);
  });

  test("cap: size stays ≤ cap, newest lines survive, torn tail tolerated", async () => {
    const root = tmpWorkspace();
    const stateDir = path.join(root, ".workbench");
    mkdirSync(stateDir, { recursive: true });
    const tracePath = path.join(stateDir, "trace.jsonl");

    // Fill just under the cap with numbered filler lines, so the next append
    // breaches it.
    const payload = "x".repeat(1024);
    const filler: string[] = [];
    let size = 0;
    for (let i = 0; size < TRACE_LOG_CAP_BYTES - 512; i++) {
      const line = JSON.stringify({ seq: i, payload });
      filler.push(line);
      size += line.length + 1;
    }
    writeFileSync(tracePath, `${filler.join("\n")}\n`);

    const tracer = new JsonlTracer(tracePath);
    tracer.startToolSpan({ tool: "cap-probe" }).end({ ok: true });
    await tracer.settle();

    const stats = readFileSync(tracePath, "utf8");
    expect(stats.length).toBeLessThanOrEqual(TRACE_LOG_CAP_BYTES);
    const lines = stats.split("\n").filter((l) => l.length > 0);
    // Newest half of the old lines plus the incoming record survive; the
    // oldest lines are gone.
    expect(lines.length).toBe(Math.floor(filler.length / 2) + 1);
    const first = JSON.parse(lines[0] ?? "{}") as { seq?: number };
    expect(first.seq).toBe(Math.ceil(filler.length / 2));
    expect(lines[lines.length - 1]).toContain('"tool:cap-probe"');

    // Torn tail: a half-written line breaks no reader and no later append.
    // The writer never repairs (contract): the record appended directly onto
    // the tear merges into it and is lost — cache-class history, not
    // correctness — and everything after that is intact.
    appendFileSync(tracePath, '{"v":1,"truncated-mid-');
    tracer.startToolSpan({ tool: "merged-into-tear" }).end({ ok: true });
    tracer.startToolSpan({ tool: "after-tear" }).end({ ok: true });
    await tracer.settle();
    const records = readTrace(tracePath);
    expect(records.some((r) => r.name === "tool:after-tear")).toBe(true);
    expect(records.some((r) => r.name === "tool:merged-into-tear")).toBe(false);
  });

  test("SC-005: recorded traced-vs-noop latency comparison (Constitution VI)", async () => {
    const root = tmpWorkspace();
    const ws = workspaceAt(root);
    mkdirSync(ws.stateDir, { recursive: true });
    const tracer = new JsonlTracer(path.join(ws.stateDir, "trace.jsonl"));

    const measure = async (t: Tracer, n: number): Promise<number> => {
      const server = buildServer(
        ws,
        coldFakeSearch(),
        undefined,
        undefined,
        undefined,
        undefined,
        t,
      );
      const client = await connectedClient(server);
      try {
        for (let i = 0; i < 20; i++) {
          await client.callTool({ name: "workbench_status", arguments: {} });
        }
        const start = performance.now();
        for (let i = 0; i < n; i++) {
          await client.callTool({ name: "workbench_status", arguments: {} });
        }
        return (performance.now() - start) / n;
      } finally {
        await client.close();
      }
    };

    const N = 200;
    const noop = await measure(noopTracer, N);
    const traced = await measure(tracer, N);
    await tracer.settle();
    // The recorded number (Constitution VI): fire-and-forget appends keep the
    // served path within measurement noise. The bound is deliberately loose —
    // it catches a synchronous-write regression, not scheduler jitter.
    console.log(
      `SC-005 recorded: traced ${traced.toFixed(3)} ms/call vs noop ${noop.toFixed(3)} ms/call (N=${N})`,
    );
    expect(traced).toBeLessThan(noop * 3 + 2);
  }, 60_000);
});

describe("US4: privacy is structural (SC-003, research R6)", () => {
  test("sentinel planted in tool arguments appears nowhere in the record", async () => {
    const root = tmpWorkspace();
    const ws = workspaceAt(root);
    mkdirSync(ws.stateDir, { recursive: true });
    const tracePath = path.join(ws.stateDir, "trace.jsonl");
    const tracer = new JsonlTracer(tracePath);
    const server = buildServer(
      ws,
      coldFakeSearch(),
      undefined,
      undefined,
      undefined,
      undefined,
      tracer,
    );
    const client = await connectedClient(server);
    try {
      await client.callTool({ name: "search", arguments: { query: SENTINEL } });
      await client.callTool({ name: "search", arguments: { query: SENTINEL, mode: "references" } });
      await client.callTool({
        name: "search",
        arguments: { query: SENTINEL, path: `${SENTINEL}/**` },
      });
    } finally {
      await client.close();
    }
    await tracer.settle();

    const raw = readFileSync(tracePath, "utf8");
    expect(readTrace(tracePath).filter((r) => r.name.startsWith("tool:"))).toHaveLength(3);
    expect(raw.includes(SENTINEL)).toBe(false);
  });

  test("no network capability in the tracing modules (source sweep)", () => {
    // m5-gate style: the guarantee holds by absence, pinned against drift.
    const sources = [
      path.join(REPO, "src", "ports", "tracing.ts"),
      path.join(REPO, "src", "protocol", "tracing.ts"),
    ];
    const forbidden = [
      /\bfetch\s*\(/,
      /node:net/,
      /node:https?/,
      /Bun\.connect/,
      /WebSocket/,
      /XMLHttpRequest/,
    ];
    for (const source of sources) {
      const text = readFileSync(source, "utf8");
      for (const pattern of forbidden) {
        expect(pattern.test(text) ? `${path.basename(source)} matches ${pattern}` : "").toBe("");
      }
    }
  });
});

describe("composition: the real server (FR-005)", () => {
  test("default on: trace.jsonl written under .workbench; tracing.enabled=false: no file", async () => {
    // Default-on server: a fresh workspace with zero configuration.
    const onRoot = tmpWorkspace();
    const onTrace = path.join(onRoot, ".workbench", "trace.jsonl");
    {
      const client = new Client({ name: "tracing-e2e-on", version: "0.0.0" });
      const transport = new StdioClientTransport({
        command: process.execPath,
        args: [MAIN, onRoot],
        env: { ...process.env, WORKBENCH_NETWORK_FS: "0" } as Record<string, string>,
        cwd: REPO,
      });
      await client.connect(transport);
      try {
        await client.callTool({ name: "workbench_status", arguments: {} });
        await client.callTool({ name: "search", arguments: { query: SENTINEL } });
        // Fire-and-forget writes: poll while the server is still alive.
        const deadline = Date.now() + 10_000;
        while (Date.now() < deadline) {
          if (existsSync(onTrace) && readTrace(onTrace).length >= 2) break;
          await new Promise((r) => setTimeout(r, 100));
        }
      } finally {
        await client.close();
      }
      const records = readTrace(onTrace);
      expect(records.filter((r) => r.name === "tool:workbench_status").length).toBeGreaterThan(0);
      expect(records.filter((r) => r.name === "tool:search").length).toBeGreaterThan(0);
      expect(readFileSync(onTrace, "utf8").includes(SENTINEL)).toBe(false);
    }

    // Off switch at the session layer: no file, nothing else changes.
    const offRoot = tmpWorkspace();
    {
      const client = new Client({ name: "tracing-e2e-off", version: "0.0.0" });
      const transport = new StdioClientTransport({
        command: process.execPath,
        args: [MAIN, offRoot, "--config", "tracing.enabled=false"],
        env: { ...process.env, WORKBENCH_NETWORK_FS: "0" } as Record<string, string>,
        cwd: REPO,
      });
      await client.connect(transport);
      try {
        const result = await client.callTool({ name: "workbench_status", arguments: {} });
        const text = (result as { content: Array<{ text: string }> }).content[0]?.text ?? "";
        expect(JSON.parse(text).ok).toBe(true);
      } finally {
        await client.close();
      }
      expect(existsSync(path.join(offRoot, ".workbench", "trace.jsonl"))).toBe(false);
    }
  }, 60_000);
});
