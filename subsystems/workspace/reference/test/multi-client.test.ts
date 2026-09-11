/**
 * Multi-client access to one workspace (specs/018-multi-client-daemon, ADR-018).
 *
 * This file is the acceptance matrix's home for the rows that need two or
 * more independent `workbench-mcp <root>` clients on ONE disposable root. It
 * is RED on purpose until chunk bridge-stdio-relay lands: today the second
 * process exits 75 ("another workbench-mcp instance already owns this
 * workspace") with zero bytes on stdout, so its client never completes
 * `initialize`. That is the measured bug the daemon work exists to fix, and
 * the failure below carries the second server's pid, exit code and stderr so
 * the red run IS the reproduction.
 *
 * Because `bun test` is a required gate, the chapter is opt-in:
 *
 *     WORKBENCH_MULTI_CLIENT=1 bun test test/multi-client.test.ts
 *
 * Without the flag the chapter is skipped with a printed reason. Later rows
 * (startup race, cross-session undo, daemon crash, idle shutdown, …) extend
 * this file as their chunks land; row 1's "doctor shows one daemon, two
 * connections" observable is asserted once a daemon exists to show.
 */

import { afterAll, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { type SpawnedServer, spawnServer } from "./support/spawned-server";

const ENABLED = process.env["WORKBENCH_MULTI_CLIENT"] === "1";
const SKIP_REASON =
  "multi-client chapter skipped: set WORKBENCH_MULTI_CLIENT=1 — red until chunk bridge-stdio-relay (specs/018 row 1: the second stdio server exits 75 today)";

/** The nine v1 tools every connected client must see (SDD §9). */
const TOOLS = [
  "workbench_status",
  "search",
  "map",
  "read",
  "edit",
  "files",
  "run",
  "script",
  "undo",
] as const;

const roots: string[] = [];
function tmpWorkspace(): string {
  const root = mkdtempSync(path.join(tmpdir(), "workbench-multi-"));
  mkdirSync(path.join(root, "src"), { recursive: true });
  writeFileSync(path.join(root, "src", "a.ts"), "export const a = 1;\n");
  writeFileSync(path.join(root, "package.json"), JSON.stringify({ name: "x", version: "0.0.0" }));
  roots.push(root);
  return root;
}

afterAll(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

/**
 * Connect, or explain exactly which process refused and how. A server that
 * dies before answering `initialize` is the row-1 failure mode; its exit
 * code and stderr are the evidence, so they go into the thrown error.
 */
async function connectOrExplain(label: string, server: SpawnedServer): Promise<void> {
  const failure = await server.connect().then(
    () => undefined,
    (error: unknown) => error,
  );
  if (failure === undefined) return;
  const code = await server.exitedWithin(5_000);
  throw new Error(
    `${label} (server pid ${server.pid}) never completed initialize: ${String(failure)}; ` +
      `server ${code === undefined ? "still running" : `exited ${code}`}; ` +
      `stderr: ${JSON.stringify(server.stderr().trim())}`,
  );
}

async function toolNames(server: SpawnedServer): Promise<string[]> {
  const listed = await server.client.listTools();
  return listed.tools.map((tool) => tool.name).sort();
}

describe("multi-client workspace access (specs/018)", () => {
  test.skipIf(ENABLED)(SKIP_REASON, () => {
    // bun test does not echo passing test names, so say why out loud.
    console.warn(SKIP_REASON);
    expect(ENABLED).toBe(false);
  });

  test.skipIf(!ENABLED)(
    "row 1: two stdio clients on one root both complete initialize and list the nine tools",
    async () => {
      const root = tmpWorkspace();
      const a = spawnServer(root, { clientName: "client-A" });
      let b: SpawnedServer | undefined;
      try {
        await connectOrExplain("client A", a);
        expect(await toolNames(a)).toEqual([...TOOLS].sort());

        // The second client arrives while A is live. Today this is where the
        // run goes red: B's server exits 75 and the error names it.
        b = spawnServer(root, { clientName: "client-B" });
        await connectOrExplain("client B", b);
        expect(await toolNames(b)).toEqual([...TOOLS].sort());

        // Both stay live: A is not displaced by B's arrival.
        const status = await a.client.callTool({ name: "workbench_status", arguments: {} });
        const [first] = status.content as Array<{ text: string }>;
        expect(JSON.parse(first?.text ?? "{}")).toHaveProperty("ok", true);
      } finally {
        await b?.close();
        await a.close();
      }
    },
    60_000,
  );
});
