/**
 * Transport wiring (SDD §5.2): stdio for the local profile, single-replica
 * Streamable HTTP for sandboxes and remote use.
 *
 * stdout belongs to the stdio transport — every diagnostic goes to stderr in
 * both modes. HTTP runs the SDK's stateless profile: no session ids, and a
 * fresh server + transport pair per request, so two replicas answering
 * alternately are indistinguishable (the statelessness gate measures exactly
 * that). Single-replica-per-workspace is enforced upstream by the advisory
 * lock in `.workbench/`, taken before either transport starts.
 */

import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { WebStandardStreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/webStandardStreamableHttp.js";
import type { Tracer } from "../ports/tracing";
import type { SearchDeps, WriteEngineDeps } from "../tools";
import { buildServer, type PluginsRuntime, type RunRuntime, type ScriptRuntime } from "./server";
import type { WorkspaceRuntime } from "./workspace";

/**
 * Serve MCP over stdio until the transport closes.
 *
 * The returned promise settles when the client goes away — stdin reaching
 * EOF closes the transport. Without that wait `connect` resolves as soon as
 * the handshake is wired and the caller cannot tell a live client from a
 * departed one, so an orphaned server keeps holding the per-workspace lock
 * and the NEXT session is refused with exit 75. The caller releases the
 * workspace when this resolves.
 */
export async function serveStdio(
  workspace: WorkspaceRuntime,
  search: SearchDeps,
  write?: WriteEngineDeps,
  script?: ScriptRuntime,
  plugins?: PluginsRuntime,
  run?: RunRuntime,
  tracer?: Tracer,
): Promise<void> {
  const server = buildServer(workspace, search, write, script, plugins, run, tracer);
  const transport = new StdioServerTransport();
  const closed = new Promise<void>((resolve) => {
    const prior = transport.onclose;
    transport.onclose = () => {
      prior?.();
      resolve();
    };
  });
  // The SDK's stdio transport subscribes to stdin's "data" and "error" only
  // (sdk 1.30.0, server/stdio.js): EOF is invisible to it and `onclose` never
  // fires when the client goes away. Notice it here — one close is enough,
  // and `close()` is idempotent, so a later signal path is harmless.
  const onStdinEof = (): void => {
    void transport.close().catch(() => {});
  };
  process.stdin.once("end", onStdinEof);
  process.stdin.once("close", onStdinEof);
  await server.connect(transport);
  await closed;
}

export interface HttpHandle {
  readonly port: number;
  stop(): void;
}

/** The single MCP endpoint path for the HTTP profile. */
export const MCP_HTTP_PATH = "/mcp";

/**
 * Serve MCP over stateless Streamable HTTP. `port` 0 lets the OS choose;
 * the caller reads the real port from the handle.
 */
export function serveHttp(
  workspace: WorkspaceRuntime,
  search: SearchDeps,
  port: number,
  write?: WriteEngineDeps,
  script?: ScriptRuntime,
  plugins?: PluginsRuntime,
  run?: RunRuntime,
  tracer?: Tracer,
): HttpHandle {
  const bunServer = Bun.serve({
    port,
    async fetch(request) {
      const url = new URL(request.url);
      if (url.pathname !== MCP_HTTP_PATH) {
        return new Response("not found", { status: 404 });
      }
      // Stateless profile: one server + transport per request, no session ids
      // (sessionIdGenerator deliberately omitted; exactOptionalPropertyTypes
      // forbids passing an explicit undefined).
      const transport = new WebStandardStreamableHTTPServerTransport({
        enableJsonResponse: true,
      });
      const server = buildServer(workspace, search, write, script, plugins, run, tracer);
      await server.connect(transport);
      return transport.handleRequest(request);
    },
  });
  return {
    port: bunServer.port ?? port,
    stop() {
      bunServer.stop(true);
    },
  };
}
