#!/usr/bin/env bun
/**
 * Codex parent/subagent check (specs/018-multi-client-daemon, acceptance
 * matrix row 23; root cause in specs/018-multi-client-daemon/root-cause.md).
 *
 * The production incident: a Codex CLI parent had every Workbench tool, its
 * `spawn_agent` subagent inherited the names, then every call answered
 * "MCP tool `workbench-mcp/read` is not available to the model". Codex starts
 * one MCP server process per subagent thread, the second `workbench-mcp <ws>`
 * exited 75 on the process-lifetime lock before `initialize`, and neither the
 * `--json` event stream nor stderr ever mentioned it.
 *
 * This script is that reproduction, checked in. It runs the REAL `codex exec`
 * under an isolated `CODEX_HOME` whose only MCP server is a shell wrapper that
 * logs every launch with pid and exit code before exec'ing this repo's
 * `src/main.ts` against a disposable workspace; the prompt makes the parent
 * call `read`, spawn one subagent that calls `read`, and answer through
 * `--output-schema` so the outcome is a JSON object rather than prose. Three
 * independent witnesses decide the verdict: the parent's `mcp_tool_call`
 * events, the schema-shaped final answer (the subagent's calls never appear
 * in the event stream; on 0.153.4 even `spawn_agent` shows up only as a
 * `collab_tool_call` named `wait`), and the wrapper's launch log.
 *
 * Opt-in (registered as the optional `codex-subagent` Magistr gate): it skips
 * with exit 0 when `codex` is not on PATH or no `auth.json` exists to copy.
 * The user's real `config.toml` is never read or copied — only `auth.json`,
 * mode 0600, deleted again before exit.
 *
 * Usage: bun run scripts/codex-subagent-check.ts [--json] [--keep] [--timeout-sec N]
 *
 * Exit codes: 0 pass or skipped; 1 the subagent could not use Workbench, a
 * server exited 75, or the run proved nothing (no subagent was spawned);
 * 2 harness failure (codex exec crashed or timed out).
 */

import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import path from "node:path";

const REPO = path.resolve(import.meta.dir, "..");
const MAIN = path.join(REPO, "src", "main.ts");
const SERVER_NAME = "workbench-mcp";
const FILE_UNDER_TEST = "src/a.ts";
const FILE_CONTENT = "export const a = 1;\n";

const argv = process.argv.slice(2);
const asJson = argv.includes("--json");
const keep = argv.includes("--keep");
const timeoutSec = Number(argv[argv.indexOf("--timeout-sec") + 1] || 0) || 600;

interface ParentCall {
  readonly tool: string;
  readonly status: string;
  readonly isError: boolean;
  readonly text: string;
}

interface FinalAnswer {
  readonly parent_read_ok: boolean;
  readonly parent_read_raw: string;
  readonly subagent_read_ok: boolean;
  readonly subagent_read_raw: string;
  readonly subagent_workbench_tool_names: readonly string[];
}

interface LaunchLog {
  readonly launches: number;
  readonly exits: readonly number[];
  readonly exit75: number;
  readonly raw: string;
}

interface Report {
  readonly verdict: "pass" | "fail" | "inconclusive" | "harness-error";
  readonly reasons: readonly string[];
  readonly codexVersion: string;
  readonly codexExit: number | null;
  readonly parentCalls: readonly ParentCall[];
  /** collab_tool_call names seen (spawn_agent, wait, …): any one means a subagent existed. */
  readonly collabTools: readonly string[];
  readonly finalAnswer: FinalAnswer | null;
  readonly launchLog: LaunchLog;
  readonly scratch: string;
}

function skip(reason: string): never {
  if (asJson) console.log(JSON.stringify({ verdict: "skipped", reason }));
  else console.log(`codex-subagent-check: SKIPPED — ${reason}`);
  process.exit(0);
}

const OUTPUT_SCHEMA = {
  type: "object",
  properties: {
    parent_read_ok: { type: "boolean" },
    parent_read_raw: { type: "string" },
    subagent_read_ok: { type: "boolean" },
    subagent_read_raw: { type: "string" },
    subagent_workbench_tool_names: { type: "array", items: { type: "string" } },
  },
  required: [
    "parent_read_ok",
    "parent_read_raw",
    "subagent_read_ok",
    "subagent_read_raw",
    "subagent_workbench_tool_names",
  ],
  additionalProperties: false,
} as const;

const SUBAGENT_INSTRUCTION =
  `Call the MCP tool \`read\` from the server \`${SERVER_NAME}\` with arguments ` +
  `{"file":"${FILE_UNDER_TEST}"}. Reply with (1) the raw result exactly as returned, ` +
  "including any error text, and (2) the names of every tool available to you whose " +
  "name contains 'workbench', one per line. Do not retry, do not use the shell.";

const PROMPT = [
  "You are a harness check. Do exactly the following steps and nothing else; never use the shell.",
  `1. Call the MCP tool \`read\` from the server \`${SERVER_NAME}\` with arguments {"file":"${FILE_UNDER_TEST}"}. Keep the raw result text.`,
  `2. Use spawn_agent to start ONE subagent with this instruction, verbatim: ${JSON.stringify(SUBAGENT_INSTRUCTION)}`,
  "3. Wait for that subagent to finish and read its reply.",
  "4. Answer with the JSON object the output schema requires: parent_read_ok is true only if your own read " +
    `returned the file content ${JSON.stringify(FILE_CONTENT.trim())} with no error; subagent_read_ok is true only if the ` +
    "subagent's raw result contained that same content and no error; copy both raw results verbatim; " +
    "subagent_workbench_tool_names is the list the subagent reported.",
].join("\n");

function parseEvents(jsonl: string): { parentCalls: ParentCall[]; collabTools: string[] } {
  const parentCalls: ParentCall[] = [];
  const collabTools: string[] = [];
  for (const line of jsonl.split("\n")) {
    if (!line.trim()) continue;
    let event: unknown;
    try {
      event = JSON.parse(line);
    } catch {
      continue;
    }
    const item = (event as { item?: Record<string, unknown> }).item;
    if (item === undefined) continue;
    const eventType = (event as { type?: string }).type;
    if (eventType === "item.completed" && item["type"] === "collab_tool_call") {
      collabTools.push(String(item["tool"]));
    }
    if (
      eventType === "item.completed" &&
      item["type"] === "mcp_tool_call" &&
      item["server"] === SERVER_NAME
    ) {
      const result = item["result"] as
        | { content?: Array<{ text?: string }>; isError?: boolean }
        | null
        | undefined;
      parentCalls.push({
        tool: String(item["tool"]),
        status: String(item["status"]),
        isError: result?.isError === true || item["error"] !== null,
        text: result?.content?.[0]?.text ?? String(item["error"] ?? ""),
      });
    }
  }
  return { parentCalls, collabTools };
}

/** The `-o` last message: JSON by schema, tolerating a fenced code block. */
function parseFinalAnswer(text: string): FinalAnswer | null {
  const candidates = [text.trim(), text.replace(/^```(?:json)?\s*|\s*```$/g, "").trim()];
  for (const candidate of candidates) {
    try {
      const parsed = JSON.parse(candidate) as Partial<FinalAnswer>;
      if (typeof parsed.subagent_read_ok === "boolean") {
        return {
          parent_read_ok: parsed.parent_read_ok === true,
          parent_read_raw: String(parsed.parent_read_raw ?? ""),
          subagent_read_ok: parsed.subagent_read_ok,
          subagent_read_raw: String(parsed.subagent_read_raw ?? ""),
          subagent_workbench_tool_names: Array.isArray(parsed.subagent_workbench_tool_names)
            ? parsed.subagent_workbench_tool_names.map(String)
            : [],
        };
      }
    } catch {
      // try the next shape
    }
  }
  return null;
}

function parseLaunchLog(raw: string): LaunchLog {
  const launches = raw.split("\n").filter((line) => line.includes(" LAUNCH ")).length;
  const exits = [...raw.matchAll(/ EXIT pid=\d+ code=(\d+)/g)].map((m) => Number(m[1]));
  return { launches, exits, exit75: exits.filter((code) => code === 75).length, raw };
}

function print(report: Report): void {
  if (asJson) {
    console.log(JSON.stringify(report, null, 2));
    return;
  }
  const lines = [
    `codex-subagent-check: ${report.verdict.toUpperCase()} (codex ${report.codexVersion}, exit ${report.codexExit})`,
    `  parent ${SERVER_NAME} calls via --json events: ${
      report.parentCalls.length === 0
        ? "none"
        : report.parentCalls
            .map((c) => `${c.tool} ${c.status}${c.isError ? " ERROR" : ""}`)
            .join(", ")
    }`,
    `  collab tool calls (subagent evidence): ${report.collabTools.length === 0 ? "none" : report.collabTools.join(", ")}`,
    `  final answer: ${
      report.finalAnswer === null
        ? "unparseable"
        : `parent_read_ok=${report.finalAnswer.parent_read_ok} subagent_read_ok=${report.finalAnswer.subagent_read_ok} ` +
          `subagent_workbench_tool_names=${JSON.stringify(report.finalAnswer.subagent_workbench_tool_names)}`
    }`,
    `  subagent raw: ${JSON.stringify(report.finalAnswer?.subagent_read_raw.slice(0, 300) ?? "")}`,
    `  wrapper launch log: ${report.launchLog.launches} launch(es), exit codes [${report.launchLog.exits.join(", ")}], exit-75 count ${report.launchLog.exit75}`,
  ];
  for (const reason of report.reasons) lines.push(`  - ${reason}`);
  lines.push(`  scratch: ${report.scratch}`);
  console.log(lines.join("\n"));
}

async function main(): Promise<number> {
  const codex = Bun.which("codex");
  if (codex === null) skip("`codex` is not on PATH");
  const authSource = path.join(
    process.env["CODEX_HOME"] ?? path.join(homedir(), ".codex"),
    "auth.json",
  );
  if (!existsSync(authSource)) skip(`no Codex auth to copy (${authSource} does not exist)`);
  const codexVersion = new TextDecoder()
    .decode(Bun.spawnSync([codex, "--version"], { stdout: "pipe", stderr: "pipe" }).stdout)
    .trim();

  const scratch = mkdtempSync(path.join(tmpdir(), "workbench-codex-check-"));
  const ws = path.join(scratch, "ws");
  const codexHome = path.join(scratch, "codex-home");
  const authCopy = path.join(codexHome, "auth.json");
  const wrapper = path.join(scratch, "workbench-mcp-wrapper.sh");
  const launchLogPath = path.join(scratch, "launches.log");
  const serverStderrPath = path.join(scratch, "server-stderr.log");
  const schemaPath = path.join(scratch, "output-schema.json");
  const lastPath = path.join(scratch, "last-message.txt");
  const eventsPath = path.join(scratch, "events.jsonl");
  const codexStderrPath = path.join(scratch, "codex-stderr.log");

  mkdirSync(path.join(ws, "src"), { recursive: true });
  writeFileSync(path.join(ws, FILE_UNDER_TEST), FILE_CONTENT);
  writeFileSync(
    path.join(ws, "package.json"),
    JSON.stringify({ name: "harness", version: "0.0.0" }),
  );
  mkdirSync(codexHome, { recursive: true, mode: 0o700 });
  writeFileSync(
    wrapper,
    [
      "#!/bin/sh",
      `echo "$(date +%H:%M:%S) LAUNCH pid=$$ ppid=$PPID" >> "${launchLogPath}"`,
      `"${process.execPath}" "${MAIN}" "${ws}" 2>>"${serverStderrPath}"`,
      "code=$?",
      `echo "$(date +%H:%M:%S) EXIT pid=$$ code=$code" >> "${launchLogPath}"`,
      "exit $code",
      "",
    ].join("\n"),
  );
  chmodSync(wrapper, 0o755);
  writeFileSync(launchLogPath, "");
  writeFileSync(serverStderrPath, "");
  writeFileSync(schemaPath, JSON.stringify(OUTPUT_SCHEMA, null, 2));
  // Only the server under test, so nothing from the user's real config.toml
  // (secrets, other servers) can reach this run; trust the disposable root so
  // Codex does not stop to ask.
  writeFileSync(
    path.join(codexHome, "config.toml"),
    [
      `[mcp_servers.${SERVER_NAME}]`,
      `command = ${JSON.stringify(wrapper)}`,
      "args = []",
      "startup_timeout_sec = 60",
      "tool_timeout_sec = 300",
      "required = true",
      "",
      `[projects.${JSON.stringify(ws)}]`,
      'trust_level = "trusted"',
      "",
    ].join("\n"),
  );

  let codexExit: number | null = null;
  let events = "";
  let harnessError: string | undefined;
  try {
    copyFileSync(authSource, authCopy);
    chmodSync(authCopy, 0o600);
    const proc = Bun.spawn(
      [
        codex,
        "exec",
        "--json",
        "--skip-git-repo-check",
        "--dangerously-bypass-approvals-and-sandbox",
        "-C",
        ws,
        "-o",
        lastPath,
        "--output-schema",
        schemaPath,
        PROMPT,
      ],
      {
        cwd: ws,
        env: { ...process.env, CODEX_HOME: codexHome },
        // Without a closed stdin `codex exec` blocks on "Reading additional
        // input from stdin" and launches nothing.
        stdin: "ignore",
        stdout: "pipe",
        stderr: "pipe",
      },
    );
    const stdoutText = new Response(proc.stdout).text();
    const stderrText = new Response(proc.stderr).text();
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      proc.kill();
    }, timeoutSec * 1_000);
    codexExit = await proc.exited;
    clearTimeout(timer);
    events = await stdoutText;
    writeFileSync(eventsPath, events);
    writeFileSync(codexStderrPath, await stderrText);
    if (timedOut) harnessError = `codex exec exceeded ${timeoutSec}s and was killed`;
  } catch (error) {
    harnessError = `codex exec could not run: ${String(error)}`;
  } finally {
    rmSync(authCopy, { force: true });
  }

  const { parentCalls, collabTools } = parseEvents(events);
  const finalAnswer = existsSync(lastPath)
    ? parseFinalAnswer(readFileSync(lastPath, "utf8"))
    : null;
  const launchLog = parseLaunchLog(readFileSync(launchLogPath, "utf8"));

  const reasons: string[] = [];
  let verdict: Report["verdict"] = "pass";
  if (harnessError !== undefined || codexExit !== 0) {
    verdict = "harness-error";
    reasons.push(harnessError ?? `codex exec exited ${codexExit} (see ${codexStderrPath})`);
  } else {
    const parentRead = parentCalls.find((call) => call.tool === "read");
    if (parentRead === undefined || parentRead.isError) {
      verdict = "fail";
      reasons.push(
        `the parent's ${SERVER_NAME}/read did not succeed (events: ${parentRead?.text ?? "no read call"})`,
      );
    }
    // A subagent existed if the parent used any collab tool on it, or if the
    // wrapper saw a second server launch (Codex starts one per thread).
    const subagentExercised = collabTools.length > 0 || launchLog.launches >= 2;
    if (!subagentExercised) {
      verdict = "inconclusive";
      reasons.push(
        "no collab tool call and no second server launch — the subagent path was not exercised",
      );
    } else if (finalAnswer === null) {
      verdict = "inconclusive";
      reasons.push(`the final answer did not follow the output schema (see ${lastPath})`);
    } else if (!finalAnswer.subagent_read_ok) {
      verdict = "fail";
      reasons.push(
        `the subagent's ${SERVER_NAME}/read failed: ${finalAnswer.subagent_read_raw.slice(0, 200)}`,
      );
    }
    if (launchLog.exit75 > 0) {
      verdict = "fail";
      reasons.push(
        `${launchLog.exit75} server process(es) exited 75 — refused by the workspace lock (see ${serverStderrPath})`,
      );
    }
    if (launchLog.launches < 2) {
      if (verdict === "pass") verdict = "inconclusive";
      reasons.push(
        `only ${launchLog.launches} server launch(es) logged; a parent plus a subagent should show two`,
      );
    }
  }

  const report: Report = {
    verdict,
    reasons,
    codexVersion,
    codexExit,
    parentCalls,
    collabTools,
    finalAnswer,
    launchLog,
    scratch,
  };
  print(report);

  if (verdict === "pass" && !keep) rmSync(scratch, { recursive: true, force: true });
  return verdict === "pass" ? 0 : verdict === "harness-error" ? 2 : 1;
}

process.exit(await main());
