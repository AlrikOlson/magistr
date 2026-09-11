/** Real stdio server regression: the former synchronous diff hang must not
 * block a follow-up request, and restart/undo must restore the original bytes. */
import { expect, test } from "bun:test";
import { randomBytes } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { contentHash, type ResponseEnvelope } from "../src/envelope";

test("large binary copy responds promptly and durable undo works over MCP", async () => {
  const root = mkdtempSync(join(tmpdir(), "workbench-binary-mcp-"));
  // Optional real-world corpus for a measured reproduction, never committed.
  const source = process.env.WORKBENCH_BINARY_SOURCE
    ? readFileSync(process.env.WORKBENCH_BINARY_SOURCE)
    : randomBytes(6 * 1024 * 1024);
  const original = process.env.WORKBENCH_BINARY_TARGET
    ? readFileSync(process.env.WORKBENCH_BINARY_TARGET)
    : randomBytes(6 * 1024 * 1024);
  writeFileSync(join(root, "source.png"), source);
  writeFileSync(join(root, "target.png"), original);
  const sourceHash = contentHash(source);
  const originalHash = contentHash(original);
  const timings: Record<string, number> = {};
  const connect = async () => {
    const client = new Client({ name: "binary-regression", version: "1" });
    await client.connect(
      new StdioClientTransport({
        command: process.execPath,
        args: [resolve(import.meta.dir, "../src/main.ts"), root],
        env: { ...process.env, WORKBENCH_NETWORK_FS: "0" } as Record<string, string>,
        stderr: "ignore",
      }),
    );
    return client;
  };
  const call = async (
    client: Client,
    name: string,
    args: Record<string, unknown>,
    label: string,
  ) => {
    const start = performance.now();
    const result = await client.callTool({ name, arguments: args }, undefined, { timeout: 5000 });
    timings[label] = Math.round(performance.now() - start);
    const block = result.content as Array<{ type: string; text: string }>;
    const envelope = JSON.parse(block[0]?.text ?? "null") as ResponseEnvelope;
    expect(envelope.ok).toBe(true);
    expect(JSON.stringify(envelope).length).toBeLessThan(2500);
    return envelope;
  };
  let client: Client | undefined;
  try {
    client = await connect();
    await call(
      client,
      "files",
      { op: "copy", from: "source.png", to: "target.png", dryRun: true },
      "previewMs",
    );
    expect(contentHash(readFileSync(join(root, "target.png")))).toBe(originalHash);
    await call(
      client,
      "files",
      { op: "copy", from: "source.png", to: "target.png", autoApply: true },
      "copyMs",
    );
    expect(contentHash(readFileSync(join(root, "target.png")))).toBe(sourceHash);
    await call(client, "workbench_status", {}, "responseAfterCopyMs");
    await client.close();
    client = await connect();
    await call(client, "undo", { scope: "workspace" }, "undoAfterRestartMs");
    expect(contentHash(readFileSync(join(root, "target.png")))).toBe(originalHash);
    await call(client, "workbench_status", {}, "responseAfterUndoMs");
    if (process.env.WORKBENCH_BINARY_REPORT) {
      writeFileSync(
        process.env.WORKBENCH_BINARY_REPORT,
        `${JSON.stringify(
          {
            sourceBytes: source.byteLength,
            targetBytes: original.byteLength,
            sourceHash,
            originalHash,
            restoredByteExact: true,
            ...timings,
          },
          null,
          2,
        )}\n`,
      );
    }
  } finally {
    await client?.close();
    rmSync(root, { recursive: true, force: true });
  }
}, 20000);
