/**
 * The M0 statelessness gate (SDD §5.1): two server processes on one
 * local-disk workspace, alternating requests, byte-identical envelopes.
 *
 * The processes alternate under full replacement — A answers the request set
 * and exits, B answers it, then a fresh A answers it again — which is the
 * strongest form of the claim: a server that carried ANY in-memory session
 * state (counters, caches keyed by session, warm-up effects) could not answer
 * byte-identically from a cold process every time. Full replacement also
 * keeps the advisory lock honest — it stays enforced in every spawned server,
 * and its refusal of a genuinely concurrent instance is asserted here too.
 */

import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const REPO = path.resolve(import.meta.dir, "..");
const MAIN = path.join(REPO, "src", "main.ts");
const LOCAL_ENV = { ...process.env, WORKBENCH_NETWORK_FS: "0" } as Record<string, string>;

/** The alternating request set. Raw text bytes are the comparison unit. */
async function answersFromFreshServer(root: string): Promise<readonly string[]> {
  const client = new Client({ name: "statelessness", version: "0.0.0" });
  await client.connect(
    new StdioClientTransport({
      command: process.execPath,
      args: [MAIN, root],
      env: LOCAL_ENV,
      cwd: REPO,
    }),
  );
  try {
    const texts: string[] = [];
    const calls: Array<Record<string, unknown> | undefined> = [
      undefined,
      { "workbench/maxEnvelopeVersion": 1 },
      { "workbench/maxEnvelopeVersion": "garbled" },
    ];
    for (const meta of calls) {
      const result = await client.callTool({
        name: "workbench_status",
        arguments: {},
        ...(meta === undefined ? {} : { _meta: meta }),
      });
      const content = result.content as Array<{ text: string }>;
      const first = content[0];
      if (first === undefined) throw new Error("empty content");
      texts.push(first.text);
    }
    return texts;
  } finally {
    await client.close();
  }
}

describe("statelessness", () => {
  test("two processes, alternating requests, byte-identical envelopes", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "workbench-stateless-"));
    try {
      const processA = await answersFromFreshServer(root);
      const processB = await answersFromFreshServer(root);
      const processA2 = await answersFromFreshServer(root);
      expect(processB).toEqual(processA);
      expect(processA2).toEqual(processA);
      // And the envelopes really are envelopes, not empty strings.
      for (const text of processA) expect(JSON.parse(text)).toHaveProperty("v", 1);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }, 60_000);

  test("a concurrent second instance is refused by the advisory lock (exit 75)", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "workbench-lock-"));
    const first = Bun.spawn({
      cmd: [process.execPath, MAIN, root],
      cwd: REPO,
      env: LOCAL_ENV,
      stdin: "pipe", // keep the stdio server alive
      stdout: "ignore",
      stderr: "ignore",
    });
    try {
      // Wait until the first server actually holds the lock, then challenge it.
      const lockPath = path.join(root, ".workbench", "server.lock");
      const deadline = Date.now() + 15_000;
      let holder: number | undefined;
      while (holder !== first.pid) {
        if (Date.now() > deadline) throw new Error("first server never took the lock");
        try {
          holder = (JSON.parse(readFileSync(lockPath, "utf8")) as { pid: number }).pid;
        } catch {
          await new Promise((r) => setTimeout(r, 50));
        }
      }
      const second = Bun.spawn({
        cmd: [process.execPath, MAIN, root],
        cwd: REPO,
        env: LOCAL_ENV,
        stdin: "pipe",
        stdout: "ignore",
        stderr: "pipe",
      });
      expect(await second.exited).toBe(75);
      const stderr = await new Response(second.stderr as ReadableStream).text();
      expect(stderr).toContain("already owns this workspace");
    } finally {
      first.kill();
      await first.exited;
      rmSync(root, { recursive: true, force: true });
    }
  }, 60_000);
});

/**
 * The M2 write-path request set (SDD §13 M2 gate: "statelessness re-run").
 * Every call runs the full §7.5 pipeline yet touches nothing: a dry-run
 * replace_text (parse-verify + format), a confirm-gated rewrite whose
 * CONFIRM_REQUIRED answer carries the HMAC token minted from the on-disk
 * workspace key, and a parse-breaking replace_text (PARSE_REJECTED). A
 * server holding ANY in-memory write state — pending confirms, cached
 * parses, session counters — could not answer these byte-identically from
 * a cold process every time.
 */
const WRITE_CALLS: ReadonlyArray<Record<string, unknown>> = [
  { op: "replace_text", file: "src/app.ts", old: "return 1", new: "return 2", dryRun: true },
  { op: "rewrite", pattern: "return 1", rewritePattern: "return 2", lang: "ts", path: "src/**" },
  { op: "replace_text", file: "src/app.ts", old: "return 1", new: "return ((" },
];

async function writeAnswersFromFreshServer(root: string): Promise<readonly string[]> {
  const client = new Client({ name: "statelessness-write", version: "0.0.0" });
  await client.connect(
    new StdioClientTransport({
      command: process.execPath,
      args: [MAIN, root],
      env: LOCAL_ENV,
      cwd: REPO,
    }),
  );
  try {
    const texts: string[] = [];
    for (const args of WRITE_CALLS) {
      const result = await client.callTool({ name: "edit", arguments: args });
      const content = result.content as Array<{ text: string }>;
      const first = content[0];
      if (first === undefined) throw new Error("empty content");
      texts.push(first.text);
    }
    return texts;
  } finally {
    await client.close();
  }
}

describe("statelessness against the write path (M2 gate)", () => {
  test("alternating write-path requests, byte-identical envelopes, nothing mutated", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "workbench-stateless-write-"));
    const app = "export function answer(): number {\n  return 1;\n}\n";
    try {
      mkdirSync(path.join(root, "src"), { recursive: true });
      writeFileSync(path.join(root, "src", "app.ts"), app);

      const processA = await writeAnswersFromFreshServer(root);
      const processB = await writeAnswersFromFreshServer(root);
      const processA2 = await writeAnswersFromFreshServer(root);
      expect(processB).toEqual(processA);
      expect(processA2).toEqual(processA);

      // The set exercised the pipeline for real, not vacuously.
      const [dryRunText, gatedText, rejectedText] = processA;
      if (dryRunText === undefined || gatedText === undefined || rejectedText === undefined) {
        throw new Error("write-path request set came back short");
      }
      const dryRun = JSON.parse(dryRunText) as {
        ok: boolean;
        results: Array<{ applied: boolean; diff: string }>;
      };
      expect(dryRun.ok).toBe(true);
      expect(dryRun.results[0]?.applied).toBe(false);
      expect(dryRun.results[0]?.diff).toContain("return 2");

      const gated = JSON.parse(gatedText) as {
        ok: boolean;
        error?: { code: string; confirmToken?: string };
      };
      expect(gated.ok).toBe(false);
      expect(gated.error?.code).toBe("CONFIRM_REQUIRED");
      expect(gated.error?.confirmToken?.startsWith("wb1.")).toBe(true);

      const rejected = JSON.parse(rejectedText) as { ok: boolean; error?: { code: string } };
      expect(rejected.ok).toBe(false);
      expect(rejected.error?.code).toBe("PARSE_REJECTED");

      // And the write path really touched nothing across three processes.
      expect(readFileSync(path.join(root, "src", "app.ts"), "utf8")).toBe(app);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }, 120_000);
});
