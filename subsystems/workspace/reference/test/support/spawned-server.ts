/**
 * A spawned `bun src/main.ts <root>` server driven through the official MCP
 * SDK client, with the PROCESS facts kept visible: pid, exit code, and every
 * byte of stderr. The SDK's own StdioClientTransport hides the exit code and
 * turns a server that dies before `initialize` into a bare "Connection
 * closed" — useless for the multi-client acceptance rows (specs/018 rows 1,
 * 2, 8, 9, 11), whose whole point is WHICH process exited with WHAT code and
 * what it said on the way out.
 *
 * `connect()` rejects, rather than hangs, when the server exits first: the
 * transport fires `onclose` on process exit and the SDK's Protocol rejects
 * every pending request on close.
 */

import path from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { ReadBuffer, serializeMessage } from "@modelcontextprotocol/sdk/shared/stdio.js";
import type { Transport } from "@modelcontextprotocol/sdk/shared/transport.js";
import type { JSONRPCMessage } from "@modelcontextprotocol/sdk/types.js";

export const REPO = path.resolve(import.meta.dir, "..", "..");
export const MAIN = path.join(REPO, "src", "main.ts");
/** Local-forced env so a network-mounted CI tmpdir cannot skew a suite. */
export const LOCAL_ENV = { ...process.env, WORKBENCH_NETWORK_FS: "0" } as Record<string, string>;

type ServerProcess = ReturnType<typeof spawnProcess>;

function spawnProcess(argv: readonly string[], env: Record<string, string>) {
  return Bun.spawn([...argv], {
    cwd: REPO,
    env,
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe",
  });
}

/** Drain a byte stream into `sink`, one chunk at a time, until it ends. */
async function drain(stream: ReadableStream<Uint8Array>, sink: (chunk: Buffer) => void) {
  const reader = stream.getReader();
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    sink(Buffer.from(value));
  }
}

class SpawnedStdioTransport implements Transport {
  onclose?: () => void;
  onerror?: (error: Error) => void;
  onmessage?: (message: JSONRPCMessage) => void;

  private readonly readBuffer = new ReadBuffer();
  private closed = false;

  constructor(private readonly process: ServerProcess) {}

  async start(): Promise<void> {
    void drain(this.process.stdout, (chunk) => {
      try {
        this.readBuffer.append(chunk);
        for (;;) {
          const message = this.readBuffer.readMessage();
          if (message === null) break;
          this.onmessage?.(message);
        }
      } catch (error) {
        this.onerror?.(error instanceof Error ? error : new Error(String(error)));
      }
    });
    void this.process.exited.then(() => this.fireClose());
  }

  async send(message: JSONRPCMessage): Promise<void> {
    this.process.stdin.write(serializeMessage(message));
    await this.process.stdin.flush();
  }

  async close(): Promise<void> {
    try {
      await this.process.stdin.end();
    } catch {
      // Already closed by the server's exit.
    }
    this.fireClose();
  }

  private fireClose(): void {
    if (this.closed) return;
    this.closed = true;
    this.onclose?.();
  }
}

export interface SpawnedServer {
  readonly client: Client;
  readonly pid: number;
  /** Resolves with the exit code once the server process has exited. */
  readonly exited: Promise<number>;
  /** Everything the server has written to stderr so far. */
  stderr(): string;
  /** The exit code if the process exits within `ms`, else undefined (still running). */
  exitedWithin(ms: number): Promise<number | undefined>;
  /**
   * initialize + initialized through the SDK client. Rejects when the server
   * exits before answering — inspect `exitedWithin` and `stderr()` for why.
   */
  connect(): Promise<void>;
  /** Close the client (stdin EOF), then wait for the server to exit. */
  close(): Promise<void>;
}

export interface SpawnServerOptions {
  readonly clientName?: string;
  readonly env?: Record<string, string>;
  readonly extraArgs?: readonly string[];
}

/** Spawn one `workbench-mcp <root>` stdio server and wrap it for a test. */
export function spawnServer(root: string, options: SpawnServerOptions = {}): SpawnedServer {
  const proc = spawnProcess([process.execPath, MAIN, root, ...(options.extraArgs ?? [])], {
    ...LOCAL_ENV,
    ...options.env,
  });
  let stderrText = "";
  void drain(proc.stderr, (chunk) => {
    stderrText += chunk.toString("utf8");
  });
  const exited = proc.exited.then((code) => code ?? -1);
  const transport = new SpawnedStdioTransport(proc);
  const client = new Client({ name: options.clientName ?? "spawned-server", version: "0.0.0" });

  return {
    client,
    pid: proc.pid,
    exited,
    stderr: () => stderrText,
    exitedWithin: (ms) =>
      Promise.race([
        exited,
        new Promise<undefined>((resolve) => setTimeout(() => resolve(undefined), ms)),
      ]),
    connect: () => client.connect(transport),
    close: async () => {
      await client.close().catch(() => undefined);
      const code = await Promise.race([
        exited,
        new Promise<undefined>((resolve) => setTimeout(() => resolve(undefined), 5_000)),
      ]);
      if (code === undefined) {
        proc.kill();
        await exited;
      }
    },
  };
}
