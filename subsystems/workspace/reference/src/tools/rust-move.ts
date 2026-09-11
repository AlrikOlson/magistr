/**
 * Rust-aware `files` moves (SDD §9.5, specs/010-rust-move-aware-files).
 *
 * Rust imports name MODULES, not files, so the specifier-based rewrite that
 * serves the other families (files.ts) cannot apply. What CAN be derived
 * deterministically is the module path of a `.rs` file — from its location
 * under a crate root by the file-module conventions (`src/lib.rs` /
 * `src/main.rs` / target roots, `<name>.rs`, `<name>/mod.rs`) — and that
 * derivation is CONFIRMED here by finding the actual `mod <name>;` item in
 * the parent module file (research R1). Given old and new module paths, the
 * rewrite is a contiguous-path substitution at segment boundaries:
 *
 *   - the declaring `mod` item is renamed in place, or removed from the old
 *     parent and added to the EXISTING new parent (R5 — nothing synthesized);
 *   - every `crate::`/`super::`/`self::`/uniform path in the crate that
 *     resolves to the moved module is rewritten, re-emitted in the same
 *     relative form when it still reaches, else `crate::`-absolute (R2/R3);
 *   - the moved file's own `super::` paths are re-rooted when its parent
 *     changes (R4);
 *   - `[lib]`/`[[bin]]`/`[[test]]`/`[[example]]`/`[[bench]]` `path` entries in
 *     the crate's Cargo.toml follow the file (R7; the write engine's TOML
 *     well-formedness check verifies the result).
 *
 * What is deliberately NOT rewritten is reported with file and line so the
 * agent can finish by hand (R2/R6/R8): brace-grouped `use` members that name
 * the moved module, non-leaf modules (files v1 moves single files, so child
 * modules would be orphaned), undiscoverable declarations, `#[path]`
 * declarations, and cross-crate `<crate_name>::` paths. Workspace members and
 * `path` dependencies are directory facts a single-file move cannot change,
 * so this module never touches them (FR-012).
 *
 * Pure: everything here works over a file listing and a read function; the
 * caller (files.ts resolveMove) turns the plan into WriteOps through the §7.5
 * choke point, confirm-gated like every other move.
 */

import type { WorkspaceFile } from "./search";

export interface RustMoveInput {
  /** Workspace-relative posix path of the moved file (before). */
  readonly from: string;
  /** Workspace-relative posix path of the moved file (after). */
  readonly to: string;
  /** The moved file's content before the move. */
  readonly fromContent: string;
  /** Workspace listing (search's own); junk entries are never scanned. */
  readonly files: readonly WorkspaceFile[];
  /** Read one workspace file; null when unreadable (missing, jailed). */
  readonly read: (path: string) => Promise<string | null>;
}

export interface RustDeclarationChange {
  readonly kind: "rename" | "move" | "remove-only";
  /** Files (workspace-relative) that held `mod old;` and were rewritten. */
  readonly from: readonly string[];
  /** The new parent file the declaration was added to; null when none exists. */
  readonly to: string | null;
}

export interface RustUnrewritten {
  readonly path: string;
  /** 1-based line. */
  readonly line: number;
  readonly reason: string;
}

export interface RustMoveWrite {
  readonly path: string;
  readonly before: string;
  readonly content: string;
}

export interface RustMovePlan {
  readonly crateDir: string;
  readonly oldModule: readonly string[] | null;
  readonly newModule: readonly string[] | null;
  readonly declaration: RustDeclarationChange | null;
  /** The moved file's content after its own rewrites (== fromContent when none). */
  readonly movedContent: string;
  readonly movedRewrites: number;
  /** Importer, parent and manifest writes — never the moved file itself. */
  readonly writes: readonly RustMoveWrite[];
  readonly pathRewrites: readonly { readonly path: string; readonly count: number }[];
  readonly manifest: { readonly path: string; readonly entries: number } | null;
  readonly unrewritten: readonly RustUnrewritten[];
  /** Set when no Rust rewrite was attempted; the move proceeds as a plain file move. */
  readonly skipped: { readonly reason: string } | null;
  /** Rendered notes for the envelope. */
  readonly notes: readonly string[];
}

// ---------------------------------------------------------------------------
// Paths

function dirOf(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash < 0 ? "" : path.slice(0, slash);
}

function baseOf(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash < 0 ? path : path.slice(slash + 1);
}

function joinPath(dir: string, name: string): string {
  return dir === "" ? name : `${dir}/${name}`;
}

function stemOf(path: string): string | null {
  const base = baseOf(path);
  return base.endsWith(".rs") ? base.slice(0, -3) : null;
}

function isUnder(path: string, dir: string): boolean {
  return dir === "" || path === dir || path.startsWith(`${dir}/`);
}

function relativeTo(path: string, dir: string): string {
  return dir === "" ? path : path.slice(dir.length + 1);
}

/** The directory whose `<dir>.rs` / `<dir>/mod.rs` (or root file) declares `rel`. */
function parentDirOf(rel: string): string {
  return stemOf(rel) === "mod" ? dirOf(dirOf(rel)) : dirOf(rel);
}

/** The nearest ancestor directory holding a Cargo.toml, else "" (workspace root). */
export function crateDirOf(path: string, files: ReadonlySet<string>): string {
  let dir = dirOf(path);
  for (;;) {
    if (files.has(joinPath(dir, "Cargo.toml"))) return dir;
    if (dir === "") return "";
    dir = dirOf(dir);
  }
}

// ---------------------------------------------------------------------------
// Manifest (R7): textual, section-split, no TOML parser — like map.cargoEntryPoints.

const TARGET_TABLES = ["[lib]", "[[bin]]", "[[test]]", "[[example]]", "[[bench]]"] as const;

function normalizeManifestPath(path: string): string {
  return path
    .replaceAll("\\", "/")
    .split("/")
    .filter((p) => p !== "" && p !== ".")
    .join("/");
}

function isTargetSection(section: string): boolean {
  const header = section.trimStart();
  return TARGET_TABLES.some((table) => header.startsWith(table));
}

/** Crate-relative `path = "..."` values under the target tables. */
export function manifestTargetPaths(manifest: string): readonly string[] {
  const out: string[] = [];
  for (const section of manifest.split(/^(?=[ \t]*\[)/m)) {
    if (!isTargetSection(section)) continue;
    const path = /^[ \t]*path[ \t]*=[ \t]*"([^"\n]+)"/m.exec(section)?.[1];
    if (path !== undefined) out.push(normalizeManifestPath(path));
  }
  return out;
}

/** Rewrite target `path` entries equal to `fromRel` to `toRel`; counts entries. */
export function rewriteManifestPaths(
  manifest: string,
  fromRel: string,
  toRel: string,
): { content: string; count: number } {
  let count = 0;
  const sections = manifest.split(/^(?=[ \t]*\[)/m).map((section) => {
    if (!isTargetSection(section)) return section;
    return section.replace(
      /^([ \t]*path[ \t]*=[ \t]*")([^"\n]+)(")/m,
      (whole, head: string, value: string, tail: string) => {
        if (normalizeManifestPath(value) !== fromRel) return whole;
        count += 1;
        return `${head}${toRel}${tail}`;
      },
    );
  });
  return { content: sections.join(""), count };
}

// ---------------------------------------------------------------------------
// Module tree (R1)

const AUTO_ROOT =
  /^(?:src\/(?:lib|main)\.rs|build\.rs|src\/bin\/[^/]+\.rs|src\/bin\/[^/]+\/main\.rs|(?:tests|examples|benches)\/[^/]+\.rs|(?:tests|examples|benches)\/[^/]+\/main\.rs)$/;

/** Is `rel` (crate-relative) a crate-root file — auto-discovered or a manifest target? */
export function isRootFile(rel: string, manifestTargets: ReadonlySet<string>): boolean {
  return AUTO_ROOT.test(rel) || manifestTargets.has(rel);
}

/** The crate's `.rs` files plus the root files grouped by directory — built once per move. */
export interface ModuleTree {
  readonly files: ReadonlySet<string>;
  readonly manifestTargets: ReadonlySet<string>;
  readonly rootsByDir: ReadonlyMap<string, readonly string[]>;
}

export function buildModuleTree(
  crateFiles: ReadonlySet<string>,
  manifestTargets: ReadonlySet<string>,
): ModuleTree {
  const rootsByDir = new Map<string, string[]>();
  for (const file of crateFiles) {
    if (!isRootFile(file, manifestTargets)) continue;
    const dir = dirOf(file);
    const list = rootsByDir.get(dir) ?? [];
    list.push(file);
    rootsByDir.set(dir, list);
  }
  return { files: crateFiles, manifestTargets, rootsByDir };
}

export interface ModuleLocation {
  /** Root files whose tree contains the file; several when lib.rs and main.rs share src/. */
  readonly roots: readonly string[];
  /** The directory the roots live in (crate-relative). */
  readonly rootDir: string;
  /** Module path segments below `crate`; [] for a root file. */
  readonly segments: readonly string[];
  /** Candidate files that declare the LAST segment (the file's parent module). */
  readonly parents: readonly string[];
}

/**
 * Locate a crate-relative `.rs` file in the module tree by convention:
 * `<dir>/<name>.rs` and `<dir>/<name>/mod.rs` are module `name` declared by
 * `<dir>.rs` / `<dir>/mod.rs`, or by the root file(s) when `<dir>` is the
 * root directory. Null when no chain to a root exists (orphan).
 */
export function locateModule(rel: string, tree: ModuleTree): ModuleLocation | null {
  const stem = stemOf(rel);
  if (stem === null) return null;
  if (isRootFile(rel, tree.manifestTargets)) {
    return { roots: [rel], rootDir: dirOf(rel), segments: [], parents: [] };
  }
  const segments: string[] = stem === "mod" ? [] : [stem];
  let dir = dirOf(rel);
  let parents: string[] | null = null;
  for (let guard = 0; guard < 64; guard += 1) {
    const rootsHere = (tree.rootsByDir.get(dir) ?? []).filter((r) => r !== rel);
    if (rootsHere.length > 0) {
      return { roots: rootsHere, rootDir: dir, segments, parents: parents ?? rootsHere };
    }
    if (dir === "") return null;
    const name = baseOf(dir);
    const candidates = [`${dir}.rs`, `${dir}/mod.rs`].filter((c) => c !== rel && tree.files.has(c));
    if (candidates.length === 0) {
      // `<dir>/mod.rs` IS this directory's module: ascend without a parent yet.
      if (`${dir}/mod.rs` === rel) {
        segments.unshift(name);
        dir = dirOf(dir);
        continue;
      }
      return null;
    }
    // `tests/foo.rs` beside `tests/foo/helper.rs`: the sibling file IS a root.
    const rootCandidates = candidates.filter((c) => isRootFile(c, tree.manifestTargets));
    if (rootCandidates.length > 0) {
      return {
        roots: rootCandidates,
        rootDir: dirOf(dir),
        segments,
        parents: parents ?? rootCandidates,
      };
    }
    parents ??= candidates;
    segments.unshift(name);
    dir = dirOf(dir);
  }
  return null;
}

// ---------------------------------------------------------------------------
// Declarations (R1/R5)

const VIS = String.raw`(?:pub(?:\([^)]*\))?[ \t]+)?`;
const IDENT = String.raw`[A-Za-z_]\w*`;

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Module names declared in this file — `mod x;` and inline `mod x {`. */
export function declaredModules(content: string): ReadonlySet<string> {
  const out = new Set<string>();
  const pattern = new RegExp(String.raw`^[ \t]*${VIS}mod[ \t]+(${IDENT})[ \t]*[;{]`, "gm");
  for (const m of content.matchAll(pattern)) out.add(m[1] as string);
  return out;
}

export interface DeclarationItem {
  /** Index of the first line of the item (attributes/docs included), 0-based. */
  readonly startLine: number;
  /** Index of the `mod name;` line, 0-based. */
  readonly declLine: number;
  readonly hasPathAttr: boolean;
  readonly isPub: boolean;
}

/** Find the `mod <name>;` item (file module, not inline) with its leading attributes/docs. */
export function findDeclaration(content: string, name: string): DeclarationItem | null {
  const lines = content.split("\n");
  const decl = new RegExp(String.raw`^[ \t]*(${VIS})mod[ \t]+${escapeRegExp(name)}[ \t]*;`);
  for (let i = 0; i < lines.length; i += 1) {
    const m = decl.exec(lines[i] as string);
    if (m === null) continue;
    let start = i;
    let hasPathAttr = false;
    while (start > 0) {
      const prev = (lines[start - 1] as string).trim();
      if (!(prev.startsWith("#[") || prev.startsWith("///"))) break;
      if (/^#\[\s*path\s*=/.test(prev)) hasPathAttr = true;
      start -= 1;
    }
    return { startLine: start, declLine: i, hasPathAttr, isPub: (m[1] ?? "").trim() !== "" };
  }
  return null;
}

/** Rename the `mod old;` item in place, keeping everything else on its lines. */
export function renameDeclaration(content: string, oldName: string, newName: string): string {
  const pattern = new RegExp(
    String.raw`^([ \t]*${VIS}mod[ \t]+)${escapeRegExp(oldName)}([ \t]*;)`,
    "m",
  );
  return content.replace(pattern, `$1${newName}$2`);
}

/** Remove the item's lines (attributes/docs included). */
export function removeDeclaration(content: string, item: DeclarationItem): string {
  const lines = content.split("\n");
  lines.splice(item.startLine, item.declLine + 1 - item.startLine);
  return lines.join("\n");
}

/** The item's own lines, flush-left, reusable in another parent. */
export function declarationItemText(content: string, item: DeclarationItem): string {
  return content
    .split("\n")
    .slice(item.startLine, item.declLine + 1)
    .map((l) => l.replace(/^[ \t]+/, ""))
    .join("\n");
}

/**
 * Insert a declaration item: after the last file-module `mod x;` line, else
 * after the leading inner-attribute / inner-doc block (which must stay first).
 */
export function insertDeclaration(content: string, item: string): string {
  const lines = content.split("\n");
  const modLine = new RegExp(String.raw`^[ \t]*${VIS}mod[ \t]+${IDENT}[ \t]*;`);
  let last = -1;
  for (let i = 0; i < lines.length; i += 1) if (modLine.test(lines[i] as string)) last = i;
  const itemLines = item.split("\n");
  if (last >= 0) {
    lines.splice(last + 1, 0, ...itemLines);
    return lines.join("\n");
  }
  let at = 0;
  while (at < lines.length) {
    const t = (lines[at] as string).trim();
    if (t.startsWith("//!") || t.startsWith("#![") || (t === "" && at > 0)) at += 1;
    else break;
  }
  if (at > 0 && (lines[at - 1] as string).trim() !== "") itemLines.unshift("");
  itemLines.push("");
  lines.splice(at, 0, ...itemLines);
  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// Path resolution and rewriting (R2/R3/R4)

type Segments = readonly string[];

function startsWithSegments(path: Segments, prefix: Segments): boolean {
  if (prefix.length > path.length) return false;
  for (let i = 0; i < prefix.length; i += 1) if (path[i] !== prefix[i]) return false;
  return true;
}

/** A `use` binding in one file: local name → absolute module path (same crate). */
interface Binding {
  readonly abs: Segments;
  readonly aliased: boolean;
  readonly viaGroup: boolean;
}

const USE_SINGLE = new RegExp(
  String.raw`\buse[ \t]+((?:crate|super|self|${IDENT})(?:::(?:super|self|${IDENT}))*)(?:[ \t]+as[ \t]+(${IDENT}))?[ \t]*;`,
  "g",
);
const USE_GROUP = new RegExp(
  String.raw`\buse[ \t]+((?:crate|super|self|${IDENT})(?:::(?:super|self|${IDENT}))*)::\{([^}]*)\}`,
  "g",
);
const CHAIN = new RegExp(
  String.raw`(?<![\w:])((?:crate|super|self)(?:::(?:super|self|${IDENT}))*|${IDENT}(?:::${IDENT})+)(?!\w)`,
  "g",
);

const NO_BINDINGS: ReadonlyMap<string, Binding> = new Map();

/** Resolve a chain against module `m` (null when the file's module is unknown). */
function resolveChain(
  chain: Segments,
  m: Segments | null,
  declared: ReadonlySet<string>,
  bindings: ReadonlyMap<string, Binding>,
): Segments | null {
  const head = chain[0];
  if (head === undefined) return null;
  if (head === "crate") return chain.slice(1).filter((s) => s !== "self");
  if (head === "self" || head === "super") {
    if (m === null) return null;
    const base = [...m];
    let i = 0;
    for (; i < chain.length; i += 1) {
      const tok = chain[i];
      if (tok === "super") {
        if (base.length === 0) return null;
        base.pop();
      } else if (tok !== "self") break;
    }
    return [...base, ...chain.slice(i)];
  }
  if (declared.has(head)) return m === null ? null : [...m, ...chain];
  const binding = bindings.get(head);
  if (binding !== undefined) return [...binding.abs, ...chain.slice(1)];
  return null;
}

/** Members of a brace group as segment lists (`self` and nested groups skipped). */
function groupMembers(group: string): { segs: Segments; alias: string | undefined }[] {
  const out: { segs: Segments; alias: string | undefined }[] = [];
  for (const raw of group.split(",")) {
    const [pathPart, alias] = raw.trim().split(/[ \t]+as[ \t]+/);
    const segs = (pathPart ?? "")
      .split("::")
      .map((s) => s.trim())
      .filter((s) => s !== "");
    if (segs.length === 0 || segs.some((s) => s.includes("{") || s === "*")) continue;
    out.push({ segs, alias });
  }
  return out;
}

function collectBindings(
  content: string,
  m: Segments | null,
  declared: ReadonlySet<string>,
): Map<string, Binding> {
  const out = new Map<string, Binding>();
  for (const match of content.matchAll(USE_SINGLE)) {
    const chain = (match[1] as string).split("::");
    const abs = resolveChain(chain, m, declared, NO_BINDINGS);
    if (abs === null || abs.length === 0) continue;
    const alias = match[2];
    out.set(alias ?? (chain[chain.length - 1] as string), {
      abs,
      aliased: alias !== undefined,
      viaGroup: false,
    });
  }
  for (const match of content.matchAll(USE_GROUP)) {
    const prefix = resolveChain((match[1] as string).split("::"), m, declared, NO_BINDINGS);
    if (prefix === null) continue;
    for (const { segs, alias } of groupMembers(match[2] as string)) {
      const isSelf = segs.length === 1 && segs[0] === "self";
      const abs = isSelf ? prefix : [...prefix, ...segs.filter((s) => s !== "self")];
      const name = alias ?? (isSelf ? prefix[prefix.length - 1] : segs[segs.length - 1]);
      if (name === undefined) continue;
      out.set(name, { abs, aliased: alias !== undefined, viaGroup: true });
    }
  }
  return out;
}

export interface RewriteContext {
  /** The file's module path for RESOLVING (null = unknown: only crate:: chains resolve). */
  readonly resolveModule: Segments | null;
  /** The file's module path for RE-EMITTING (differs from resolveModule only for the moved file). */
  readonly emitModule: Segments | null;
  /** Module names declared in this file BEFORE the move (resolution). */
  readonly declaredBefore: ReadonlySet<string>;
  /** Module names declared in this file AFTER the move (uniform re-emission). */
  readonly declaredAfter: ReadonlySet<string>;
  readonly oldModule: Segments;
  readonly newModule: Segments;
}

/** Re-emit `abs` from `emitModule` in the style of `chain`. */
function emitPath(
  chain: Segments,
  abs: Segments,
  ctx: RewriteContext,
  bindings: ReadonlyMap<string, Binding>,
): string {
  const head = chain[0] as string;
  const absolute = abs.length === 0 ? "crate" : `crate::${abs.join("::")}`;
  const m = ctx.emitModule;
  if (head === "crate" || m === null) return absolute;
  if (head === "self" || head === "super") {
    const base = [...m];
    const prefix: string[] = [];
    for (const tok of chain) {
      if (tok === "super") {
        if (base.length === 0) return absolute;
        base.pop();
        prefix.push("super");
      } else if (tok === "self") {
        prefix.push("self");
      } else break;
    }
    return startsWithSegments(abs, base)
      ? [...prefix, ...abs.slice(base.length)].join("::")
      : absolute;
  }
  if (ctx.declaredBefore.has(head)) {
    // Resolved through a module declared in this file: same style if the
    // first segment below this module is (still) declared here.
    const next = abs[m.length];
    if (startsWithSegments(abs, m) && next !== undefined && ctx.declaredAfter.has(next)) {
      return abs.slice(m.length).join("::");
    }
    return absolute;
  }
  const binding = bindings.get(head);
  if (binding !== undefined && !binding.viaGroup) {
    // The binding's own `use` line is rewritten by the chain rule, so its
    // local name becomes the new last segment unless it was aliased.
    const inMoved = startsWithSegments(binding.abs, ctx.oldModule);
    const boundAbs = inMoved
      ? [...ctx.newModule, ...binding.abs.slice(ctx.oldModule.length)]
      : binding.abs;
    if (startsWithSegments(abs, boundAbs)) {
      const name = binding.aliased ? head : (boundAbs[boundAbs.length - 1] as string);
      return [name, ...abs.slice(boundAbs.length)].join("::");
    }
  }
  return absolute;
}

/**
 * Rewrite every contiguous path in `content` that resolves into the moved
 * module; for the moved file itself (resolveModule !== emitModule) every
 * relative path is re-emitted from its new location (R4). Returns the new
 * content, the rewrite count, and brace-group sites left alone (R2).
 */
export function rewritePaths(
  content: string,
  ctx: RewriteContext,
): { content: string; count: number; unrewritten: readonly { line: number; text: string }[] } {
  const bindings = collectBindings(content, ctx.resolveModule, ctx.declaredBefore);
  const relocated = ctx.resolveModule !== ctx.emitModule;
  let count = 0;
  const rewritten = content.replace(CHAIN, (whole: string) => {
    const chain = whole.split("::");
    const abs = resolveChain(chain, ctx.resolveModule, ctx.declaredBefore, bindings);
    if (abs === null) return whole;
    const inMoved = startsWithSegments(abs, ctx.oldModule);
    if (!inMoved && (!relocated || chain[0] === "crate")) return whole;
    const target = inMoved ? [...ctx.newModule, ...abs.slice(ctx.oldModule.length)] : abs;
    const next = emitPath(chain, target, ctx, bindings);
    if (next === whole) return whole;
    count += 1;
    return next;
  });
  // Brace groups whose MEMBER names the moved module (when the prefix itself
  // is inside it, the chain rule already rewrote the prefix).
  const unrewritten: { line: number; text: string }[] = [];
  for (const match of content.matchAll(USE_GROUP)) {
    const prefix = resolveChain(
      (match[1] as string).split("::"),
      ctx.resolveModule,
      ctx.declaredBefore,
      bindings,
    );
    if (prefix === null || startsWithSegments(prefix, ctx.oldModule)) continue;
    const hit = groupMembers(match[2] as string).some(({ segs }) =>
      startsWithSegments([...prefix, ...segs.filter((s) => s !== "self")], ctx.oldModule),
    );
    if (!hit) continue;
    const line = content.slice(0, match.index).split("\n").length;
    unrewritten.push({ line, text: match[0].trim() });
  }
  return { content: rewritten, count, unrewritten };
}

// ---------------------------------------------------------------------------
// The plan (R1–R8)

function renderModule(segments: Segments): string {
  return segments.length === 0 ? "crate" : `crate::${segments.join("::")}`;
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

function quoteAll(paths: readonly string[]): string {
  return paths.map((p) => JSON.stringify(p)).join(", ");
}

/** Compute every rewrite a Rust-aware move needs; never writes anything. */
export async function planRustMove(input: RustMoveInput): Promise<RustMovePlan> {
  const { from, to, fromContent } = input;
  const all = new Set(input.files.map((f) => f.path));
  const junk = new Set(input.files.filter((f) => f.junk).map((f) => f.path));
  const crateDir = crateDirOf(from, all);
  const abs = (rel: string): string => joinPath(crateDir, rel);
  const manifestPath = abs("Cargo.toml");
  const manifestContent = all.has(manifestPath) ? await input.read(manifestPath) : null;
  const manifestTargets = new Set(
    manifestContent === null ? [] : manifestTargetPaths(manifestContent),
  );

  const crateFiles = new Set<string>();
  for (const path of all) {
    if (isUnder(path, crateDir) && path.endsWith(".rs") && !junk.has(path)) {
      crateFiles.add(relativeTo(path, crateDir));
    }
  }
  const tree = buildModuleTree(crateFiles, manifestTargets);
  const fromRel = relativeTo(from, crateDir);
  const toRel = isUnder(to, crateDir) ? relativeTo(to, crateDir) : null;

  const writes: RustMoveWrite[] = [];
  const trailingNotes: string[] = [];

  // Manifest target entries follow the file whatever else happens (R7).
  let manifest: RustMovePlan["manifest"] = null;
  if (manifestContent !== null && toRel !== null && manifestTargets.has(fromRel)) {
    const next = rewriteManifestPaths(manifestContent, fromRel, toRel);
    if (next.count > 0) {
      writes.push({ path: manifestPath, before: manifestContent, content: next.content });
      manifest = { path: manifestPath, entries: next.count };
      trailingNotes.push(
        `${manifestPath}: ${plural(next.count, "target path entry")} rewritten to ${JSON.stringify(toRel)}`,
      );
    }
  }

  const skipped = (reason: string, oldModule: Segments | null = null): RustMovePlan => ({
    crateDir,
    oldModule,
    newModule: null,
    declaration: null,
    movedContent: fromContent,
    movedRewrites: 0,
    writes,
    pathRewrites: [],
    manifest,
    unrewritten: [],
    skipped: { reason },
    notes: [`rust: no module rewrite — ${reason}; the file was moved as-is`, ...trailingNotes],
  });

  // Root files (lib.rs, main.rs, targets): no declaration binds them.
  const location = locateModule(fromRel, tree);
  if (location !== null && location.segments.length === 0) {
    const auto = !manifestTargets.has(fromRel);
    return skipped(
      auto
        ? `${JSON.stringify(from)} is a crate root discovered by location (src/lib.rs, src/main.rs, src/bin/, tests/, examples/, benches/); moving it changes or removes that Cargo target — pin it with a [lib]/[[bin]]/[[test]]/[[example]]/[[bench]] path entry if that is not intended`
        : `${JSON.stringify(from)} is a crate root named by a Cargo.toml target path; its module tree is not rewritten`,
      [],
    );
  }

  // Same module, different file style (R6): nothing to rewrite.
  const stem = stemOf(fromRel) ?? "";
  const childDir = stem === "mod" ? dirOf(fromRel) : joinPath(dirOf(fromRel), stem);
  const sameModule =
    toRel !== null &&
    ((stem === "mod" && toRel === `${dirOf(fromRel)}.rs`) ||
      (stem !== "mod" && toRel === `${childDir}/mod.rs`));
  if (sameModule) {
    return skipped(
      `module path unchanged (${JSON.stringify(fromRel)} ↔ ${JSON.stringify(toRel)} is a style conversion of the same module); nothing to rewrite`,
      location?.segments ?? null,
    );
  }
  // Non-leaf (R6): the children cannot follow a single-file move.
  const children = [...crateFiles].filter((f) => f !== fromRel && isUnder(f, childDir));
  if (children.length > 0) {
    return skipped(
      `${JSON.stringify(from)} has child modules (${children.map((c) => relativeTo(c, childDir)).join(", ")}) and directories are not movable in v1, so they cannot follow`,
      location?.segments ?? null,
    );
  }
  if (location === null) {
    return skipped(
      `no module chain from ${JSON.stringify(from)} to a crate root was found by the file-module conventions (is it declared with #[path], or not declared at all?)`,
    );
  }
  const oldModule = location.segments;
  const oldName = oldModule[oldModule.length - 1] as string;

  // Confirm the derivation: the parent must actually say `mod <name>;` (R1).
  const contents = new Map<string, string | null>();
  const readRel = async (rel: string): Promise<string | null> => {
    if (!contents.has(rel)) contents.set(rel, await input.read(abs(rel)));
    return contents.get(rel) ?? null;
  };
  const declaring: { rel: string; content: string; item: DeclarationItem }[] = [];
  for (const parent of location.parents) {
    const content = await readRel(parent);
    if (content === null) continue;
    const item = findDeclaration(content, oldName);
    if (item !== null) declaring.push({ rel: parent, content, item });
  }
  if (declaring.length === 0) {
    return skipped(
      `no \`mod ${oldName};\` declaration found in ${location.parents.map((p) => JSON.stringify(abs(p))).join(" or ")}`,
      oldModule,
    );
  }
  const pathDeclared = declaring.find((p) => p.item.hasPathAttr);
  if (pathDeclared !== undefined) {
    return skipped(
      `\`mod ${oldName};\` in ${JSON.stringify(abs(pathDeclared.rel))} carries a #[path] attribute; #[path]-declared modules are out of scope (edit the attribute's path by hand)`,
      oldModule,
    );
  }

  // The new module path (R5): under the same root directory by convention.
  const toStem = toRel === null ? null : stemOf(toRel);
  if (toRel === null || toStem === null || !isUnder(toRel, location.rootDir)) {
    return skipped(
      `destination ${JSON.stringify(to)} is outside the module tree rooted at ${quoteAll(location.roots.map(abs))}; the old \`mod ${oldName};\` in ${quoteAll(declaring.map((p) => abs(p.rel)))} would dangle`,
      oldModule,
    );
  }
  if (toStem === "mod" && dirOf(toRel) === location.rootDir) {
    return skipped(
      `destination ${JSON.stringify(to)} is not a module file position (a mod.rs directly in the root directory)`,
      oldModule,
    );
  }
  const newParentDir = parentDirOf(toRel);
  const newModule = relativeTo(newParentDir, location.rootDir)
    .split("/")
    .filter((s) => s !== "");
  newModule.push(toStem === "mod" ? baseOf(dirOf(toRel)) : toStem);
  const newName = newModule[newModule.length - 1] as string;
  const sameParent = parentDirOf(fromRel) === newParentDir;

  // Where the NEW declaration goes: `<dir>.rs` / `<dir>/mod.rs`, or — at the
  // root directory — the root(s) that already own this module's top segment.
  let newParentCandidates: string[];
  if (newParentDir === location.rootDir) {
    const owners: string[] = [];
    for (const root of location.roots) {
      const content = await readRel(root);
      if (content !== null && declaredModules(content).has(oldModule[0] as string)) {
        owners.push(root);
      }
    }
    newParentCandidates = owners.length > 0 ? owners : [...location.roots];
  } else {
    newParentCandidates = [`${newParentDir}.rs`, `${newParentDir}/mod.rs`].filter((c) =>
      crateFiles.has(c),
    );
  }

  // Declaration edits, plus the declared-module sets AFTER the move per file.
  const contentAfter = new Map<string, string>();
  const declaredAfterByFile = new Map<string, Set<string>>();
  const declaration: { kind: RustDeclarationChange["kind"]; from: string[]; to: string | null } = {
    kind: sameParent ? "rename" : "move",
    from: [],
    to: null,
  };
  if (sameParent) {
    for (const p of declaring) {
      contentAfter.set(p.rel, renameDeclaration(p.content, oldName, newName));
      declaration.from.push(abs(p.rel));
      const declared = new Set(declaredModules(p.content));
      declared.delete(oldName);
      declared.add(newName);
      declaredAfterByFile.set(p.rel, declared);
    }
  } else {
    let itemText: string | null = null;
    for (const p of declaring) {
      contentAfter.set(p.rel, removeDeclaration(p.content, p.item));
      itemText ??= declarationItemText(p.content, p.item);
      declaration.from.push(abs(p.rel));
      const declared = new Set(declaredModules(p.content));
      declared.delete(oldName);
      declaredAfterByFile.set(p.rel, declared);
    }
    const newParent = newParentCandidates[0];
    const base =
      newParent === undefined ? null : (contentAfter.get(newParent) ?? (await readRel(newParent)));
    if (newParent !== undefined && base !== null && itemText !== null) {
      contentAfter.set(
        newParent,
        insertDeclaration(base, renameDeclaration(itemText, oldName, newName)),
      );
      declaration.to = abs(newParent);
      const declared = declaredAfterByFile.get(newParent) ?? new Set(declaredModules(base));
      declared.add(newName);
      declaredAfterByFile.set(newParent, declared);
    } else {
      declaration.kind = "remove-only";
    }
  }

  // Rewrite paths: the moved file (resolve at old, emit at new — R4) …
  const pathRewrites: { path: string; count: number }[] = [];
  const unrewritten: RustUnrewritten[] = [];
  const reportGroups = (path: string, sites: readonly { line: number; text: string }[]) => {
    for (const u of sites) {
      unrewritten.push({
        path,
        line: u.line,
        reason: `use-tree member names the moved module (${u.text}); split it by hand`,
      });
    }
  };
  const movedDeclared = declaredModules(fromContent);
  const movedResult = rewritePaths(fromContent, {
    resolveModule: oldModule,
    emitModule: newModule,
    declaredBefore: movedDeclared,
    declaredAfter: movedDeclared,
    oldModule,
    newModule,
  });
  reportGroups(to, movedResult.unrewritten);

  // … then every other file that shares this module tree (R2/R3/R8).
  for (const rel of [...crateFiles].sort()) {
    if (rel === fromRel) continue;
    const loc = locateModule(rel, tree);
    if (loc === null || !loc.roots.some((r) => location.roots.includes(r))) continue;
    const before = await readRel(rel);
    if (before === null) continue;
    const working = contentAfter.get(rel) ?? before;
    const declaredBefore = declaredModules(before);
    const result = rewritePaths(working, {
      resolveModule: loc.segments,
      emitModule: loc.segments,
      declaredBefore,
      declaredAfter: declaredAfterByFile.get(rel) ?? declaredBefore,
      oldModule,
      newModule,
    });
    reportGroups(abs(rel), result.unrewritten);
    if (result.count > 0) pathRewrites.push({ path: abs(rel), count: result.count });
    if (result.content !== before) writes.push({ path: abs(rel), before, content: result.content });
  }

  // Notes (R2/R5/R6/R8).
  const notes: string[] = [];
  const total = pathRewrites.reduce((n, r) => n + r.count, 0);
  const declNote =
    declaration.kind === "rename"
      ? `declaration renamed in ${quoteAll(declaration.from)}`
      : declaration.kind === "move"
        ? `declaration moved from ${quoteAll(declaration.from)} to ${JSON.stringify(declaration.to as string)}`
        : `declaration removed from ${quoteAll(declaration.from)} (it would name a missing file)`;
  notes.push(
    `rust: module ${renderModule(oldModule)} → ${renderModule(newModule)}; ${declNote}; ` +
      `${plural(total, "path")} rewritten across ${plural(pathRewrites.length, "file")}` +
      (movedResult.count > 0
        ? ` plus ${plural(movedResult.count, "path")} re-rooted in the moved file`
        : "") +
      " (contiguous crate::/super::/self::/uniform paths only; nothing synthesized)",
  );
  if (declaration.kind === "remove-only") {
    const wanted = quoteAll([`${newParentDir}.rs`, `${newParentDir}/mod.rs`].map(abs));
    const grandDir = parentDirOf(`${newParentDir}.rs`);
    const grand =
      grandDir === location.rootDir
        ? quoteAll(location.roots.map(abs))
        : quoteAll([`${grandDir}.rs`, `${grandDir}/mod.rs`].map(abs));
    notes.push(
      `no parent module file for ${JSON.stringify(abs(newParentDir))}/ (${wanted}): create one ` +
        `declaring \`mod ${newName};\` and declare \`mod ${baseOf(newParentDir)};\` in ${grand} ` +
        "yourself — nothing was synthesized",
    );
  }
  for (const u of unrewritten) notes.push(`not rewritten: ${u.path}:${u.line} — ${u.reason}`);
  if (declaring.some((p) => p.item.isPub)) {
    notes.push(
      `paths naming ${renderModule(oldModule)} from OTHER crates in the workspace ` +
        `(<crate_name>::${oldModule.join("::")}) are not rewritten`,
    );
  }

  return {
    crateDir,
    oldModule,
    newModule,
    declaration,
    movedContent: movedResult.content,
    movedRewrites: movedResult.count,
    writes,
    pathRewrites,
    manifest,
    unrewritten,
    skipped: null,
    notes: [...notes, ...trailingNotes],
  };
}
