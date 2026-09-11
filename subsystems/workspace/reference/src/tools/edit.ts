/**
 * The `edit` tool (SDD §9.4) — replaces sed, awk and patch. Three verb
 * families resolve an intent into full-content WriteOps and hand them to the
 * §7.5 write-engine choke point (runWriteBatch), so blind byte-level
 * substitution is impossible by construction:
 *
 *   symbol verbs   replace_body | insert_after | insert_before | rename
 *   structural     rewrite (ast-grep pattern → rewritePattern, glob-scoped)
 *   anchored text  replace_text (literal old → new, occurrence-addressed)
 *
 * Confirm gate (§9.4): single-file symbol/text edits auto-apply — parse-
 * verify is the guard, and the applied diff is the receipt. rewrite and
 * rename are inherently multi-file, and any op touching more than one file
 * or more than EDIT_CONFIRM_LINE_LIMIT changed lines is confirm-gated:
 * without autoApply the answer is the typed CONFIRM_REQUIRED carrying the
 * aggregated dry-run diff in notes plus an HMAC-sealed confirmToken minted
 * through the ConfirmPort. Re-invoking the identical call with
 * confirm:token applies. Verification is stateless (the token embeds the
 * diff and aggregate content hashes it was sealed over): a bad seal answers
 * CONFIRM_MISMATCH, a valid seal over content that has since moved answers
 * STALE_ANCHOR with the fresh aggregate hash, and a valid seal whose diff
 * hash no longer matches (the arguments changed) answers CONFIRM_MISMATCH.
 * The protocol layer runs the same token flow inside one call for MRTR
 * clients by eliciting the confirmation mid-call.
 *
 * Staleness (§9.3/§7.7): read's contentHash arrives as expectedHash; every
 * file-targeted verb re-anchors the hash it resolved against onto its
 * WriteOps, so a file changed between resolution and apply answers
 * STALE_ANCHOR with the fresh hash. A replace_text whose `old` is absent
 * answers the same code: the anchor no longer describes the file.
 *
 * This layer sees only ports and the envelope contract (import-graph rule);
 * file access arrives through the write engine's jail-checked
 * WriteWorkspaceAccess, and a thrown jail refusal maps back to its typed
 * envelope rather than escaping as a crash.
 */

import {
  type ErrorEnvelope,
  type FileContent,
  okEnvelope,
  type ResponseEnvelope,
  taxonomyError,
  unifiedDiff,
} from "../envelope";
import type { IndexPort, IndexStatus, ReferenceEdge } from "../ports";
import { extentOf, resolveSymbol } from "./read";
import type { WorkspaceFile } from "./search";
import {
  runWriteBatch,
  type WriteEngineDeps,
  type WriteOp,
  type WriteReport,
} from "./write-engine";

export const EDIT_OPS = [
  "replace_body",
  "insert_after",
  "insert_before",
  "rename",
  "rewrite",
  "replace_text",
] as const;
export type EditOpKind = (typeof EDIT_OPS)[number];

/** §9.4: ops beyond this many changed lines are confirm-gated. */
export const EDIT_CONFIRM_LINE_LIMIT = 200;

/** Ceiling on the aggregated dry-run diff CONFIRM_REQUIRED carries in notes. */
export const EDIT_CONFIRM_DIFF_CHARS = 8000;

/**
 * The confirm-token seam (§9.4): mint seals (diffHash, contentHash) with the
 * per-workspace key; check authenticates a presented token and returns the
 * hashes it was sealed over. The key itself never enters the tool layer.
 */
export interface ConfirmPort {
  mint(diffHash: string, contentHash: string): string;
  check(token: string): { ok: true; diffHash: string; contentHash: string } | { ok: false };
}

export interface EditDeps {
  readonly write: WriteEngineDeps;
  readonly index: IndexPort;
  readonly confirm: ConfirmPort;
  /** Workspace file listing for rewrite's glob scope (search's own listing). */
  list(): Promise<readonly WorkspaceFile[]>;
}

/** Wire shape of one tool call, mirroring schemas/tools/edit.input.schema.json. */
export interface EditInput {
  readonly op: EditOpKind;
  readonly file?: string;
  readonly symbol?: string;
  readonly content?: string;
  readonly newName?: string;
  readonly pattern?: string;
  readonly rewritePattern?: string;
  readonly path?: string;
  readonly lang?: string;
  readonly old?: string;
  readonly new?: string;
  readonly occurrence?: number;
  readonly expectedHash?: string;
  readonly confirm?: string;
  readonly dryRun?: boolean;
  readonly autoApply?: boolean;
  readonly budget?: number;
}

const EDIT_BUDGET_DEFAULT = 4000;

/** Mirror of read.ts/write-engine.ts: the workspace-relative posix dialect. */
function normalizeRelPath(path: string): string {
  return path
    .replaceAll("\\", "/")
    .split("/")
    .filter((p) => p !== "" && p !== ".")
    .join("/");
}

function sha256(content: FileContent): string {
  return new Bun.CryptoHasher("sha256").update(content).digest("hex");
}

function isMissingFile(error: unknown): boolean {
  const code = (error as NodeJS.ErrnoException).code;
  return code === "ENOENT" || code === "ENOTDIR" || code === "EISDIR";
}

/** Jail refusals thrown by the composed WriteWorkspaceAccess (§7.1). */
export function jailRefusal(v: number, error: unknown): ErrorEnvelope | null {
  const code = (error as { code?: unknown }).code;
  if (
    code === "PATH_OUTSIDE_WORKSPACE" ||
    code === "PROTECTED_PATH" ||
    code === "PATTERN_INVALID"
  ) {
    return taxonomyError({
      v,
      code,
      message: error instanceof Error ? error.message : String(error),
    });
  }
  return null;
}

function fileNotFound(v: number, file: string): ErrorEnvelope {
  return taxonomyError({
    v,
    code: "FILE_NOT_FOUND",
    message:
      `no such file in the workspace: ${JSON.stringify(file)}; ` +
      "check the path (search mode:files locates files by name)",
  });
}

/** §9.4: symbol addressing strictly requires the index — refuse with progress. */
function indexUnavailable(v: number, status: IndexStatus | null): ErrorEnvelope {
  const progress =
    status === null
      ? "no index exists yet for this workspace"
      : status.building === true
        ? `index build in progress: ${status.filesIndexed ?? 0}/${status.filesTotal ?? 0} files`
        : "the Tier-1 index build has not completed";
  return taxonomyError({
    v,
    code: "INDEX_UNAVAILABLE",
    message:
      "edit with a symbol address requires the Tier-1 index and it is not up yet " +
      "(SDD §9.4, Constitution I); retry once the build completes, or use replace_text",
    notes: [progress],
  });
}

function staleAnchor(v: number, message: string, currentContent: string): ErrorEnvelope {
  return taxonomyError({
    v,
    code: "STALE_ANCHOR",
    message,
    contentHash: sha256(currentContent),
  });
}

/** Split into lines, remembering whether the content ended with a newline. */
function toLines(content: string): { lines: string[]; trailingNewline: boolean } {
  const lines = content.split("\n");
  const trailingNewline = lines[lines.length - 1] === "";
  if (trailingNewline) lines.pop();
  return { lines, trailingNewline };
}

function fromLines(lines: readonly string[], trailingNewline: boolean): string {
  return lines.join("\n") + (trailingNewline ? "\n" : "");
}

/** Lines a diff would apply: its `+`/`-` rows (headers excluded). */
export function changedLineCount(diff: string): number {
  let count = 0;
  for (const line of diff.split("\n")) {
    if (/^[+-]/.test(line) && !line.startsWith("+++") && !line.startsWith("---")) count += 1;
  }
  return count;
}

const IDENTIFIER = /^[A-Za-z_$][A-Za-z0-9_$]*$/;

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Extension families for rewrite's lang filter; unknown = literal extension. */
const LANG_EXTENSIONS: Readonly<Record<string, readonly string[]>> = {
  ts: ["ts", "tsx", "mts", "cts"],
  typescript: ["ts", "tsx", "mts", "cts"],
  js: ["js", "jsx", "mjs", "cjs"],
  javascript: ["js", "jsx", "mjs", "cjs"],
  py: ["py"],
  python: ["py"],
  rs: ["rs"],
  rust: ["rs"],
  go: ["go"],
  java: ["java"],
  c: ["c", "h"],
  cpp: ["cpp", "cc", "cxx", "hpp", "hh"],
  csharp: ["cs"],
  ruby: ["rb"],
  bash: ["sh", "bash"],
  json: ["json"],
};

function matchesLang(path: string, lang: string): boolean {
  const ext = path.slice(path.lastIndexOf(".") + 1).toLowerCase();
  const family = LANG_EXTENSIONS[lang.toLowerCase()];
  return family === undefined ? ext === lang.toLowerCase() : family.includes(ext);
}

/** One verb's resolution: the writes to run, plus how they were derived. */
interface ResolvedEdit {
  readonly writes: readonly WriteOp[];
  readonly notes: readonly string[];
  /** §9.4: rewrite/rename always gate; others gate on files/lines below. */
  readonly alwaysGated: boolean;
  /** Before-content per write path, from the resolution reads — the gate's
   * diff source, so no second filesystem pass races the anchor. */
  readonly beforeByPath: ReadonlyMap<string, string>;
}

type Resolved =
  | { readonly ok: true; readonly edit: ResolvedEdit }
  | { readonly ok: false; readonly envelope: ErrorEnvelope };

function refuse(envelope: ErrorEnvelope): Resolved {
  return { ok: false, envelope };
}

/** Read one target file's before-content and check the caller's anchor. */
async function readAnchored(
  deps: EditDeps,
  v: number,
  file: string,
  expectedHash: string | undefined,
): Promise<{ ok: true; content: string } | { ok: false; envelope: ErrorEnvelope }> {
  let content: string;
  try {
    content = await deps.write.workspace.read(file);
  } catch (error) {
    if (isMissingFile(error)) return { ok: false, envelope: fileNotFound(v, file) };
    const refusal = jailRefusal(v, error);
    if (refusal !== null) return { ok: false, envelope: refusal };
    throw error;
  }
  if (expectedHash !== undefined && sha256(content) !== expectedHash) {
    return {
      ok: false,
      envelope: staleAnchor(
        v,
        `${JSON.stringify(file)} changed since it was read (expectedHash mismatch); ` +
          "re-read the file and re-derive the edit against the returned contentHash",
        content,
      ),
    };
  }
  return { ok: true, content };
}

async function resolveSymbolVerb(deps: EditDeps, input: EditInput, v: number): Promise<Resolved> {
  const file = normalizeRelPath(input.file ?? "");
  const anchored = await readAnchored(deps, v, file, input.expectedHash);
  if (!anchored.ok) return refuse(anchored.envelope);
  const before = anchored.content;

  let status: IndexStatus | null;
  try {
    status = await deps.index.status();
  } catch {
    status = null;
  }
  if (status === null || status.tier === 0) return refuse(indexUnavailable(v, status));
  let outline: Awaited<ReturnType<IndexPort["queryOutline"]>>;
  try {
    outline = await deps.index.queryOutline({ paths: [file] });
  } catch {
    return refuse(indexUnavailable(v, status));
  }
  const rows = outline.files.find((f) => f.path === file)?.symbols ?? [];
  const resolution = resolveSymbol(v, rows, input.symbol ?? "", file);
  if (!resolution.ok) return refuse(resolution.envelope);
  const resolved = resolution.symbol;

  const { lines, trailingNewline } = toLines(before);
  const extent = extentOf(resolved, rows, lines.length);
  const expectedHashOut = input.expectedHash ?? sha256(before);

  if (input.op === "rename") {
    return resolveRename(deps, input, v, file, resolved.name, expectedHashOut, before);
  }

  const body = toLines(input.content ?? "").lines;
  let next: string[];
  if (input.op === "replace_body") {
    next = [...lines.slice(0, extent.start - 1), ...body, ...lines.slice(extent.end)];
  } else if (input.op === "insert_after") {
    next = [...lines.slice(0, extent.end), ...body, ...lines.slice(extent.end)];
  } else {
    next = [...lines.slice(0, extent.start - 1), ...body, ...lines.slice(extent.start - 1)];
  }
  return {
    ok: true,
    edit: {
      writes: [
        { path: file, content: fromLines(next, trailingNewline), expectedHash: expectedHashOut },
      ],
      notes: [
        `${input.op}: resolved ${JSON.stringify(resolved.name)} (${resolved.kind}) ` +
          `to lines ${extent.start}–${extent.end} of ${file}`,
      ],
      alwaysGated: false,
      beforeByPath: new Map([[file, before]]),
    },
  };
}

/**
 * rename: the definition file plus every file the index knows a reference
 * edge in, word-boundary identifier replacement per file. Name-resolution-
 * free by design (§14): the notes carry the confidence provenance instead of
 * overclaiming, and parse-verify guards every touched file.
 */
async function resolveRename(
  deps: EditDeps,
  input: EditInput,
  v: number,
  file: string,
  oldName: string,
  definitionHash: string,
  definitionBefore: string,
): Promise<Resolved> {
  const newName = input.newName ?? "";
  if (!IDENTIFIER.test(newName)) {
    return refuse(
      taxonomyError({
        v,
        code: "PATTERN_INVALID",
        message: `rename: newName ${JSON.stringify(newName)} is not a valid identifier`,
      }),
    );
  }
  const member = oldName.split(".").pop() ?? oldName;
  let edges: readonly ReferenceEdge[];
  try {
    edges = (await deps.index.queryReferences({ name: member })).edges;
  } catch {
    return refuse(indexUnavailable(v, await deps.index.status().catch(() => null)));
  }
  const paths = new Set<string>([file]);
  const confidence = { high: 0, medium: 0, low: 0 };
  for (const edge of edges) {
    paths.add(normalizeRelPath(edge.location.path));
    confidence[edge.confidence] += 1;
  }

  const pattern = new RegExp(`\\b${escapeRegExp(member)}\\b`, "g");
  const writes: WriteOp[] = [];
  const notes: string[] = [];
  const beforeByPath = new Map<string, string>();
  for (const path of [...paths].sort()) {
    let before: string;
    let expectedHash: string;
    if (path === file) {
      before = definitionBefore;
      expectedHash = definitionHash;
    } else {
      const anchored = await readAnchored(deps, v, path, undefined);
      if (!anchored.ok) {
        // A reference file the index knew but the jail or filesystem now
        // refuses: skip it honestly rather than failing the whole rename.
        notes.push(`rename: skipped ${path} (${anchored.envelope.error.code})`);
        continue;
      }
      before = anchored.content;
      expectedHash = sha256(before);
    }
    const after = before.replace(pattern, newName);
    if (after === before) continue;
    beforeByPath.set(path, before);
    writes.push({ path, content: after, expectedHash });
  }
  notes.unshift(
    `rename ${JSON.stringify(member)} → ${JSON.stringify(newName)}: ` +
      `${writes.length} file(s) from the definition plus index reference edges ` +
      `(confidence high:${confidence.high} medium:${confidence.medium} low:${confidence.low}); ` +
      "word-boundary matches, name-resolution-free — strings and comments included",
  );
  return { ok: true, edit: { writes, notes, alwaysGated: true, beforeByPath } };
}

async function resolveRewrite(deps: EditDeps, input: EditInput, v: number): Promise<Resolved> {
  const pattern = input.pattern ?? "";
  const replacement = input.rewritePattern ?? "";
  const glob = input.path === undefined ? null : new Bun.Glob(input.path);
  const files = await deps.list();
  const writes: WriteOp[] = [];
  const notes: string[] = [];
  const beforeByPath = new Map<string, string>();
  let considered = 0;
  for (const entry of files) {
    if (entry.junk) continue;
    if (glob !== null && !glob.match(entry.path)) continue;
    if (input.lang !== undefined && !matchesLang(entry.path, input.lang)) continue;
    considered += 1;
    let before: string;
    try {
      before = await deps.write.workspace.read(entry.path);
    } catch (error) {
      if (isMissingFile(error) || jailRefusal(v, error) !== null) continue;
      throw error;
    }
    let result: { content: string; changed: boolean };
    try {
      result = await deps.write.engine.rewrite({
        path: entry.path,
        content: before,
        edits: [{ find: pattern, replace: replacement }],
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const code = (error as { code?: unknown }).code;
      if (code === "INPUT_QUARANTINED") {
        return refuse(taxonomyError({ v, code: "INPUT_QUARANTINED", message }));
      }
      return refuse(taxonomyError({ v, code: "ENGINE_UNAVAILABLE", message }));
    }
    if (!result.changed) continue;
    beforeByPath.set(entry.path, before);
    writes.push({ path: entry.path, content: result.content, expectedHash: sha256(before) });
  }
  notes.push(
    `rewrite: pattern matched in ${writes.length} of ${considered} candidate file(s)` +
      `${input.path === undefined ? "" : ` under ${input.path}`}`,
  );
  return { ok: true, edit: { writes, notes, alwaysGated: true, beforeByPath } };
}

async function resolveReplaceText(deps: EditDeps, input: EditInput, v: number): Promise<Resolved> {
  const file = normalizeRelPath(input.file ?? "");
  const anchored = await readAnchored(deps, v, file, input.expectedHash);
  if (!anchored.ok) return refuse(anchored.envelope);
  const before = anchored.content;
  const old = input.old ?? "";
  const replacement = input.new ?? "";

  const indices: number[] = [];
  for (let at = before.indexOf(old); at >= 0; at = before.indexOf(old, at + old.length)) {
    indices.push(at);
  }
  if (indices.length === 0) {
    return refuse(
      staleAnchor(
        v,
        `replace_text: the anchor text was not found in ${JSON.stringify(file)}; ` +
          "the file may have changed since it was read — re-read it and re-derive " +
          "the edit against the returned contentHash",
        before,
      ),
    );
  }
  if (input.occurrence !== undefined && input.occurrence > indices.length) {
    return refuse(
      staleAnchor(
        v,
        `replace_text: occurrence ${input.occurrence} was requested but the anchor ` +
          `text appears ${indices.length} time(s) in ${JSON.stringify(file)}; ` +
          "re-read the file and re-derive the edit against the returned contentHash",
        before,
      ),
    );
  }

  let after: string;
  let note: string;
  if (input.occurrence === undefined) {
    after = before.replaceAll(old, replacement);
    note = `replace_text: replaced all ${indices.length} occurrence(s) in ${file}`;
  } else {
    const at = indices[input.occurrence - 1] as number;
    after = before.slice(0, at) + replacement + before.slice(at + old.length);
    note = `replace_text: replaced occurrence ${input.occurrence} of ${indices.length} in ${file}`;
  }
  return {
    ok: true,
    edit: {
      writes: [{ path: file, content: after, expectedHash: input.expectedHash ?? sha256(before) }],
      notes: [note],
      alwaysGated: false,
      beforeByPath: new Map([[file, before]]),
    },
  };
}

/**
 * The (diffHash, contentHash) pair a confirm token seals (§9.4), derived
 * deterministically from the resolved writes: paths in codepoint order,
 * before-content hashed per file, per-file diffs concatenated in the same
 * order. Re-resolving the identical call over unchanged files reproduces
 * both hashes exactly — that reproduction is what a confirm verifies.
 */
export function aggregateHashes(
  writes: readonly WriteOp[],
  beforeByPath: ReadonlyMap<string, FileContent>,
): { diffHash: string; contentHash: string } {
  const sorted = [...writes].sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  const content = sorted
    .map((write) => `${write.path}\n${sha256(beforeByPath.get(write.path) ?? "")}`)
    .join("\n");
  const diffs = sorted
    .map((write) =>
      unifiedDiff({
        path: write.path,
        before: beforeByPath.get(write.path) ?? null,
        after: write.content,
      }),
    )
    .join("\n");
  return { diffHash: sha256(diffs), contentHash: sha256(content) };
}

/** The aggregated dry-run diff CONFIRM_REQUIRED carries, capped honestly. */
export function confirmDiffNotes(
  writes: readonly WriteOp[],
  beforeByPath: ReadonlyMap<string, FileContent>,
): string[] {
  const notes: string[] = [];
  let spent = 0;
  for (const write of writes) {
    const diff = unifiedDiff({
      path: write.path,
      before: beforeByPath.get(write.path) ?? null,
      after: write.content,
    });
    if (spent + diff.length > EDIT_CONFIRM_DIFF_CHARS) {
      notes.push(
        `…aggregated diff truncated at ${EDIT_CONFIRM_DIFF_CHARS} chars ` +
          `(${writes.length} file(s) total); dryRun:true returns every diff in full`,
      );
      break;
    }
    spent += diff.length;
    notes.push(diff);
  }
  return notes;
}

/**
 * Answer one edit call with a ResponseEnvelope. The caller owns the jail
 * decision on composition, negotiates the envelope version, and seals
 * (redacts) the result before it reaches a wire. Throws RangeError on
 * schema-level misuse; returns taxonomy errors for everything recoverable.
 */
export async function runEdit(
  deps: EditDeps,
  input: EditInput,
  v: number,
  sessionId = "",
): Promise<ResponseEnvelope<WriteReport>> {
  if (!(EDIT_OPS as readonly string[]).includes(input.op)) {
    throw new RangeError(`edit: unknown op ${JSON.stringify(input.op)}`);
  }
  const symbolVerb =
    input.op === "replace_body" ||
    input.op === "insert_after" ||
    input.op === "insert_before" ||
    input.op === "rename";
  if (symbolVerb || input.op === "replace_text") {
    if ((input.file ?? "") === "") throw new RangeError(`edit: ${input.op} requires file`);
  }
  if (symbolVerb && (input.symbol ?? "") === "") {
    throw new RangeError(`edit: ${input.op} requires symbol`);
  }
  if (
    (input.op === "replace_body" || input.op === "insert_after" || input.op === "insert_before") &&
    input.content === undefined
  ) {
    throw new RangeError(`edit: ${input.op} requires content`);
  }
  if (
    input.op === "rewrite" &&
    ((input.pattern ?? "") === "" || input.rewritePattern === undefined)
  ) {
    throw new RangeError("edit: rewrite requires pattern and rewritePattern");
  }
  if (input.op === "replace_text" && ((input.old ?? "") === "" || input.new === undefined)) {
    throw new RangeError("edit: replace_text requires old and new");
  }

  const resolved = symbolVerb
    ? await resolveSymbolVerb(deps, input, v)
    : input.op === "rewrite"
      ? await resolveRewrite(deps, input, v)
      : await resolveReplaceText(deps, input, v);
  if (!resolved.ok) return resolved.envelope;
  const { writes, notes, alwaysGated, beforeByPath } = resolved.edit;

  const budget = input.budget ?? EDIT_BUDGET_DEFAULT;

  // A presented confirm is verified before anything else — even before the
  // nothing-to-change answer, because a rewrite whose matches evaporated is
  // exactly what a stale confirm looks like (§7.7: fail closed).
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
          "confirm: the token is authentic but seals a different diff — the call's " +
          "arguments changed since it was minted; re-run without confirm to get a " +
          "token for this exact call",
      });
    }
    confirmed = true;
  }

  if (writes.length === 0) {
    return okEnvelope<WriteReport>({
      v,
      freshness: "warm",
      budget: { requested: budget, used: 1 },
      results: [],
      notes: [...notes, "nothing to change: no file content differs; no write was attempted"],
    });
  }

  let changedLines = 0;
  for (const write of writes) {
    changedLines += changedLineCount(
      unifiedDiff({
        path: write.path,
        before: beforeByPath.get(write.path) ?? null,
        after: write.content,
      }),
    );
  }
  const gated = alwaysGated || writes.length > 1 || changedLines > EDIT_CONFIRM_LINE_LIMIT;
  const dryRun = input.dryRun === true;

  if (gated && !dryRun && !confirmed && input.autoApply !== true) {
    const { diffHash, contentHash } = aggregateHashes(writes, beforeByPath);
    return {
      ...taxonomyError({
        v,
        code: "CONFIRM_REQUIRED",
        confirmToken: deps.confirm.mint(diffHash, contentHash),
        message:
          `this ${input.op} touches ${writes.length} file(s) / ${changedLines} changed line(s) ` +
          "and is confirm-gated (SDD §9.4); review the aggregated diff in notes, then " +
          "re-invoke the identical call with confirm set to the confirmToken to apply " +
          "(dryRun:true previews in full; autoApply:true is the trusted-harness bypass)",
      }),
      notes: [...notes, ...confirmDiffNotes(writes, beforeByPath)],
    };
  }

  const argsHash = sha256(JSON.stringify(input));
  let envelope: ResponseEnvelope<WriteReport>;
  try {
    envelope = await runWriteBatch(
      deps.write,
      {
        writes,
        dryRun,
        budget,
        origin: { tool: "edit", argsHash, sessionId },
      },
      v,
    );
  } catch (error) {
    const refusal = jailRefusal(v, error);
    if (refusal === null) throw error;
    envelope = refusal;
  }
  if (notes.length === 0) return envelope;
  return { ...envelope, notes: [...notes, ...(envelope.notes ?? [])] };
}
