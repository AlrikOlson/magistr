/**
 * Scripted fake language server (specs/001-lsp-resolver-plugin research D7;
 * extended for specs/007-resolver-server-pool research R8).
 *
 * Speaks just enough LSP over Content-Length-framed stdio for the resolver
 * plugin's flow: initialize → workspace/symbol → textDocument/references.
 * Spawned by the Rust subprocess capability through a test-seam spec whose
 * bin is the bun executable — the same lane a real detected server uses.
 *
 * Modes (argv[2..], default "normal"; several may be combined, e.g.
 * `slow-init:2500 count:/tmp/inits.txt`):
 *   normal            — answers everything; symbol lives at src/greet.ts:1:17
 *                       with a second reference at src/use.ts:3:1 (LSP wire
 *                       is 0-based).
 *   crash             — exits(1) on workspace/symbol, mid-conversation.
 *   hang              — accepts requests and never answers references.
 *   slow-init:<ms>    — delays the `initialize` reply by <ms> (a server whose
 *                       handshake is slower than the host's per-request
 *                       budget).
 *   count:<file>      — appends one line per `initialize` received to <file>
 *                       and reports the running count as `initializeCount`
 *                       on every workspace/symbol hit (the pool's
 *                       "handshake exactly once" pin).
 *   pid:<file>        — writes "<pid> <ppid>" to <file> at startup (the
 *                       kill-on-child-death pins; ppid is the engine child).
 *   crash-at:<k>      — exits(1) on the k-th workspace/symbol of this
 *                       process (a warm server dying between requests).
 *   ignore-eof        — keeps running after stdin closes, so only an OS-level
 *                       kill (job object / parent-death signal) can end it.
 */

import { appendFileSync, writeFileSync } from "node:fs";

const modes = process.argv.slice(2);
const has = (m: string): boolean => modes.includes(m);
const arg = (prefix: string): string | undefined =>
  modes.find((m) => m.startsWith(`${prefix}:`))?.slice(prefix.length + 1);

const slowInitMs = Number(arg("slow-init") ?? "0");
const countFile = arg("count");
const pidFile = arg("pid");
const crashAt = Number(arg("crash-at") ?? "0");
let rootUri = "";
let initializeCount = 0;
let symbolRequests = 0;

if (pidFile !== undefined) writeFileSync(pidFile, `${process.pid} ${process.ppid}\n`);

function frame(body: unknown): Uint8Array {
  const json = JSON.stringify(body);
  const bytes = new TextEncoder().encode(json);
  const header = new TextEncoder().encode(`Content-Length: ${bytes.length}\r\n\r\n`);
  const out = new Uint8Array(header.length + bytes.length);
  out.set(header, 0);
  out.set(bytes, header.length);
  return out;
}

function respond(id: unknown, result: unknown): void {
  process.stdout.write(frame({ jsonrpc: "2.0", id, result }));
}

function handle(message: Record<string, unknown>): void {
  const method = message["method"];
  const id = message["id"];
  if (method === "initialize") {
    initializeCount += 1;
    if (countFile !== undefined) appendFileSync(countFile, `initialize ${initializeCount}\n`);
    const params = message["params"] as Record<string, unknown> | undefined;
    rootUri = typeof params?.["rootUri"] === "string" ? (params["rootUri"] as string) : "";
    const reply = () => respond(id, { capabilities: {} });
    if (slowInitMs > 0) setTimeout(reply, slowInitMs);
    else reply();
    return;
  }
  if (method === "initialized") return;
  if (method === "workspace/symbol") {
    symbolRequests += 1;
    if (has("crash") || (crashAt > 0 && symbolRequests === crashAt)) process.exit(1);
    const params = message["params"] as Record<string, unknown> | undefined;
    const query = typeof params?.["query"] === "string" ? (params["query"] as string) : "";
    respond(id, [
      {
        name: query,
        kind: 12,
        ...(countFile !== undefined ? { initializeCount } : {}),
        location: {
          uri: `${rootUri}/src/greet.ts`,
          range: { start: { line: 0, character: 16 }, end: { line: 0, character: 21 } },
        },
      },
    ]);
    return;
  }
  if (method === "textDocument/references") {
    if (has("hang")) return; // never answer — the guest's read must time out
    respond(id, [
      {
        uri: `${rootUri}/src/greet.ts`,
        range: { start: { line: 0, character: 16 }, end: { line: 0, character: 21 } },
      },
      {
        uri: `${rootUri}/src/use.ts`,
        range: { start: { line: 2, character: 0 }, end: { line: 2, character: 5 } },
      },
    ]);
    return;
  }
  if (id !== undefined) respond(id, null);
}

let buffer = new Uint8Array(0);
process.stdin.on("data", (chunk: Uint8Array) => {
  const merged = new Uint8Array(buffer.length + chunk.length);
  merged.set(buffer, 0);
  merged.set(chunk, buffer.length);
  buffer = merged;
  for (;;) {
    const text = new TextDecoder().decode(buffer);
    const headerEnd = text.indexOf("\r\n\r\n");
    if (headerEnd < 0) return;
    const match = /content-length:\s*(\d+)/i.exec(text.slice(0, headerEnd));
    if (match?.[1] === undefined) process.exit(2);
    const bodyLength = Number(match[1]);
    const bodyStart = new TextEncoder().encode(text.slice(0, headerEnd + 4)).length;
    if (buffer.length < bodyStart + bodyLength) return;
    const body = new TextDecoder().decode(buffer.slice(bodyStart, bodyStart + bodyLength));
    buffer = buffer.slice(bodyStart + bodyLength);
    handle(JSON.parse(body) as Record<string, unknown>);
  }
});
process.stdin.on("end", () => {
  if (has("ignore-eof")) {
    // Outlive our stdin on purpose: only the OS kill group may end us now.
    setInterval(() => {}, 60_000);
    return;
  }
  process.exit(0);
});
