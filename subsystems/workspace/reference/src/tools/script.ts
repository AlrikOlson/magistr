/**
 * The `script` tool (SDD §9.7, law #4) — typed Code Mode for the long tail.
 * Agents write better code than tool-call chains; bash is an untyped code
 * mode, so this is the typed one. The source runs as a JavaScript function
 * body in a QuickJS-ng-on-WASM guest under the §8.1 Wasmtime host: fuel-
 * metered, memory-capped, zero ambient filesystem/network/shell. The guest
 * sees ONE global, `workspace`:
 *
 *   search(args) / read(args)   live, jail-checked, answering the same
 *                               envelopes the MCP tools answer
 *   glob(pattern)               matching workspace paths (junk excluded)
 *   edit.queue(op)              QUEUED replace_text-shaped write
 *   files.queue(op)             QUEUED create | mkdir | delete
 *
 * Live reads against an argv-and-stdout-only guest work by DETERMINISTIC
 * RE-ENTRY: the guest's workspace shim serves host call k from an injected
 * response journal when k is already answered, and otherwise emits a
 * nonce-framed request sentinel on stdout and unwinds; the host services the
 * request through the same dep-injected tool functions the MCP surface uses,
 * appends to the journal, and re-runs. The sandbox has no preopens, no env
 * and no network, so a script is a pure function of (source, journal) — the
 * shim verifies each replayed request against the journal and reports
 * divergence (a guest steering host calls with Date.now/Math.random) as a
 * typed guest failure instead of silently serving it.
 *
 * Writes are exactly as safe as `edit` (§9.7): the final round's queue
 * resolves to full-content WriteOps through the §7.5 choke point, and the
 * batch is ALWAYS confirm-gated — ad-hoc code has no single reviewable
 * intent, so the aggregated dry-run diff is the only truth a human can
 * approve (the same posture as files.move). dryRun:true (the default) never
 * applies. Confirm re-invocation re-runs the script; determinism reproduces
 * the queue, which is exactly the re-resolution property edit's stateless
 * token verification already assumes.
 *
 * Sandbox kills map one-to-one onto the taxonomy: out-of-fuel →
 * SANDBOX_FUEL_EXCEEDED, memory-cap → SANDBOX_MEMORY_EXCEEDED, and a
 * capability violation AT THE HOST-CALL LAYER (a jail-escaping read, an
 * unknown host method, a queue op naming a jailed path) → SANDBOX_CAP_DENIED.
 * A guest that merely throws is DATA (status "threw"), like a failing test.
 *
 * The guest's sentinel nonce is host-generated per call and blanked from
 * scriptArgs before user code runs; a user-printed forgery lacking the nonce
 * is ignored, and only exit-0 rounds are parsed at all. A guest that escapes
 * the source wrapper syntactically can at worst lie to ITS OWN caller about
 * host results — the jail still decides every real read, and the confirm
 * gate still fronts every real write.
 *
 * This layer sees only ports and the envelope contract (import-graph rule);
 * guest execution and file access arrive through injected deps.
 */

import { createHash, randomBytes } from "node:crypto";
import { okEnvelope, type ResponseEnvelope, taxonomyError } from "../envelope";
import type { IndexPort } from "../ports";
import { aggregateHashes, type ConfirmPort, confirmDiffNotes, jailRefusal } from "./edit";
import { type ReadDeps, type ReadInput, runRead } from "./read";
import { runSearch, type SearchDeps, type SearchInput, type WorkspaceFile } from "./search";
import {
  runWriteBatch,
  type WriteEngineDeps,
  type WriteOp,
  type WriteReport,
} from "./write-engine";

/** Default CPU budget per guest round (~400x the measured qjs boot cost). */
export const SCRIPT_FUEL_DEFAULT = 1_000_000_000;

/** Default guest linear-memory cap (SDD §9.7: 256MB). */
export const SCRIPT_MEMORY_CAP_DEFAULT = 256 * 1024 * 1024;

/** Host-call ceiling: N calls cost N+1 guest rounds, so this bounds both. */
export const SCRIPT_HOST_CALLS_MAX = 32;

/** Ceiling on the return-value JSON echoed into CONFIRM_REQUIRED notes. */
export const SCRIPT_CONFIRM_VALUE_CHARS = 2000;

const SCRIPT_BUDGET_DEFAULT = 4000;

/** How one guest round ended — mirrors the engine child's wasi outcome. */
export type ScriptGuestOutcomeKind =
  | "ok"
  | "out-of-fuel"
  | "memory-cap"
  | "cap-denied"
  | "guest-trap"
  | "guest-exit"
  | "invalid-guest";

export interface ScriptGuestOutcome {
  readonly outcome: ScriptGuestOutcomeKind;
  readonly detail: string;
  readonly fuelUsed: number;
  readonly exitCode: number | null;
  readonly stdout: string;
  readonly stderr: string;
}

export interface ScriptGuestPolicy {
  readonly fuel: number;
  readonly memoryCapBytes: number;
}

/** Jail-checked read access for the guest's live read lane. */
export type ScriptOpenRead =
  | { readonly ok: true; readonly access: ReadDeps["workspace"] }
  | { readonly ok: false; readonly reason: string };

export interface ScriptDeps {
  readonly search: SearchDeps;
  readonly write: WriteEngineDeps;
  readonly confirm: ConfirmPort;
  readonly index: IndexPort;
  /** Jail decision + file access for one workspace-relative path (§7.1). */
  openRead(path: string): Promise<ScriptOpenRead>;
  /** One guest round in the engine child (SDD §6.1/§8.1). */
  runGuest(argv: readonly string[], policy: ScriptGuestPolicy): Promise<ScriptGuestOutcome>;
  /** Workspace file listing for glob (search's own listing). */
  list(): Promise<readonly WorkspaceFile[]>;
}

/** Wire shape of one tool call, mirroring schemas/tools/script.input.schema.json. */
export interface ScriptInput {
  readonly source: string;
  readonly dryRun?: boolean;
  readonly confirm?: string;
  readonly autoApply?: boolean;
  readonly fuel?: number;
  readonly memoryCapBytes?: number;
  readonly budget?: number;
}

/** The script row of the result envelope; write rows follow when queued. */
export interface ScriptResult {
  readonly kind: "script";
  readonly status: "ok" | "threw";
  readonly value?: unknown;
  readonly error?: string;
  readonly stdout: string;
  readonly stderr: string;
  readonly fuelUsed: number;
  readonly rounds: number;
  readonly hostCalls: number;
  readonly queuedWrites: number;
}

function sha256(content: string): string {
  return createHash("sha256").update(content, "utf8").digest("hex");
}

/** Queued ops in guest call order, exactly as the shim collected them. */
interface QueuedEdit {
  readonly file?: unknown;
  readonly old?: unknown;
  readonly new?: unknown;
  readonly occurrence?: unknown;
}

interface QueuedFile {
  readonly op?: unknown;
  readonly from?: unknown;
  readonly to?: unknown;
}

interface GuestDone {
  readonly value: unknown;
  readonly threw: string | null;
  readonly edits: readonly QueuedEdit[];
  readonly files: readonly QueuedFile[];
}

/**
 * The bootstrap program: the workspace shim, the replay journal, and the
 * sentinel protocol, wrapped around the user source as a function body.
 * scriptArgs = [journalJson, nonce], blanked before user code runs.
 */
export function buildGuestProgram(source: string): string {
  return (
    '"use strict";(function(){' +
    "var __args=scriptArgs;var __journal=JSON.parse(__args[0]);var __nonce=__args[1];" +
    "globalThis.scriptArgs=[];" +
    "var __calls=0;var __edits=[];var __files=[];" +
    'function __emit(p){print("\\n@@WB["+__nonce+"]"+JSON.stringify(p)+"["+__nonce+"]WB@@");}' +
    "var __PENDING={};" +
    "function __host(method,args){" +
    "var req={method:method,args:args===undefined?null:args};var i=__calls++;" +
    "if(i<__journal.length){var e=__journal[i];" +
    "if(JSON.stringify(req)!==e.k){throw {__wbNondet:i};}return e.r;}" +
    '__emit({k:"call",i:i,req:req});throw __PENDING;}' +
    "var workspace={" +
    'search:function(a){return __host("search",a);},' +
    'read:function(a){return __host("read",a);},' +
    'glob:function(p){return __host("glob",p);},' +
    "edit:{queue:function(op){__edits.push(op);}}," +
    "files:{queue:function(op){__files.push(op);}}};" +
    "var __value;var __threw=null;" +
    "try{__value=(function(workspace){\n" +
    source +
    "\n})(workspace);}" +
    "catch(e){if(e===__PENDING){return;}" +
    'if(e&&typeof e.__wbNondet==="number"){__emit({k:"nondet",i:e.__wbNondet});return;}' +
    "__threw=String(e&&e.message!==undefined?e.message:e);}" +
    'if(__threw!==null){__emit({k:"done",threw:__threw,edits:__edits,files:__files});return;}' +
    "var __v;try{__v=JSON.parse(JSON.stringify(__value===undefined?null:__value));}" +
    'catch(e2){__emit({k:"done",threw:"return value is not JSON-serializable: "+String(e2),' +
    "edits:__edits,files:__files});return;}" +
    '__emit({k:"done",value:__v,edits:__edits,files:__files});' +
    "})();"
  );
}

/** The last nonce-framed sentinel in an exit-0 round's stdout, else null. */
function lastSentinel(stdout: string, nonce: string): Record<string, unknown> | null {
  const open = `@@WB[${nonce}]`;
  const close = `[${nonce}]WB@@`;
  const start = stdout.lastIndexOf(open);
  if (start === -1) return null;
  const end = stdout.indexOf(close, start + open.length);
  if (end === -1) return null;
  const body = stdout.slice(start + open.length, end);
  try {
    const parsed: unknown = JSON.parse(body);
    return typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

/** Strip every sentinel frame from the guest's reported stdout. */
function stripSentinels(stdout: string, nonce: string): string {
  const open = `@@WB[${nonce}]`;
  const close = `[${nonce}]WB@@`;
  let out = stdout;
  for (;;) {
    const start = out.indexOf(open);
    if (start === -1) return out;
    const end = out.indexOf(close, start + open.length);
    if (end === -1) return out.slice(0, start);
    out = out.slice(0, start) + out.slice(end + close.length);
  }
}

/** Bun.Glob dialect, same matcher search's files mode uses. */
function matchesGlob(pattern: string, path: string): boolean {
  return new Bun.Glob(pattern).match(path);
}

type HostCallAnswer =
  | { readonly ok: true; readonly response: unknown }
  | { readonly ok: false; readonly capDenied: string };

/**
 * Service one guest host call through the same tool functions the MCP
 * surface uses. A capability violation — unknown method, malformed request,
 * jail-refused path — denies the CALL, which kills the SCRIPT typed.
 */
async function serviceHostCall(
  deps: ScriptDeps,
  method: unknown,
  args: unknown,
  v: number,
): Promise<HostCallAnswer> {
  if (method === "search") {
    if (typeof args !== "object" || args === null || Array.isArray(args)) {
      return { ok: false, capDenied: "workspace.search expects an args object" };
    }
    const input = args as SearchInput;
    if (typeof input.query !== "string" || input.query.length === 0) {
      return { ok: false, capDenied: "workspace.search requires a non-empty query" };
    }
    return { ok: true, response: await runSearch(deps.search, input, v) };
  }
  if (method === "read") {
    if (typeof args !== "object" || args === null || Array.isArray(args)) {
      return { ok: false, capDenied: "workspace.read expects an args object" };
    }
    const input = args as ReadInput;
    if (typeof input.file !== "string" || input.file.length === 0) {
      return { ok: false, capDenied: "workspace.read requires a non-empty file" };
    }
    const opened = await deps.openRead(input.file);
    if (!opened.ok) {
      return {
        ok: false,
        capDenied: `workspace.read(${JSON.stringify(input.file)}): ${opened.reason}`,
      };
    }
    const readDeps: ReadDeps = { index: deps.index, workspace: opened.access };
    return { ok: true, response: await runRead(readDeps, input, v) };
  }
  if (method === "glob") {
    if (typeof args !== "string" || args.length === 0) {
      return { ok: false, capDenied: "workspace.glob expects a non-empty pattern string" };
    }
    const files = await deps.list();
    const paths = files.filter((f) => !f.junk && matchesGlob(args, f.path)).map((f) => f.path);
    paths.sort();
    return { ok: true, response: paths };
  }
  return { ok: false, capDenied: `unknown host method ${JSON.stringify(method)}` };
}

/** Mirror of read.ts/write-engine.ts: the workspace-relative posix dialect. */
function normalizeRelPath(path: string): string {
  return path
    .replaceAll("\\", "/")
    .split("/")
    .filter((p) => p !== "" && p !== ".")
    .join("/");
}

type QueueResolution =
  | {
      readonly ok: true;
      readonly writes: readonly WriteOp[];
      readonly beforeByPath: ReadonlyMap<string, string>;
      readonly notes: readonly string[];
    }
  | { readonly ok: false; readonly envelope: ResponseEnvelope<never> };

/**
 * Resolve the final round's queue to full-content WriteOps, in guest call
 * order (edits first, then file ops — the shim keeps two arrays). Reads go
 * through the write engine's jail-checked access; a refusal is a capability
 * violation. An `old` that no longer (or never) matched answers STALE_ANCHOR
 * with the fresh hash — the identical contract edit's replace_text pins.
 */
async function resolveQueue(
  deps: ScriptDeps,
  done: GuestDone,
  v: number,
): Promise<QueueResolution> {
  const contentByPath = new Map<string, string | null>();
  const beforeByPath = new Map<string, string>();
  const notes: string[] = [];
  const capDenied = (message: string): QueueResolution => ({
    ok: false,
    envelope: taxonomyError({ v, code: "SANDBOX_CAP_DENIED", message }),
  });

  const load = async (
    path: string,
  ): Promise<{ ok: true; text: string | null } | { ok: false; reason: string }> => {
    if (contentByPath.has(path)) return { ok: true, text: contentByPath.get(path) ?? null };
    let text: string | null;
    try {
      text = await deps.write.workspace.read(path);
    } catch (error) {
      // ENOENT inside the jail is "no such file"; a §7.1 jail refusal is
      // classified by its error code, never by message text (constitution II).
      const refusal = jailRefusal(v, error);
      if (refusal !== null) return { ok: false, reason: refusal.error.message };
      text = null;
    }
    contentByPath.set(path, text);
    if (text !== null) beforeByPath.set(path, text);
    return { ok: true, text };
  };

  for (const [i, op] of done.edits.entries()) {
    if (
      typeof op.file !== "string" ||
      op.file === "" ||
      typeof op.old !== "string" ||
      op.old === "" ||
      typeof op.new !== "string"
    ) {
      return capDenied(`edit.queue #${i}: requires non-empty file, non-empty old, and new`);
    }
    const path = normalizeRelPath(op.file);
    const loaded = await load(path);
    if (!loaded.ok) return capDenied(`edit.queue #${i} (${path}): ${loaded.reason}`);
    if (loaded.text === null) {
      return {
        ok: false,
        envelope: taxonomyError({
          v,
          code: "FILE_NOT_FOUND",
          message: `edit.queue #${i}: ${path} does not exist`,
        }),
      };
    }
    const occurrence = op.occurrence;
    if (
      occurrence !== undefined &&
      (typeof occurrence !== "number" || !Number.isInteger(occurrence) || occurrence < 1)
    ) {
      return capDenied(`edit.queue #${i}: occurrence must be a positive integer`);
    }
    let next: string;
    if (occurrence === undefined) {
      if (!loaded.text.includes(op.old)) {
        return {
          ok: false,
          envelope: taxonomyError({
            v,
            code: "STALE_ANCHOR",
            message: `edit.queue #${i}: old text not found in ${path}`,
            contentHash: sha256(loaded.text),
          }),
        };
      }
      next = loaded.text.replaceAll(op.old, op.new);
    } else {
      let index = -1;
      for (let n = 0; n < occurrence; n++) {
        index = loaded.text.indexOf(op.old, index + 1);
        if (index === -1) break;
      }
      if (index === -1) {
        return {
          ok: false,
          envelope: taxonomyError({
            v,
            code: "STALE_ANCHOR",
            message: `edit.queue #${i}: occurrence ${occurrence} of old text not found in ${path}`,
            contentHash: sha256(loaded.text),
          }),
        };
      }
      next = loaded.text.slice(0, index) + op.new + loaded.text.slice(index + op.old.length);
    }
    contentByPath.set(path, next);
  }

  for (const [i, op] of done.files.entries()) {
    if (op.op === "create" || op.op === "mkdir") {
      if (typeof op.to !== "string" || op.to === "") {
        return capDenied(`files.queue #${i}: ${String(op.op)} requires a non-empty to`);
      }
      const path =
        op.op === "mkdir" ? `${normalizeRelPath(op.to)}/.gitkeep` : normalizeRelPath(op.to);
      const loaded = await load(path);
      if (!loaded.ok) return capDenied(`files.queue #${i} (${path}): ${loaded.reason}`);
      if (loaded.text !== null) {
        notes.push(`files.queue #${i}: ${path} already exists; create never overwrites (§9.5)`);
        continue;
      }
      contentByPath.set(path, "");
    } else if (op.op === "delete") {
      if (typeof op.from !== "string" || op.from === "") {
        return capDenied(`files.queue #${i}: delete requires a non-empty from`);
      }
      const path = normalizeRelPath(op.from);
      const loaded = await load(path);
      if (!loaded.ok) return capDenied(`files.queue #${i} (${path}): ${loaded.reason}`);
      if (loaded.text === null) {
        return {
          ok: false,
          envelope: taxonomyError({
            v,
            code: "FILE_NOT_FOUND",
            message: `files.queue #${i}: ${path} does not exist`,
          }),
        };
      }
      contentByPath.set(path, null);
    } else {
      return capDenied(
        `files.queue #${i}: op must be create, mkdir or delete (move/copy are not queueable from script; use the files tool)`,
      );
    }
  }

  const writes: WriteOp[] = [];
  for (const [path, content] of contentByPath) {
    const before = beforeByPath.get(path);
    if (content === before) continue; // an edit chain that round-tripped
    if (content === null && before === undefined) continue;
    writes.push({ path, content });
  }
  return { ok: true, writes, beforeByPath, notes };
}

type GuestRun =
  | {
      readonly kind: "done";
      readonly done: GuestDone;
      readonly stdout: string;
      readonly stderr: string;
    }
  | { readonly kind: "error"; readonly envelope: ResponseEnvelope<never> }
  | {
      readonly kind: "threw";
      readonly error: string;
      readonly stdout: string;
      readonly stderr: string;
    };

interface GuestLoopResult {
  readonly run: GuestRun;
  readonly rounds: number;
  readonly hostCalls: number;
  readonly fuelUsed: number;
}

/** The re-entry loop: run, service the sentinel, re-run with the journal. */
async function runGuestLoop(
  deps: ScriptDeps,
  input: ScriptInput,
  policy: ScriptGuestPolicy,
  v: number,
): Promise<GuestLoopResult> {
  const program = buildGuestProgram(input.source);
  const nonce = randomBytes(16).toString("hex");
  const journal: { k: string; r: unknown }[] = [];
  let rounds = 0;
  let fuelUsed = 0;
  const finish = (run: GuestRun): GuestLoopResult => ({
    run,
    rounds,
    hostCalls: journal.length,
    fuelUsed,
  });

  for (;;) {
    if (journal.length >= SCRIPT_HOST_CALLS_MAX) {
      return finish({
        kind: "error",
        envelope: taxonomyError({
          v,
          code: "SANDBOX_FUEL_EXCEEDED",
          message:
            `script exceeded the host-call budget (${SCRIPT_HOST_CALLS_MAX} workspace calls); ` +
            "batch reads (one search/glob instead of many) or split the work into smaller scripts",
        }),
      });
    }
    rounds++;
    let out: ScriptGuestOutcome;
    try {
      out = await deps.runGuest(["qjs", "-e", program, JSON.stringify(journal), nonce], policy);
    } catch (error) {
      // Child down, sandbox lane absent (§6.2), or the request was rejected —
      // typed and retryable; the supervisor is already restarting the child.
      return finish({
        kind: "error",
        envelope: taxonomyError({
          v,
          code: "ENGINE_UNAVAILABLE",
          message: `script guest could not run: ${error instanceof Error ? error.message : String(error)}`,
        }),
      });
    }
    fuelUsed += out.fuelUsed;
    if (out.outcome === "out-of-fuel") {
      return finish({
        kind: "error",
        envelope: taxonomyError({
          v,
          code: "SANDBOX_FUEL_EXCEEDED",
          message:
            `script exhausted its fuel allowance (${policy.fuel} per round, round ${rounds}); ` +
            "split the work into smaller scripts or reduce its scope",
        }),
      });
    }
    if (out.outcome === "memory-cap") {
      return finish({
        kind: "error",
        envelope: taxonomyError({
          v,
          code: "SANDBOX_MEMORY_EXCEEDED",
          message:
            `script breached its ${policy.memoryCapBytes}-byte memory cap (round ${rounds}); ` +
            "reduce the working set — stream over files instead of accumulating them",
        }),
      });
    }
    if (
      out.outcome === "guest-trap" ||
      out.outcome === "invalid-guest" ||
      out.outcome === "cap-denied"
    ) {
      // The interpreter artifact itself misbehaved — an engine-side problem,
      // not the user's program. §6.2: typed, retryable, supervisor-adjacent.
      return finish({
        kind: "error",
        envelope: taxonomyError({
          v,
          code: "ENGINE_UNAVAILABLE",
          message: `script guest interpreter failed (${out.outcome}): ${out.detail}`,
        }),
      });
    }
    if (out.outcome === "guest-exit" && out.exitCode !== 0) {
      const error =
        out.stderr.trim() === "" ? `guest exited with code ${out.exitCode}` : out.stderr.trim();
      return finish({
        kind: "threw",
        error,
        stdout: stripSentinels(out.stdout, nonce),
        stderr: out.stderr,
      });
    }
    const sentinel = lastSentinel(out.stdout, nonce);
    if (sentinel === null) {
      return finish({
        kind: "threw",
        error: "guest produced no result frame (the source may have escaped the wrapper)",
        stdout: stripSentinels(out.stdout, nonce),
        stderr: out.stderr,
      });
    }
    if (sentinel["k"] === "nondet") {
      return finish({
        kind: "threw",
        error:
          `nondeterministic host-call replay at call #${String(sentinel["i"])}: the script's ` +
          "workspace calls must not depend on Date.now/Math.random or other per-run state",
        stdout: stripSentinels(out.stdout, nonce),
        stderr: out.stderr,
      });
    }
    if (sentinel["k"] === "call") {
      const req = sentinel["req"] as { method?: unknown; args?: unknown } | undefined;
      const answer = await serviceHostCall(deps, req?.method, req?.args, v);
      if (!answer.ok) {
        return finish({
          kind: "error",
          envelope: taxonomyError({ v, code: "SANDBOX_CAP_DENIED", message: answer.capDenied }),
        });
      }
      journal.push({
        k: JSON.stringify({ method: req?.method, args: req?.args ?? null }),
        r: answer.response,
      });
      continue;
    }
    if (sentinel["k"] === "done") {
      const threw = sentinel["threw"];
      const edits = Array.isArray(sentinel["edits"]) ? (sentinel["edits"] as QueuedEdit[]) : [];
      const files = Array.isArray(sentinel["files"]) ? (sentinel["files"] as QueuedFile[]) : [];
      const done: GuestDone = {
        value: sentinel["value"] ?? null,
        threw: typeof threw === "string" ? threw : null,
        edits,
        files,
      };
      return finish({
        kind: "done",
        done,
        stdout: stripSentinels(out.stdout, nonce),
        stderr: out.stderr,
      });
    }
    return finish({
      kind: "threw",
      error: `guest emitted an unknown result frame kind ${JSON.stringify(sentinel["k"])}`,
      stdout: stripSentinels(out.stdout, nonce),
      stderr: out.stderr,
    });
  }
}

export async function runScript(
  deps: ScriptDeps,
  input: ScriptInput,
  v: number,
  sessionId: string,
): Promise<ResponseEnvelope<ScriptResult | WriteReport>> {
  if (input.source === "") {
    throw new RangeError("script: source must be a non-empty string");
  }
  const budget = input.budget ?? SCRIPT_BUDGET_DEFAULT;
  // Caps clamp DOWNWARD only: a request narrows the sandbox, never widens it.
  const policy: ScriptGuestPolicy = {
    fuel: Math.min(input.fuel ?? SCRIPT_FUEL_DEFAULT, SCRIPT_FUEL_DEFAULT),
    memoryCapBytes: Math.min(
      input.memoryCapBytes ?? SCRIPT_MEMORY_CAP_DEFAULT,
      SCRIPT_MEMORY_CAP_DEFAULT,
    ),
  };

  const loop = await runGuestLoop(deps, input, policy, v);
  const { run } = loop;
  if (run.kind === "error") {
    return {
      ...run.envelope,
      notes: [
        ...(run.envelope.notes ?? []),
        `rounds ${loop.rounds}, host calls ${loop.hostCalls}, fuel used ${loop.fuelUsed}`,
      ],
    };
  }

  const scriptRow = (
    status: "ok" | "threw",
    extra: Partial<ScriptResult>,
    queued: number,
  ): ScriptResult => ({
    kind: "script",
    status,
    stdout: run.stdout,
    stderr: run.stderr,
    fuelUsed: loop.fuelUsed,
    rounds: loop.rounds,
    hostCalls: loop.hostCalls,
    queuedWrites: queued,
    ...extra,
  });

  if (run.kind === "threw") {
    return okEnvelope<ScriptResult | WriteReport>({
      v,
      freshness: "warm",
      budget: { requested: budget, used: 1 },
      results: [scriptRow("threw", { error: run.error }, 0)],
      notes: ["the script itself failed; nothing was queued or applied"],
    });
  }

  const { done } = run;
  if (done.threw !== null) {
    return okEnvelope<ScriptResult | WriteReport>({
      v,
      freshness: "warm",
      budget: { requested: budget, used: 1 },
      results: [scriptRow("threw", { error: done.threw }, done.edits.length + done.files.length)],
      notes: ["the script threw; its queued writes were discarded, nothing was applied"],
    });
  }

  const queuedCount = done.edits.length + done.files.length;
  if (queuedCount === 0) {
    return okEnvelope<ScriptResult | WriteReport>({
      v,
      freshness: "warm",
      budget: { requested: budget, used: 1 },
      results: [scriptRow("ok", { value: done.value }, 0)],
    });
  }

  const resolution = await resolveQueue(deps, done, v);
  if (!resolution.ok) return resolution.envelope;
  const { writes, beforeByPath, notes } = resolution;

  // A presented confirm is verified before the nothing-to-change answer —
  // a queue whose effects evaporated is exactly what a stale confirm looks
  // like (§7.7: fail closed). Contract identical to edit's.
  let confirmed = false;
  if (input.confirm !== undefined) {
    const current = aggregateHashes(writes, beforeByPath);
    const grant = deps.confirm.check(input.confirm);
    if (!grant.ok) {
      return taxonomyError({
        v,
        code: "CONFIRM_MISMATCH",
        message:
          "confirm: the token does not verify against this workspace's key — it was not " +
          "minted here, was corrupted in transit, or the key was rotated since; re-run " +
          "the call without confirm for a fresh diff and token",
      });
    }
    if (grant.contentHash !== current.contentHash) {
      return taxonomyError({
        v,
        code: "STALE_ANCHOR",
        message:
          "confirm: the confirmed files changed after the token was minted (§7.7 " +
          "fail-closed); re-run the call without confirm to review the fresh diff and " +
          "get a new token",
        contentHash: current.contentHash,
      });
    }
    if (grant.diffHash !== current.diffHash) {
      return taxonomyError({
        v,
        code: "CONFIRM_MISMATCH",
        message:
          "confirm: the token is authentic but seals a different diff — the script or " +
          "its queue changed since it was minted; re-run without confirm to get a " +
          "token for this exact call",
      });
    }
    confirmed = true;
  }

  if (writes.length === 0) {
    return okEnvelope<ScriptResult | WriteReport>({
      v,
      freshness: "warm",
      budget: { requested: budget, used: 1 },
      results: [scriptRow("ok", { value: done.value }, queuedCount)],
      notes: [...notes, "nothing to change: no file content differs; no write was attempted"],
    });
  }

  const dryRun = input.dryRun !== false;

  // §9.7: script writes are ALWAYS confirm-gated when applying — ad-hoc code
  // has no single reviewable intent, so the aggregated diff is the approval
  // surface (same posture as files.move).
  if (!dryRun && !confirmed && input.autoApply !== true) {
    const { diffHash, contentHash } = aggregateHashes(writes, beforeByPath);
    const valueJson = JSON.stringify(done.value ?? null);
    return {
      ...taxonomyError({
        v,
        code: "CONFIRM_REQUIRED",
        confirmToken: deps.confirm.mint(diffHash, contentHash),
        message:
          `this script queued writes to ${writes.length} file(s) and is confirm-gated ` +
          "(SDD §9.7/§9.4); review the aggregated diff in notes, then re-invoke the " +
          "identical call with confirm set to the confirmToken to apply (the script " +
          "re-runs deterministically; dryRun:true previews in full)",
      }),
      notes: [
        ...notes,
        `script return value: ${valueJson.length > SCRIPT_CONFIRM_VALUE_CHARS ? `${valueJson.slice(0, SCRIPT_CONFIRM_VALUE_CHARS)}…` : valueJson}`,
        ...confirmDiffNotes(writes, beforeByPath),
      ],
    };
  }

  let batch: ResponseEnvelope<WriteReport>;
  try {
    batch = await runWriteBatch(
      deps.write,
      {
        writes,
        dryRun,
        budget,
        origin: {
          tool: "script",
          argsHash: sha256(JSON.stringify({ source: input.source, dryRun: input.dryRun ?? true })),
          sessionId,
        },
      },
      v,
    );
  } catch (error) {
    // §7.1: a queued write to a protected or out-of-jail path is refused as
    // DATA, never a protocol throw — the same jailRefusal contract edit and
    // files pin over the served surface.
    const refusal = jailRefusal(v, error);
    if (refusal === null) throw error;
    batch = refusal;
  }
  if (!batch.ok) return batch;
  return {
    ...batch,
    results: [scriptRow("ok", { value: done.value }, queuedCount), ...batch.results],
    notes: [
      ...(batch.notes ?? []),
      ...notes,
      dryRun
        ? "dryRun: the queued writes were previewed, not applied (§9.7 default)"
        : "queued writes applied through the §7.5 write engine",
    ],
  };
}
