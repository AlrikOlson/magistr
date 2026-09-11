/**
 * The MRTR confirm flow (SDD §5.1, §9.4, risk row 2) — chunk:m2-mrtr-confirm
 * acceptance:
 *
 *   1. the MRTR flow and the token-fallback flow both pass, as equal paths;
 *   2. a stale confirm fails closed with STALE_ANCHOR; a forged or
 *      mismatched token fails with CONFIRM_MISMATCH;
 *   3. confirm tokens verify against the per-workspace key and survive a
 *      server restart — no in-memory state anywhere.
 *
 * The token half runs against runEdit with a ConfirmPort over a real on-disk
 * workspace key; the MRTR half runs a real SDK Client against buildServer
 * over InMemoryTransport, with the client answering elicitation/create.
 */

import { describe, expect, test } from "bun:test";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { ElicitRequestSchema } from "@modelcontextprotocol/sdk/types.js";
import { createInProcessPorts, type InProcessPorts } from "../src/adapters/inprocess";
import { defaultResolvedConfig } from "../src/config";
import type { ResponseEnvelope } from "../src/envelope";
import {
  checkConfirmGrant,
  ensureWorkspaceKey,
  KEY_LENGTH,
  mintConfirmGrant,
  rotateWorkspaceKey,
} from "../src/jail";
import { buildServer, type WorkspaceRuntime } from "../src/protocol";
import type { SearchDeps, WriteEngineDeps } from "../src/tools";
import { type ConfirmPort, type EditDeps, type EditInput, runEdit } from "../src/tools/edit";
import type { WriteReport } from "../src/tools/write-engine";

const WB1_TOKEN = /^wb1\.[0-9a-f]{64}\.[0-9a-f]{64}\.[0-9a-f]{64}$/;

/** Two files sharing the pattern, so the rewrite is inherently multi-file. */
const SEED: Record<string, string> = {
  "src/a.ts": "function a(): number {\n  return 1;\n}\n",
  "src/b.ts": "function b(): number {\n  return 1;\n}\n",
};

const REWRITE: EditInput = {
  op: "rewrite",
  pattern: "return 1;",
  rewritePattern: "return 7;",
  path: "src/**",
};

function tmpStateDir(): string {
  return join(mkdtempSync(join(tmpdir(), "wb-mrtr-")), ".workbench");
}

/** The composition root's port shape: the key is re-read from disk per call. */
function confirmPortFor(stateDir: string): ConfirmPort {
  return {
    mint: (diffHash, contentHash) =>
      mintConfirmGrant(ensureWorkspaceKey(stateDir), diffHash, contentHash),
    check: (token) => checkConfirmGrant(ensureWorkspaceKey(stateDir), token),
  };
}

function enoent(path: string): NodeJS.ErrnoException {
  const error = new Error(`ENOENT: no such file: ${path}`) as NodeJS.ErrnoException;
  error.code = "ENOENT";
  return error;
}

interface Rig {
  readonly deps: EditDeps;
  readonly ports: InProcessPorts;
  readonly files: Map<string, string>;
  readonly stateDir: string;
  readonly write: WriteEngineDeps;
}

function makeRig(stateDir = tmpStateDir(), seed: Record<string, string> = SEED): Rig {
  const ports = createInProcessPorts();
  const files = ports.workspace.files;
  for (const [path, content] of Object.entries(seed)) files.set(path, content);
  const write: WriteEngineDeps = {
    engine: ports.engine,
    formatter: ports.formatter,
    journal: ports.journal,
    workspace: {
      read: (path) =>
        files.has(path) ? Promise.resolve(files.get(path) as string) : Promise.reject(enoent(path)),
      write: (path, content) => {
        files.set(path, content);
        return Promise.resolve();
      },
      remove: (path) => {
        files.delete(path);
        return Promise.resolve();
      },
    },
  };
  const deps: EditDeps = {
    write,
    index: ports.index,
    confirm: confirmPortFor(stateDir),
    list: () => Promise.resolve([...files.keys()].map((path) => ({ path, junk: false }))),
  };
  return { deps, ports, files, stateDir, write };
}

function refused(envelope: ResponseEnvelope<WriteReport>) {
  if (envelope.ok) throw new Error("expected an error envelope, got ok");
  return envelope.error as unknown as { code: string; message: string } & Record<string, unknown>;
}

/** Run the gated call, harvest the CONFIRM_REQUIRED token. */
async function mintViaGate(rig: Rig, input: EditInput = REWRITE): Promise<string> {
  const envelope = await runEdit(rig.deps, input, 1);
  const error = refused(envelope);
  expect(error.code).toBe("CONFIRM_REQUIRED");
  expect(error["confirmToken"]).toMatch(WB1_TOKEN);
  return error["confirmToken"] as string;
}

describe("the token-fallback flow (first-class equal path)", () => {
  test("gated rewrite answers CONFIRM_REQUIRED with a wb1 token and the aggregated diff; the identical call with confirm applies", async () => {
    const rig = makeRig();
    const envelope = await runEdit(rig.deps, REWRITE, 1);
    const error = refused(envelope);
    expect(error.code).toBe("CONFIRM_REQUIRED");
    const token = error["confirmToken"] as string;
    expect(token).toMatch(WB1_TOKEN);
    expect((envelope.notes ?? []).join("\n")).toContain("return 7;");
    // Nothing was applied by the refusal.
    expect(rig.files.get("src/a.ts")).toContain("return 1;");

    const applied = await runEdit(rig.deps, { ...REWRITE, confirm: token }, 1);
    expect(applied.ok).toBe(true);
    expect(rig.files.get("src/a.ts")).toContain("return 7;");
    expect(rig.files.get("src/b.ts")).toContain("return 7;");
  });

  test("tokens survive a restart: minted by one composition, applied by a fresh one over the same disk key", async () => {
    const rig = makeRig();
    const token = await mintViaGate(rig);
    // A "restarted server": a brand-new ConfirmPort instance whose only
    // shared state with the minting one is the key file on disk.
    const restarted: EditDeps = { ...rig.deps, confirm: confirmPortFor(rig.stateDir) };
    const applied = await runEdit(restarted, { ...REWRITE, confirm: token }, 1);
    expect(applied.ok).toBe(true);
    expect(rig.files.get("src/b.ts")).toContain("return 7;");
  });
});

describe("stale and forged confirms fail closed", () => {
  test("a file moved under the confirmed diff: STALE_ANCHOR with the fresh aggregate hash, nothing applied", async () => {
    const rig = makeRig();
    const token = await mintViaGate(rig);
    const moved = "function a(): number {\n  return 1;\n}\n// moved\n";
    rig.files.set("src/a.ts", moved);
    const envelope = await runEdit(rig.deps, { ...REWRITE, confirm: token }, 1);
    const error = refused(envelope);
    expect(error.code).toBe("STALE_ANCHOR");
    expect(typeof error["contentHash"]).toBe("string");
    expect(rig.files.get("src/a.ts")).toBe(moved);
    expect(rig.files.get("src/b.ts")).toContain("return 1;");
  });

  test("matches that evaporated answer STALE_ANCHOR, not nothing-to-change", async () => {
    const rig = makeRig();
    const token = await mintViaGate(rig);
    rig.files.set("src/a.ts", "function a(): number {\n  return 2;\n}\n");
    rig.files.set("src/b.ts", "function b(): number {\n  return 2;\n}\n");
    const envelope = await runEdit(rig.deps, { ...REWRITE, confirm: token }, 1);
    expect(refused(envelope).code).toBe("STALE_ANCHOR");
  });

  test("a tampered, garbage, or foreign-key token answers CONFIRM_MISMATCH and applies nothing", async () => {
    const rig = makeRig();
    const token = await mintViaGate(rig);
    const flipped = token.slice(0, -1) + (token.endsWith("0") ? "1" : "0");
    for (const bad of [
      flipped,
      "not-a-token",
      "wb1.deadbeef.deadbeef.deadbeef",
      mintConfirmGrant(Buffer.alloc(KEY_LENGTH, 9), "d".repeat(64), "c".repeat(64)),
    ]) {
      const envelope = await runEdit(rig.deps, { ...REWRITE, confirm: bad }, 1);
      expect(refused(envelope).code).toBe("CONFIRM_MISMATCH");
    }
    expect(rig.files.get("src/a.ts")).toContain("return 1;");
  });

  test("an authentic token for different arguments answers CONFIRM_MISMATCH", async () => {
    const rig = makeRig();
    const token = await mintViaGate(rig);
    const envelope = await runEdit(
      rig.deps,
      { ...REWRITE, rewritePattern: "return 8;", confirm: token },
      1,
    );
    expect(refused(envelope).code).toBe("CONFIRM_MISMATCH");
    expect(rig.files.get("src/a.ts")).toContain("return 1;");
  });

  test("key rotation voids every outstanding token", async () => {
    const rig = makeRig();
    const token = await mintViaGate(rig);
    rotateWorkspaceKey(rig.stateDir);
    const envelope = await runEdit(rig.deps, { ...REWRITE, confirm: token }, 1);
    expect(refused(envelope).code).toBe("CONFIRM_MISMATCH");
  });
});

/** Server-level rig: a real Client over InMemoryTransport. */
async function connectedEditClient(options: {
  readonly elicitation: boolean;
  readonly onElicit?: (message: string) => { action: "accept" | "decline"; apply?: boolean };
}): Promise<{ client: Client; rig: Rig }> {
  const rig = makeRig();
  const workspace: WorkspaceRuntime = {
    root: join(rig.stateDir, ".."),
    stateDir: rig.stateDir,
    detection: { networkFs: false, method: "default-local" },
    journalMode: "WAL",
    appliedJournalMode: "wal",
    config: defaultResolvedConfig(),
    notes: [],
    close: () => {},
  };
  const search: SearchDeps = {
    engine: rig.ports.engine,
    index: rig.ports.index,
    workspace: {
      list: () => Promise.resolve([...rig.files.keys()].map((path) => ({ path, junk: false }))),
      read: (path) =>
        rig.files.has(path)
          ? Promise.resolve(rig.files.get(path) as string)
          : Promise.reject(enoent(path)),
    },
  };
  const server = buildServer(workspace, search, rig.write);
  const client = new Client(
    { name: "mrtr-test", version: "0.0.0" },
    options.elicitation ? { capabilities: { elicitation: {} } } : {},
  );
  if (options.elicitation) {
    client.setRequestHandler(ElicitRequestSchema, (request) => {
      const decision = (options.onElicit ?? (() => ({ action: "accept" as const, apply: true })))(
        request.params.message,
      );
      return decision.action === "accept"
        ? { action: "accept", content: { apply: decision.apply === true } }
        : { action: "decline" };
    });
  }
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  await server.connect(serverTransport);
  await client.connect(clientTransport);
  return { client, rig };
}

function envelopeOf(result: unknown): ResponseEnvelope<WriteReport> {
  return (result as { structuredContent: ResponseEnvelope<WriteReport> }).structuredContent;
}

describe("the MRTR flow over the server (elicited confirm inside one call)", () => {
  test("an accept applies the gated rewrite in one tools/call, and the elicitation carried the diff", async () => {
    const seen: string[] = [];
    const { client, rig } = await connectedEditClient({
      elicitation: true,
      onElicit: (message) => {
        seen.push(message);
        return { action: "accept", apply: true };
      },
    });
    try {
      const result = await client.callTool({ name: "edit", arguments: { ...REWRITE } });
      const envelope = envelopeOf(result);
      expect(envelope.ok).toBe(true);
      expect(rig.files.get("src/a.ts")).toContain("return 7;");
      expect(rig.files.get("src/b.ts")).toContain("return 7;");
      expect(seen).toHaveLength(1);
      expect(seen[0]).toContain("return 7;");
    } finally {
      await client.close();
    }
  });

  test("a decline falls back to the token path: CONFIRM_REQUIRED whose token then applies over the wire", async () => {
    const { client, rig } = await connectedEditClient({
      elicitation: true,
      onElicit: () => ({ action: "decline" }),
    });
    try {
      const first = envelopeOf(await client.callTool({ name: "edit", arguments: { ...REWRITE } }));
      const error = refused(first);
      expect(error.code).toBe("CONFIRM_REQUIRED");
      const token = error["confirmToken"] as string;
      expect(token).toMatch(WB1_TOKEN);
      expect(rig.files.get("src/a.ts")).toContain("return 1;");

      const second = envelopeOf(
        await client.callTool({ name: "edit", arguments: { ...REWRITE, confirm: token } }),
      );
      expect(second.ok).toBe(true);
      expect(rig.files.get("src/a.ts")).toContain("return 7;");
    } finally {
      await client.close();
    }
  });

  test("a client without the elicitation capability gets the token fallback with no elicitation attempted", async () => {
    const { client, rig } = await connectedEditClient({ elicitation: false });
    try {
      const first = envelopeOf(await client.callTool({ name: "edit", arguments: { ...REWRITE } }));
      const error = refused(first);
      expect(error.code).toBe("CONFIRM_REQUIRED");
      expect(error["confirmToken"]).toMatch(WB1_TOKEN);
      expect(rig.files.get("src/a.ts")).toContain("return 1;");
    } finally {
      await client.close();
    }
  });
});
