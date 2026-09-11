/**
 * The `files` tool (SDD §9.5) — replaces mv, cp, mkdir, touch and rm. Five
 * typed verbs resolve an intent into full-content WriteOps and hand them to
 * the §7.5 write-engine choke point (runWriteBatch):
 *
 *   move    import-aware relocation: importer files whose import specifiers
 *           resolve to the moved file are rewritten to its new location, and
 *           the moved file's own relative specifiers are recomputed from its
 *           new directory. Inherently multi-file — always confirm-gated.
 *   copy    duplicate `from` at `to`; overwriting a differing target gates.
 *   create  touch: create an empty file; an existing file is left untouched.
 *   delete  WriteOp{content:null} through the choke point — the composed
 *           workspace access renames into .workbench/trash/ (§7.6: nothing is
 *           ever unlinked destructively), and undo restores from the journal.
 *   mkdir   materialize the directory as a journaled `<to>/.gitkeep`: the
 *           journal's file model has no directory entity, and a bare mkdir
 *           would be an unjournaled mutation.
 *
 * Import rewriting is specifier-based and name-resolution-free (§14): a
 * specifier is rewritten only when it resolves — by the language family's
 * path convention — to exactly the moved file, and the notes carry what was
 * rewritten. Covered families: typescript/javascript (relative specifiers,
 * json imports included), python (absolute dotted modules), java (imports
 * whose dotted path suffix-matches the file, static imports included), and
 * c/c++ (quoted #include, importer-relative or root-relative). Rust names
 * MODULES, not files, so it has its own plan (rust-move.ts, specs/010): the
 * declaring `mod` item, the crate's `crate::`/`super::`/`self::`/uniform
 * paths that resolve to the moved module, and Cargo.toml target `path`
 * entries are rewritten, and what is deliberately left alone is named in
 * the notes. Go is deliberately not rewritten: its imports name packages,
 * and a file-level rewrite would overclaim.
 *
 * Confirm gate, staleness anchors, and jail mapping follow edit (§9.4)
 * exactly; the shared helpers are imported from there.
 */

import {
  contentEqual,
  type ErrorEnvelope,
  type FileContent,
  okEnvelope,
  type ResponseEnvelope,
  taxonomyError,
  unifiedDiff,
} from "../envelope";
import {
  aggregateHashes,
  type ConfirmPort,
  changedLineCount,
  confirmDiffNotes,
  EDIT_CONFIRM_LINE_LIMIT,
} from "./edit";
import { planRustMove } from "./rust-move";
import type { WorkspaceFile } from "./search";
import {
  readWorkspaceContent,
  runWriteBatch,
  type WriteEngineDeps,
  type WriteOp,
  type WriteReport,
} from "./write-engine";

export const FILES_OPS = ["move", "copy", "create", "delete", "mkdir"] as const;
export type FilesOpKind = (typeof FILES_OPS)[number];

export interface FilesDeps {
  readonly write: WriteEngineDeps;
  readonly confirm: ConfirmPort;
  /** Workspace file listing (search's own) — move's importer-scan scope. */
  list(): Promise<readonly WorkspaceFile[]>;
}

/** Wire shape of one tool call, mirroring schemas/tools/files.input.schema.json. */
export interface FilesInput {
  readonly op: FilesOpKind;
  readonly from?: string;
  readonly to?: string;
  readonly expectedHash?: string;
  readonly confirm?: string;
  readonly dryRun?: boolean;
  readonly autoApply?: boolean;
  readonly budget?: number;
}

const FILES_BUDGET_DEFAULT = 4000;

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
function jailRefusal(v: number, error: unknown): ErrorEnvelope | null {
  const code = (error as { code?: unknown }).code;
  if (code === "PATH_OUTSIDE_WORKSPACE" || code === "PROTECTED_PATH") {
    return taxonomyError({
      v,
      code,
      message: error instanceof Error ? error.message : String(error),
    });
  }
  return null;
}

function extOf(path: string): string {
  const dot = path.lastIndexOf(".");
  return dot < 0 ? "" : path.slice(dot + 1).toLowerCase();
}

function dirOf(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash < 0 ? "" : path.slice(0, slash);
}

/** Posix-relative path from a workspace-relative dir to a workspace-relative file. */
function relativePosix(fromDir: string, toPath: string): string {
  const a = fromDir === "" ? [] : fromDir.split("/");
  const b = toPath.split("/");
  let common = 0;
  while (common < a.length && a[common] === b[common]) common += 1;
  const ups = Array.from({ length: a.length - common }, () => "..");
  return [...ups, ...b.slice(common)].join("/");
}

const JS_EXTS = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"] as const;
const C_EXTS = ["c", "h", "cpp", "cc", "cxx", "hpp", "hh"] as const;

/** Which importer files to scan for specifiers, per moved-file extension. */
function importerExtensions(movedExt: string): readonly string[] {
  if ((JS_EXTS as readonly string[]).includes(movedExt) || movedExt === "json") return JS_EXTS;
  if (movedExt === "py") return ["py"];
  if (movedExt === "java") return ["java"];
  if ((C_EXTS as readonly string[]).includes(movedExt)) return C_EXTS;
  return [];
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Replace capture group 2 of every `pattern` match via `next`; count changes. */
function rewriteSpecifiers(
  content: string,
  pattern: RegExp,
  next: (specifier: string) => string | null,
): { content: string; count: number } {
  let count = 0;
  const rewritten = content.replace(pattern, (whole, head: string, spec: string, tail: string) => {
    const replacement = next(spec);
    if (replacement === null || replacement === spec) return whole;
    count += 1;
    return `${head}${replacement}${tail}`;
  });
  return { content: rewritten, count };
}

/** import/export-from, bare import, dynamic import() and require() specifiers. */
const JS_SPECIFIER_PATTERNS = [
  /((?:^|\n)[^\n"'`]*?(?:import|export)\b[^\n"'`]*?\bfrom\s*["'])([^"'\n]+)(["'])/g,
  /((?:^|\n)\s*import\s*["'])([^"'\n]+)(["'])/g,
  /(\bimport\s*\(\s*["'])([^"'\n]+)(["']\s*\))/g,
  /(\brequire\s*\(\s*["'])([^"'\n]+)(["']\s*\))/g,
];

/** Resolve a ts/js relative specifier from `fromDir` to a workspace path. */
function resolveJsSpecifier(
  fromDir: string,
  specifier: string,
  files: ReadonlySet<string>,
): string | null {
  if (!specifier.startsWith(".")) return null;
  const base = normalizeRelPath(joinRel(fromDir, specifier));
  if (files.has(base)) return base;
  for (const ext of JS_EXTS) {
    if (files.has(`${base}.${ext}`)) return `${base}.${ext}`;
    if (files.has(`${base}/index.${ext}`)) return `${base}/index.${ext}`;
  }
  return null;
}

/** Join a workspace-relative dir with a ./ or ../ specifier, resolving dots. */
function joinRel(fromDir: string, specifier: string): string {
  const parts = fromDir === "" ? [] : fromDir.split("/");
  for (const part of specifier.split("/")) {
    if (part === "" || part === ".") continue;
    if (part === "..") parts.pop();
    else parts.push(part);
  }
  return parts.join("/");
}

/** The ts/js specifier for `target` seen from `fromDir`, in `like`'s style. */
function jsSpecifierFor(fromDir: string, target: string, like: string): string {
  const explicitExt = /\.[a-z0-9]+$/i.test(like);
  let path = relativePosix(fromDir, target);
  if (!explicitExt) {
    const ext = extOf(target);
    if ((JS_EXTS as readonly string[]).includes(ext)) {
      path = path.slice(0, -(ext.length + 1));
    }
  }
  return path.startsWith(".") ? path : `./${path}`;
}

/** Rewrite ts/js specifiers in one importer that resolve to `from` → `to`. */
function rewriteJsImporter(
  importerPath: string,
  content: string,
  from: string,
  to: string,
  files: ReadonlySet<string>,
): { content: string; count: number } {
  const fromDir = dirOf(importerPath);
  let total = 0;
  let current = content;
  for (const pattern of JS_SPECIFIER_PATTERNS) {
    const step = rewriteSpecifiers(current, pattern, (spec) =>
      resolveJsSpecifier(fromDir, spec, files) === from ? jsSpecifierFor(fromDir, to, spec) : null,
    );
    current = step.content;
    total += step.count;
  }
  return { content: current, count: total };
}

/** Recompute the moved ts/js file's own relative specifiers from its new dir. */
function rewriteJsMovedContent(
  from: string,
  to: string,
  content: string,
  files: ReadonlySet<string>,
): { content: string; count: number } {
  const oldDir = dirOf(from);
  const newDir = dirOf(to);
  if (oldDir === newDir) return { content, count: 0 };
  let total = 0;
  let current = content;
  for (const pattern of JS_SPECIFIER_PATTERNS) {
    const step = rewriteSpecifiers(current, pattern, (spec) => {
      const target = resolveJsSpecifier(oldDir, spec, files);
      // A self-import cannot survive the move; importer rewriting owns `from`.
      if (target === null || target === from) return null;
      return jsSpecifierFor(newDir, target, spec);
    });
    current = step.content;
    total += step.count;
  }
  return { content: current, count: total };
}

/** The absolute dotted module a python file answers to (a/b.py → a.b). */
function pythonModuleOf(path: string): string | null {
  if (extOf(path) !== "py") return null;
  const stem = path.slice(0, -3);
  const base = stem.endsWith("/__init__") ? stem.slice(0, -"/__init__".length) : stem;
  return base.replaceAll("/", ".");
}

/** Rewrite absolute `import a.b` / `from a.b import x` lines in one importer. */
function rewritePythonImporter(
  content: string,
  from: string,
  to: string,
): { content: string; count: number } {
  const oldModule = pythonModuleOf(from);
  const newModule = pythonModuleOf(to);
  if (oldModule === null || newModule === null) return { content, count: 0 };
  let count = 0;
  const pattern = new RegExp(
    `(^\\s*(?:from|import)\\s[^#\\n]*?)(?<![\\w.])${escapeRegExp(oldModule)}(?![\\w.])`,
    "gm",
  );
  const rewritten = content.replace(pattern, (_whole, head: string) => {
    count += 1;
    return `${head}${newModule}`;
  });
  return { content: rewritten, count };
}

/** Rewrite `import a.b.C;` (static included) lines in one java importer. */
function rewriteJavaImporter(
  content: string,
  from: string,
  to: string,
): { content: string; count: number } {
  const fromDotted = from.slice(0, -".java".length).replaceAll("/", ".");
  const toDotted = to.endsWith(".java") ? to.slice(0, -".java".length).replaceAll("/", ".") : null;
  if (toDotted === null) return { content, count: 0 };
  let count = 0;
  const pattern = /(^\s*import\s+(?:static\s+)?)([\w.]+)(\s*;)/gm;
  const rewritten = content.replace(pattern, (whole, head: string, spec: string, tail: string) => {
    // The import's dotted path must suffix-match the file's path at a path
    // boundary — java roots (src/main/java/…) sit above the package tree.
    const specAsPath = spec.replaceAll(".", "/");
    let matched: string | null = null;
    if (fromDotted === spec || fromDotted.endsWith(`.${spec}`)) matched = spec;
    else if (from === `${specAsPath}.java` || from.endsWith(`/${specAsPath}.java`)) matched = spec;
    else {
      // Static member import: `import static a.b.C.max;` — the class is the
      // specifier minus its last segment.
      const lastDot = spec.lastIndexOf(".");
      if (lastDot > 0) {
        const cls = spec.slice(0, lastDot);
        const clsAsPath = cls.replaceAll(".", "/");
        if (from === `${clsAsPath}.java` || from.endsWith(`/${clsAsPath}.java`)) {
          const root = from.slice(0, from.length - `${clsAsPath}.java`.length);
          if (!to.startsWith(root)) return whole;
          count += 1;
          const newCls = to.slice(root.length, -".java".length).replaceAll("/", ".");
          return `${head}${newCls}${spec.slice(lastDot)}${tail}`;
        }
      }
      return whole;
    }
    const root = from.slice(0, from.length - `${matched.replaceAll(".", "/")}.java`.length);
    if (!to.startsWith(root)) return whole;
    count += 1;
    return `${head}${to.slice(root.length, -".java".length).replaceAll("/", ".")}${tail}`;
  });
  return { content: rewritten, count };
}

const C_INCLUDE_PATTERN = /(^\s*#\s*include\s*")([^"\n]+)(")/gm;

/** Rewrite quoted #include lines in one c/c++ importer. */
function rewriteCImporter(
  importerPath: string,
  content: string,
  from: string,
  to: string,
): { content: string; count: number } {
  const fromDir = dirOf(importerPath);
  return rewriteSpecifiers(content, C_INCLUDE_PATTERN, (spec) => {
    // Importer-relative first (the compiler's quoted-include order), then the
    // root-relative include-path form.
    if (normalizeRelPath(joinRel(fromDir, spec)) === from) return relativePosix(fromDir, to);
    if (normalizeRelPath(spec) === from) return to;
    return null;
  });
}

/** Recompute the moved c/c++ file's own quoted includes from its new dir. */
function rewriteCMovedContent(
  from: string,
  to: string,
  content: string,
  files: ReadonlySet<string>,
): { content: string; count: number } {
  const oldDir = dirOf(from);
  const newDir = dirOf(to);
  if (oldDir === newDir) return { content, count: 0 };
  return rewriteSpecifiers(content, C_INCLUDE_PATTERN, (spec) => {
    const importerRelative = normalizeRelPath(joinRel(oldDir, spec));
    // Root-relative includes survive a move unchanged; only importer-relative
    // ones need recomputing, and only when they name a real workspace file.
    if (!files.has(importerRelative) || files.has(normalizeRelPath(spec))) return null;
    if (importerRelative === from) return null;
    return relativePosix(newDir, importerRelative);
  });
}

/** One importer file's rewrite for the move, per the moved file's family. */
function rewriteImporter(
  importerPath: string,
  content: string,
  from: string,
  to: string,
  files: ReadonlySet<string>,
): { content: string; count: number } {
  const movedExt = extOf(from);
  if ((JS_EXTS as readonly string[]).includes(movedExt) || movedExt === "json") {
    return rewriteJsImporter(importerPath, content, from, to, files);
  }
  if (movedExt === "py") return rewritePythonImporter(content, from, to);
  if (movedExt === "java") return rewriteJavaImporter(content, from, to);
  if ((C_EXTS as readonly string[]).includes(movedExt)) {
    return rewriteCImporter(importerPath, content, from, to);
  }
  return { content, count: 0 };
}

/** The moved file's own outgoing-import rewrite, per its family. */
function rewriteMovedContent(
  from: string,
  to: string,
  content: string,
  files: ReadonlySet<string>,
): { content: string; count: number } {
  const movedExt = extOf(from);
  if ((JS_EXTS as readonly string[]).includes(movedExt)) {
    return rewriteJsMovedContent(from, to, content, files);
  }
  if ((C_EXTS as readonly string[]).includes(movedExt)) {
    return rewriteCMovedContent(from, to, content, files);
  }
  // Python absolute modules and java imports do not change with the mover's
  // own location; their importer-side rewrites carry the move.
  return { content, count: 0 };
}

/** One verb's resolution: the writes to run, plus how they were derived. */
interface ResolvedFiles {
  readonly writes: readonly WriteOp[];
  readonly notes: readonly string[];
  readonly alwaysGated: boolean;
  readonly beforeByPath: ReadonlyMap<string, FileContent>;
}

type Resolved =
  | { readonly ok: true; readonly files: ResolvedFiles }
  | { readonly ok: false; readonly envelope: ErrorEnvelope }
  | { readonly ok: "noop"; readonly notes: readonly string[] };

function refuse(envelope: ErrorEnvelope): Resolved {
  return { ok: false, envelope };
}

function fileNotFound(v: number, file: string, op: string): ErrorEnvelope {
  return taxonomyError({
    v,
    code: "FILE_NOT_FOUND",
    message:
      `files.${op}: no such file in the workspace: ${JSON.stringify(file)}; ` +
      "check the path (search mode:files locates files by name) — directories " +
      "are not movable/copyable in v1, only files",
  });
}

function staleAnchor(v: number, message: string, currentContent: FileContent): ErrorEnvelope {
  return taxonomyError({
    v,
    code: "STALE_ANCHOR",
    message,
    contentHash: sha256(currentContent),
  });
}

/** Read `from`'s before-content and check the caller's staleness anchor. */
async function readSource(
  deps: FilesDeps,
  v: number,
  op: string,
  from: string,
  expectedHash: string | undefined,
): Promise<{ ok: true; content: FileContent } | { ok: false; envelope: ErrorEnvelope }> {
  let content: FileContent;
  try {
    content = await readWorkspaceContent(deps.write.workspace, from);
  } catch (error) {
    if (isMissingFile(error)) return { ok: false, envelope: fileNotFound(v, from, op) };
    const refusal = jailRefusal(v, error);
    if (refusal !== null) return { ok: false, envelope: refusal };
    throw error;
  }
  if (expectedHash !== undefined && sha256(content) !== expectedHash) {
    return {
      ok: false,
      envelope: staleAnchor(
        v,
        `${JSON.stringify(from)} changed since it was read (expectedHash mismatch); ` +
          "re-read the file and re-derive the operation against the returned contentHash",
        content,
      ),
    };
  }
  return { ok: true, content };
}

/** `to`'s current content when it exists; null when it does not. */
async function readTarget(
  deps: FilesDeps,
  v: number,
  to: string,
): Promise<{ ok: true; content: FileContent | null } | { ok: false; envelope: ErrorEnvelope }> {
  try {
    return { ok: true, content: await readWorkspaceContent(deps.write.workspace, to) };
  } catch (error) {
    if (isMissingFile(error)) return { ok: true, content: null };
    const refusal = jailRefusal(v, error);
    if (refusal !== null) return { ok: false, envelope: refusal };
    throw error;
  }
}

async function resolveMove(deps: FilesDeps, input: FilesInput, v: number): Promise<Resolved> {
  const from = normalizeRelPath(input.from ?? "");
  const to = normalizeRelPath(input.to ?? "");
  if (from === to) {
    return refuse(
      taxonomyError({
        v,
        code: "PATTERN_INVALID",
        message: `files.move: from and to name the same path ${JSON.stringify(from)}`,
      }),
    );
  }
  const source = await readSource(deps, v, "move", from, input.expectedHash);
  if (!source.ok) return refuse(source.envelope);
  const target = await readTarget(deps, v, to);
  if (!target.ok) return refuse(target.envelope);

  // Opaque files have no source imports. Avoid both text rewriting and the
  // workspace-wide importer scan, but keep the same gated write batch.
  const listed = typeof source.content === "string" ? await deps.list() : [];
  const files = new Set(listed.map((f) => f.path));
  const notes: string[] = [];

  // Rust (specs/010): module paths, not specifiers — a plan over the crate's
  // files that the generic loop below never sees (importerExtensions("rs")
  // stays empty, so every other family is untouched).
  const rust =
    extOf(from) === "rs" && typeof source.content === "string"
      ? await planRustMove({
          from,
          to,
          fromContent: source.content,
          files: listed,
          read: async (path) => {
            try {
              return await deps.write.workspace.read(path);
            } catch (error) {
              if (isMissingFile(error) || jailRefusal(v, error) !== null) return null;
              throw error;
            }
          },
        })
      : null;
  const moved =
    typeof source.content !== "string"
      ? { content: source.content, count: 0 }
      : rust === null
        ? rewriteMovedContent(from, to, source.content, files)
        : { content: rust.movedContent, count: rust.movedRewrites };
  const writes: WriteOp[] = [
    {
      path: to,
      content: moved.content,
      // §7.5 baseline: the source's content — relocating an already-broken
      // file reports its diagnostics instead of rejecting as a "new" error.
      baseline: source.content,
      ...(target.content === null ? {} : { expectedHash: sha256(target.content) }),
    },
    { path: from, content: null, expectedHash: input.expectedHash ?? sha256(source.content) },
  ];
  const beforeByPath = new Map<string, FileContent>([[from, source.content]]);
  if (target.content !== null) {
    beforeByPath.set(to, target.content);
    notes.push(`move: ${JSON.stringify(to)} already exists and will be overwritten`);
  }

  const scanExts = importerExtensions(extOf(from));
  let importersRewritten = 0;
  let specifiersRewritten = moved.count;
  for (const entry of listed) {
    if (entry.junk) continue;
    if (entry.path === from || entry.path === to) continue;
    if (!scanExts.includes(extOf(entry.path))) continue;
    let before: string;
    try {
      before = await deps.write.workspace.read(entry.path);
    } catch (error) {
      if (isMissingFile(error) || jailRefusal(v, error) !== null) continue;
      throw error;
    }
    const rewritten = rewriteImporter(entry.path, before, from, to, files);
    if (rewritten.count === 0) continue;
    importersRewritten += 1;
    specifiersRewritten += rewritten.count;
    beforeByPath.set(entry.path, before);
    writes.push({ path: entry.path, content: rewritten.content, expectedHash: sha256(before) });
  }
  if (rust !== null) {
    // Parent, importer and manifest writes ride the same batch; the notes say
    // exactly what was and was not rewritten (FR-007).
    for (const write of rust.writes) {
      if (write.path === from || write.path === to) continue;
      beforeByPath.set(write.path, write.before);
      writes.push({
        path: write.path,
        content: write.content,
        expectedHash: sha256(write.before),
      });
    }
    notes.unshift(`move ${JSON.stringify(from)} → ${JSON.stringify(to)}`, ...rust.notes);
    return { ok: true, files: { writes, notes, alwaysGated: true, beforeByPath } };
  }
  notes.unshift(
    `move ${JSON.stringify(from)} → ${JSON.stringify(to)}: rewrote ` +
      `${specifiersRewritten} import specifier(s) — ${moved.count} in the moved file, ` +
      `the rest across ${importersRewritten} importer file(s); specifier-based and ` +
      "name-resolution-free (live scan; go imports name packages, not files, " +
      "and are not rewritten)",
  );
  return { ok: true, files: { writes, notes, alwaysGated: true, beforeByPath } };
}

async function resolveCopy(deps: FilesDeps, input: FilesInput, v: number): Promise<Resolved> {
  const from = normalizeRelPath(input.from ?? "");
  const to = normalizeRelPath(input.to ?? "");
  if (from === to) {
    return refuse(
      taxonomyError({
        v,
        code: "PATTERN_INVALID",
        message: `files.copy: from and to name the same path ${JSON.stringify(from)}`,
      }),
    );
  }
  const source = await readSource(deps, v, "copy", from, input.expectedHash);
  if (!source.ok) return refuse(source.envelope);
  const target = await readTarget(deps, v, to);
  if (!target.ok) return refuse(target.envelope);
  if (contentEqual(target.content, source.content)) {
    return { ok: "noop", notes: [`copy: ${JSON.stringify(to)} already matches the source`] };
  }
  const overwriting = target.content !== null;
  const beforeByPath = new Map<string, FileContent>([[from, source.content]]);
  if (target.content !== null) beforeByPath.set(to, target.content);
  return {
    ok: true,
    files: {
      writes: [
        {
          path: to,
          content: source.content,
          baseline: source.content,
          ...(target.content === null ? {} : { expectedHash: sha256(target.content) }),
        },
      ],
      notes: [
        `copy ${JSON.stringify(from)} → ${JSON.stringify(to)}` +
          (overwriting ? " (overwrites the existing target)" : ""),
      ],
      alwaysGated: overwriting,
      beforeByPath,
    },
  };
}

async function resolveCreate(deps: FilesDeps, input: FilesInput, v: number): Promise<Resolved> {
  const to = normalizeRelPath(input.to ?? "");
  const target = await readTarget(deps, v, to);
  if (!target.ok) return refuse(target.envelope);
  if (target.content !== null) {
    return {
      ok: "noop",
      notes: [
        `create: ${JSON.stringify(to)} already exists and was left untouched ` +
          "(touch semantics; use edit to change its content)",
      ],
    };
  }
  return {
    ok: true,
    files: {
      writes: [{ path: to, content: "" }],
      notes: [`create: new empty file ${JSON.stringify(to)}`],
      alwaysGated: false,
      beforeByPath: new Map(),
    },
  };
}

async function resolveDelete(deps: FilesDeps, input: FilesInput, v: number): Promise<Resolved> {
  const from = normalizeRelPath(input.from ?? "");
  const source = await readSource(deps, v, "delete", from, input.expectedHash);
  if (!source.ok) return refuse(source.envelope);
  return {
    ok: true,
    files: {
      writes: [
        { path: from, content: null, expectedHash: input.expectedHash ?? sha256(source.content) },
      ],
      notes: [
        `delete ${JSON.stringify(from)}: journal-backed move to .workbench/trash/ — ` +
          "recoverable with undo",
      ],
      alwaysGated: false,
      beforeByPath: new Map([[from, source.content]]),
    },
  };
}

async function resolveMkdir(deps: FilesDeps, input: FilesInput, v: number): Promise<Resolved> {
  const to = normalizeRelPath(input.to ?? "");
  const keep = `${to}/.gitkeep`;
  const target = await readTarget(deps, v, keep);
  if (!target.ok) return refuse(target.envelope);
  if (target.content !== null) {
    return { ok: "noop", notes: [`mkdir: ${JSON.stringify(to)} already exists`] };
  }
  return {
    ok: true,
    files: {
      writes: [{ path: keep, content: "" }],
      notes: [
        `mkdir: materialized ${JSON.stringify(to)} as a journaled ${JSON.stringify(keep)} — ` +
          "the journal's file model has no directory entity, so the marker file is " +
          "what makes the operation journaled and undoable",
      ],
      alwaysGated: false,
      beforeByPath: new Map(),
    },
  };
}

/**
 * Answer one files call with a ResponseEnvelope. The caller owns the jail
 * decision on composition, negotiates the envelope version, and seals
 * (redacts) the result before it reaches a wire. Throws RangeError on
 * schema-level misuse; returns taxonomy errors for everything recoverable.
 */
export async function runFiles(
  deps: FilesDeps,
  input: FilesInput,
  v: number,
  sessionId = "",
): Promise<ResponseEnvelope<WriteReport>> {
  if (!(FILES_OPS as readonly string[]).includes(input.op)) {
    throw new RangeError(`files: unknown op ${JSON.stringify(input.op)}`);
  }
  const needsFrom = input.op === "move" || input.op === "copy" || input.op === "delete";
  const needsTo = input.op !== "delete";
  if (needsFrom && normalizeRelPath(input.from ?? "") === "") {
    throw new RangeError(`files: ${input.op} requires from`);
  }
  if (needsTo && normalizeRelPath(input.to ?? "") === "") {
    throw new RangeError(`files: ${input.op} requires to`);
  }

  const resolved =
    input.op === "move"
      ? await resolveMove(deps, input, v)
      : input.op === "copy"
        ? await resolveCopy(deps, input, v)
        : input.op === "create"
          ? await resolveCreate(deps, input, v)
          : input.op === "delete"
            ? await resolveDelete(deps, input, v)
            : await resolveMkdir(deps, input, v);
  if (resolved.ok === false) return resolved.envelope;

  const budget = input.budget ?? FILES_BUDGET_DEFAULT;
  if (resolved.ok === "noop") {
    return okEnvelope<WriteReport>({
      v,
      freshness: "warm",
      budget: { requested: budget, used: 1 },
      results: [],
      notes: [...resolved.notes, "nothing to change: no write was attempted"],
    });
  }
  const { writes, notes, alwaysGated, beforeByPath } = resolved.files;

  // A presented confirm is verified before anything else (§7.7: fail closed) —
  // the same stateless token contract as edit, over the same aggregate hashes.
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
          `this files.${input.op} touches ${writes.length} file(s) / ${changedLines} changed ` +
          "line(s) and is confirm-gated (SDD §9.5); review the aggregated diff in notes, " +
          "then re-invoke the identical call with confirm set to the confirmToken to apply " +
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
        origin: { tool: "files", argsHash, sessionId },
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
