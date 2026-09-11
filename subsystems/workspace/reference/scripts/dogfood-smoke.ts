/**
 * The self-hosting smoke loop (chunk:dogfood-read-path): drive the workbench
 * read path against the workbench source tree.
 *
 * The point is that this repository is a *real* workspace — 1.5 GB of Rust
 * build output and 249 MB of node_modules behind a root .gitignore, a mixed
 * TypeScript/Rust corpus, a Tier-1 build racing the first calls — and none of
 * that is what the fixture-sized tmpdirs in `test/` exercise.
 *
 * The argv is not written here. It is READ OUT of this repo's own `.mcp.json`
 * entry, so the thing under test is exactly the thing an agent host launches:
 * a config that drifts from the driver cannot pass. The client is the official
 * MCP SDK over stdio, the same transport `test/mcp-conformance.test.ts` uses.
 *
 * What it exercises, all against this repo: tools/list, the Tasks extension
 * (tasks/list plus tasks/get polling the first-start `index/build` to a
 * terminal status), search in lexical / structural / references / files modes,
 * map, and read by explicit lines and by symbol address.
 *
 * Every envelope is checked against the normative `schemas/envelope.schema.json`
 * and must carry a freshness tier. Findings are reported, not asserted away: a
 * call that legitimately degrades (INDEX_UNAVAILABLE while the index warms, a
 * typed refusal) prints as a NOTE and does not fail the run, while a
 * schema-invalid envelope, a missing freshness tier, a hung build or a
 * transport failure exits non-zero.
 *
 * Usage: bun run scripts/dogfood-smoke.ts [--json]
 */

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { GetTaskResultSchema, ListTasksResultSchema } from "@modelcontextprotocol/sdk/types.js";
import envelopeSchema from "../schemas/envelope.schema.json";
import type { ResponseEnvelope } from "../src/envelope";
import { envelopeValidatorFor } from "../test/support/ajv";

const REPO = path.resolve(import.meta.dir, "..");
const SERVER_NAME = "workbench-mcp";
/** The first-start Tier-1 build over this whole repo, generously bounded. */
const BUILD_DEADLINE_MS = 120_000;

const asJson = process.argv.includes("--json");
const { validate, errorsText } = envelopeValidatorFor(envelopeSchema);

interface Finding {
  readonly kind: "fail" | "note";
  readonly where: string;
  readonly detail: string;
}

const findings: Finding[] = [];

function fail(where: string, detail: string): void {
  findings.push({ kind: "fail", where, detail });
}

function note(where: string, detail: string): void {
  findings.push({ kind: "note", where, detail });
}

function say(line: string): void {
  if (!asJson) console.log(line);
}

/** The registered launch, read out of `.mcp.json` rather than restated here. */
function registeredArgv(): { command: string; args: string[] } {
  const config = JSON.parse(readFileSync(path.join(REPO, ".mcp.json"), "utf8")) as {
    mcpServers?: Record<string, { command?: string; args?: string[] }>;
  };
  const entry = config.mcpServers?.[SERVER_NAME];
  if (entry?.command === undefined) {
    throw new Error(`.mcp.json has no ${SERVER_NAME} server entry with a command`);
  }
  return { command: entry.command, args: entry.args ?? [] };
}

function envelopeFrom(result: unknown): ResponseEnvelope {
  const content = (result as { content?: Array<{ type: string; text: string }> }).content;
  const first = content?.[0];
  if (first === undefined || first.type !== "text") {
    throw new Error("tool result carried no text content");
  }
  return JSON.parse(first.text) as ResponseEnvelope;
}

/** Rendering is best-effort: results are tool-specific by design. */
function digest(results: readonly unknown[]): string[] {
  return results.map((raw) => {
    const r = raw as Record<string, unknown>;
    const file = (r.file ?? r.path ?? r.name ?? "") as string;
    const line = r.line ?? (r.lines as { start?: number } | undefined)?.start;
    const score = typeof r.score === "number" ? ` score=${r.score.toFixed(3)}` : "";
    const kind = typeof r.kind === "string" ? ` [${r.kind}]` : "";
    const head = `${file}${line === undefined ? "" : `:${String(line)}`}${kind}${score}`;
    return head.trim() === "" ? JSON.stringify(raw).slice(0, 120) : head;
  });
}

/** One tool call, reported: schema validity, freshness tier, budget, top hits. */
async function call(
  client: Client,
  label: string,
  name: string,
  args: Record<string, unknown>,
): Promise<ResponseEnvelope | null> {
  let envelope: ResponseEnvelope;
  try {
    envelope = envelopeFrom(await client.callTool({ name, arguments: args }));
  } catch (error) {
    fail(label, `call threw: ${String(error)}`);
    return null;
  }
  if (!validate(envelope)) {
    fail(label, `envelope invalid against schemas/envelope.schema.json: ${errorsText()}`);
    return envelope;
  }
  if (!envelope.ok) {
    // A typed refusal is a legitimate answer, not a crash — Constitution I.
    note(label, `${envelope.error.code} (recovery: ${envelope.error.recovery})`);
    say(`  ${label.padEnd(22)} refused  ${envelope.error.code} -> ${envelope.error.recovery}`);
    return envelope;
  }
  if (envelope.freshness === undefined) {
    fail(label, "success envelope carried no freshness tier");
  }
  const used = envelope.budget?.used ?? 0;
  const requested = envelope.budget?.requested ?? 0;
  const count = envelope.results?.length ?? 0;
  say(
    `  ${label.padEnd(22)} ok  freshness=${String(envelope.freshness).padEnd(7)} ` +
      `results=${String(count).padStart(3)}  budget=${used}/${requested}`,
  );
  for (const line of digest(envelope.results ?? []).slice(0, 3)) say(`      ${line}`);
  if (envelope.truncation !== undefined) {
    const cut = envelope.truncation;
    say(`      cut ${cut.dropped} (${cut.reason}); narrow: ${cut.narrow.join(", ") || "-"}`);
    if (cut.narrow.length === 0) note(label, "truncated with an empty narrow-hint list");
  }
  return envelope;
}

const { command, args } = registeredArgv();
say(`dogfood: ${SERVER_NAME} as registered in .mcp.json -> ${command} ${args.join(" ")}`);
say(`dogfood: workspace = this repository (${REPO})`);

const client = new Client({ name: "dogfood-smoke", version: "0.0.0" });
const transport = new StdioClientTransport({
  command,
  args,
  cwd: REPO,
  env: process.env as Record<string, string>,
});
const startedAt = Date.now();
await client.connect(transport);
const serverPid = transport.pid;

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function killTree(pid: number): void {
  if (process.platform === "win32") {
    spawnSync("taskkill", ["/F", "/T", "/PID", String(pid)], { stdio: "ignore" });
    return;
  }
  try {
    process.kill(pid, "SIGKILL");
  } catch {
    // Already gone between the liveness probe and here.
  }
}

/**
 * The SDK's `close()` already escalates stdin EOF -> SIGTERM -> SIGKILL, and
 * that is enough right up until the first engine call. Once the engine child
 * exists it outlives the kill — SIGKILL cannot run main.ts's release() path,
 * so nothing reaps the child — and it keeps the workspace lock held and the
 * stdio pipes open, so the next start of this workspace fails with exit 75.
 *
 * This driver spawned the tree, so it reaps the tree, using the server's own
 * published handle: the pid recorded in `.workbench/server.lock`. It says so
 * every time it has to, because a leak that gets silently cleaned up is a
 * leak nobody fixes.
 */
function reapServer(spawnedPid: number | null): void {
  const leaked: number[] = [];
  if (spawnedPid !== null && alive(spawnedPid)) leaked.push(spawnedPid);
  try {
    const lock = JSON.parse(readFileSync(path.join(REPO, ".workbench", "server.lock"), "utf8")) as {
      pid?: number;
    };
    if (typeof lock.pid === "number" && lock.pid !== spawnedPid && alive(lock.pid)) {
      leaked.push(lock.pid);
    }
  } catch {
    // No lock file, or it went away with a clean shutdown — nothing to reap.
  }
  if (leaked.length === 0) return;
  note(
    "lifecycle",
    `pid(s) ${leaked.join(", ")} survived a clean client close (the SDK escalates to ` +
      "SIGKILL); force-killed by the driver, which would otherwise leave the workspace " +
      "lock held and the next start refused with exit 75",
  );
  for (const pid of leaked) killTree(pid);
}

try {
  const server = client.getServerVersion();
  if (server?.name !== SERVER_NAME) {
    fail("initialize", `server identified as ${String(server?.name)}, expected ${SERVER_NAME}`);
  }

  say("\ntools/list");
  const tools = await client.listTools();
  const names = tools.tools.map((t) => t.name).sort();
  say(`  ${names.join(", ")}`);
  for (const required of ["search", "map", "read", "workbench_status"]) {
    if (!names.includes(required)) fail("tools/list", `missing tool ${required}`);
  }

  say("\nstatus");
  await call(client, "workbench_status", "workbench_status", {});

  // Tier 0 first: the first search kicks the non-blocking Tier-1 build, and
  // the answer must arrive without waiting for it (SDD §7.3.2).
  say("\nsearch - cold (kicks the first-start build)");
  const cold = await call(client, "lexical/cold", "search", {
    query: "ResponseEnvelope",
    mode: "lexical",
    context: 0,
  });
  say(`  cold answer at +${((Date.now() - startedAt) / 1000).toFixed(2)}s from spawn`);
  if (cold?.ok === true && cold.freshness === "warm") {
    note("lexical/cold", "first answer was already warm - a prior index survived in .workbench/");
  }

  say("\ntasks - first-start index/build");
  const listed = await client.request({ method: "tasks/list", params: {} }, ListTasksResultSchema);
  const listedIds = listed.tasks.map((t) => t.taskId);
  say(`  tasks/list -> ${listedIds.join(", ") || "(none)"}`);
  if (!listedIds.includes("index/build")) {
    fail("tasks/list", "the first-start build did not register as index/build");
  }

  const deadline = Date.now() + BUILD_DEADLINE_MS;
  let polls = 0;
  let last: { status: string; statusMessage?: string | undefined } | undefined;
  for (;;) {
    const task = await client.request(
      { method: "tasks/get", params: { taskId: "index/build" } },
      GetTaskResultSchema,
    );
    polls++;
    if (task.taskId !== "index/build") fail("tasks/get", `echoed taskId ${task.taskId}`);
    if (!["working", "completed", "failed"].includes(task.status)) {
      fail("tasks/get", `illegal status ${task.status}`);
    }
    last = task;
    if (task.status !== "working") break;
    if (Date.now() > deadline) {
      fail("tasks/get", `index/build still working after ${BUILD_DEADLINE_MS}ms`);
      break;
    }
    await new Promise((resolve) => setTimeout(resolve, task.pollInterval ?? 250));
  }
  const buildS = ((Date.now() - startedAt) / 1000).toFixed(2);
  say(
    `  tasks/get x ${polls} -> ${String(last?.status)} - ${last?.statusMessage ?? ""} (+${buildS}s)`,
  );
  if (last?.status !== "completed") {
    fail("tasks/get", `index/build ended ${String(last?.status)} on this repository`);
  }

  say("\nsearch - warm, all four live modes");
  const warm = await call(client, "lexical/warm", "search", {
    query: "ResponseEnvelope",
    mode: "lexical",
    context: 0,
  });
  if (warm?.ok === true && warm.freshness !== "warm") {
    note("lexical/warm", `post-build answer was ${String(warm.freshness)}, not warm`);
  }
  await call(client, "structural", "search", {
    query: "readFileSync($$$A)",
    mode: "structural",
    context: 0,
  });
  await call(client, "references", "search", {
    query: "runSearch",
    mode: "references",
    context: 0,
  });
  // The natural spelling an agent reaches for first. Bun.Glob does not let a
  // leading `*` cross a `/`, so in a repo whose tests all live under `test/`
  // this answers zero — silently. Kept alongside the anchored spelling so the
  // gap stays visible rather than being quietly worked around.
  const bare = await call(client, "files/bare-glob", "search", {
    query: "*.test.ts",
    mode: "files",
  });
  const anchored = await call(client, "files/anchored-glob", "search", {
    query: "**/*.test.ts",
    mode: "files",
  });
  if (bare?.ok === true && anchored?.ok === true) {
    const bareCount = bare.results?.length ?? 0;
    const anchoredCount = anchored.results?.length ?? 0;
    if (bareCount === 0 && anchoredCount > 0) {
      note(
        "files/bare-glob",
        `"*.test.ts" answered 0 while "**/*.test.ts" answered ${anchoredCount}: ` +
          "an unanchored glob silently matches only the workspace root",
      );
    }
  }
  await call(client, "lexical/scoped", "search", {
    query: "freshness",
    mode: "lexical",
    path: "src/**",
    context: 0,
  });
  // A query with thousands of hits across a real tree: the cut must be loud.
  await call(client, "lexical/truncating", "search", {
    query: "const",
    mode: "lexical",
    budget: 600,
  });

  say("\nmap - this repository");
  await call(client, "map/skeleton", "map", { path: ".", depth: 2, detail: "skeleton" });
  await call(client, "map/outline", "map", { path: "src", depth: 3, detail: "outline" });

  say("\nread - this repository's own source");
  await call(client, "read/lines", "read", {
    file: "src/tools/search.ts",
    lines: { start: 339, end: 360 },
  });
  await call(client, "read/symbol", "read", {
    file: "src/tools/search.ts",
    symbol: "runSearch",
    surround: "enclosing",
  });
  await call(client, "read/whole", "read", { file: "src/envelope/index.ts", budget: 1500 });
  await call(client, "read/missing", "read", { file: "src/does-not-exist.ts" });
} finally {
  await client.close();
  reapServer(serverPid);
}

const failures = findings.filter((f) => f.kind === "fail");
const notes = findings.filter((f) => f.kind === "note");

if (asJson) {
  console.log(JSON.stringify({ ok: failures.length === 0, findings }, null, 2));
} else {
  say(`\n${"-".repeat(72)}`);
  if (notes.length > 0) {
    say(`notes (${notes.length}) - degraded or typed answers, not failures:`);
    for (const entry of notes) say(`  . ${entry.where}: ${entry.detail}`);
  }
  if (failures.length > 0) {
    say(`FAILURES (${failures.length}):`);
    for (const entry of failures) say(`  x ${entry.where}: ${entry.detail}`);
  }
  const elapsed = ((Date.now() - startedAt) / 1000).toFixed(1);
  say(
    failures.length === 0
      ? `dogfood: read path serves its own repository (${elapsed}s total)`
      : `dogfood: ${failures.length} failure(s)`,
  );
}

process.exit(failures.length === 0 ? 0 : 1);
