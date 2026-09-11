/**
 * The Cargo workspace shape behind `map`'s crate rows (chunk:rust-cargo-
 * workspace-map, specs/014). Pure: manifests arrive through the injected
 * workspace read (only paths the walk listed), nothing here touches the
 * filesystem or runs cargo, and the `use` half of the graph is whatever the
 * index answered — counts of importing files per member dependency, never a
 * claim that a path resolves.
 *
 * Textual manifest reading, no TOML parser (the idiom of map, specs/010,
 * 012 and 013). The section/string/array readers duplicate ~30 lines of
 * src/config/detect.ts on purpose: tools may not import config (the
 * import-graph rule), and a shared module would have to live in a layer both
 * can reach, which neither `ports` nor `envelope` is.
 */

import type { CrateImportEdge, CrateScope } from "../ports";
import { isRootFile, manifestTargetPaths } from "./rust-move";

// ---------------------------------------------------------------------------
// Row vocabulary (mirrored into map's result union)

export type MapCrateRole = "lib" | "bin" | "proc-macro" | "build";
export type MapCrateEntryPointKind = "lib" | "bin" | "example" | "test" | "bench" | "build";

export interface MapCrateEntryPoint {
  /** Workspace-relative path as the manifest or the convention names it. */
  readonly path: string;
  readonly kind: MapCrateEntryPointKind;
  readonly name?: string;
}

export interface MapCrateDependency {
  /** The member's package name. */
  readonly name: string;
  /** Absent = a normal dependency. */
  readonly kind?: "dev" | "build";
  /** Importing files in this crate, when the index answered (R6). */
  readonly files?: number;
}

/** One workspace member as map reports it (specs/014 R1). */
export interface MapCrate {
  readonly kind: "crate";
  /** The member directory, workspace-relative; "." for a root package. */
  readonly path: string;
  readonly name: string;
  readonly roles: readonly MapCrateRole[];
  readonly edition?: string;
  readonly entryPoints: readonly MapCrateEntryPoint[];
  readonly moreEntryPoints?: number;
  readonly features: readonly string[];
  readonly moreFeatures?: number;
  /** Workspace members this crate depends on. */
  readonly dependencies: readonly MapCrateDependency[];
  /** Members that depend on this crate. */
  readonly dependents: number;
  /** Dependencies that are not workspace members (registry, git, outside). */
  readonly externalDependencies: number;
}

/** The overview's `cargo` summary: the whole workspace, whatever `path` scoped. */
export interface MapCargoSummary {
  readonly kind: "workspace" | "crate";
  readonly members: number;
  /** Member → member Cargo edges. */
  readonly edges: number;
  /** Members beyond the row cap (or outside the scope) not shown as rows. */
  readonly omitted?: number;
  /** Declared member dependencies no file imports — only when the index answered. */
  readonly unusedDeps?: number;
  readonly top: readonly { readonly name: string; readonly dependents: number }[];
}

/** Rows kept — a ranked summary, not an inventory (the MAP_ENTRY_POINTS_MAX rule). */
export const MAP_CRATE_ROWS_MAX = 24;
export const MAP_CRATE_ENTRY_POINTS_MAX = 8;
export const MAP_CRATE_FEATURES_MAX = 16;
/** Member manifests read per call. */
export const MAP_CRATE_MANIFESTS_MAX = 256;
const SUMMARY_TOP = 5;

// ---------------------------------------------------------------------------
// Textual manifest reading

interface Section {
  /** `package`, `[bin]` (for `[[bin]]`), `dependencies.foo`, `target.'cfg(x)'.dependencies`. */
  readonly name: string;
  readonly array: boolean;
  readonly body: string;
}

const HEADER = /^[ \t]*(\[\[?)([^\]]+)\]\]?[ \t]*(?:#.*)?$/;

/** Every `[section]` with its body; the preamble (no header) is dropped. */
function sectionsOf(manifest: string): Section[] {
  const out: Section[] = [];
  let current: { name: string; array: boolean; lines: string[] } | undefined;
  for (const line of manifest.replace(/\r\n/g, "\n").split("\n")) {
    const header = line.match(HEADER);
    if (header !== null) {
      if (current !== undefined) {
        out.push({ name: current.name, array: current.array, body: current.lines.join("\n") });
      }
      current = { name: (header[2] as string).trim(), array: header[1] === "[[", lines: [] };
      continue;
    }
    current?.lines.push(line);
  }
  if (current !== undefined) {
    out.push({ name: current.name, array: current.array, body: current.lines.join("\n") });
  }
  return out;
}

function stringOf(body: string, key: string): string | undefined {
  const match = body.match(new RegExp(`^[ \\t]*${key}[ \\t]*=[ \\t]*(?:"([^"]*)"|'([^']*)')`, "m"));
  return match === null ? undefined : (match[1] ?? match[2]);
}

function boolOf(body: string, key: string): boolean | undefined {
  const match = body.match(new RegExp(`^[ \\t]*${key}[ \\t]*=[ \\t]*(true|false)\\b`, "m"));
  return match === null ? undefined : match[1] === "true";
}

/** `key = ["a", "b"]`, possibly spanning lines. */
function stringArrayOf(body: string, key: string): readonly string[] | undefined {
  const match = body.match(new RegExp(`^[ \\t]*${key}[ \\t]*=[ \\t]*\\[([^\\]]*)\\]`, "m"));
  if (match === null) return undefined;
  return [...(match[1] as string).matchAll(/"([^"]*)"|'([^']*)'/g)].map(
    (m) => (m[1] ?? m[2]) as string,
  );
}

/** Keys of a table body (`name = …`), in order. */
function keysOf(body: string): string[] {
  const out: string[] = [];
  for (const line of body.split("\n")) {
    const match = line.match(/^[ \t]*(?:"([^"]+)"|([A-Za-z0-9_.-]+))[ \t]*=/);
    if (match !== null) out.push((match[1] ?? match[2]) as string);
  }
  return out;
}

const normalizeRel = (path: string): string =>
  path
    .replace(/\\/g, "/")
    .split("/")
    .filter((p) => p !== "" && p !== ".")
    .join("/");

/** Join a member directory and a manifest-relative path, resolving `..`. */
function resolveRel(dir: string, rel: string): string {
  const parts: string[] = dir === "." ? [] : dir.split("/");
  for (const part of normalizeRel(rel).split("/")) {
    if (part === "") continue;
    if (part === "..") parts.pop();
    else parts.push(part);
  }
  return parts.length === 0 ? "." : parts.join("/");
}

const DEP_TABLE =
  /^(?:target\.(?:'[^']*'|"[^"]*"|[^.]+)\.)?(dependencies|dev-dependencies|build-dependencies)(?:\.(.+))?$/;

interface RawDependency {
  readonly key: string;
  readonly kind?: "dev" | "build";
  readonly path?: string;
  readonly packageName?: string;
  readonly workspace?: boolean;
}

function kindOf(table: string): "dev" | "build" | undefined {
  if (table === "dev-dependencies") return "dev";
  if (table === "build-dependencies") return "build";
  return undefined;
}

/** Every dependency entry across the plain, dev, build and target tables. */
function dependenciesOf(sections: readonly Section[]): RawDependency[] {
  const out: RawDependency[] = [];
  for (const section of sections) {
    const table = section.name.match(DEP_TABLE);
    if (table === null) continue;
    const kind = kindOf(table[1] as string);
    const sub = table[2];
    if (sub !== undefined) {
      // `[dependencies.foo]` — the body IS the entry.
      out.push({
        key: sub.replace(/^"|"$/g, ""),
        ...(kind === undefined ? {} : { kind }),
        ...pick(section.body),
      });
      continue;
    }
    for (const line of section.body.split("\n")) {
      const entry = line.match(/^[ \t]*(?:"([^"]+)"|([A-Za-z0-9_-]+))[ \t]*=[ \t]*(.+)$/);
      if (entry === null) continue;
      const key = (entry[1] ?? entry[2]) as string;
      const value = (entry[3] as string).trim();
      const inline = value.startsWith("{") ? value : "";
      out.push({ key, ...(kind === undefined ? {} : { kind }), ...pick(inline) });
    }
  }
  return out;
}

/** `path`, `package`, `workspace` from an inline table or a sub-table body. */
function pick(text: string): Pick<RawDependency, "path" | "packageName" | "workspace"> {
  const path = text.match(/\bpath[ \t]*=[ \t]*"([^"]*)"/)?.[1];
  const packageName = text.match(/\bpackage[ \t]*=[ \t]*"([^"]*)"/)?.[1];
  const workspace = /\bworkspace[ \t]*=[ \t]*true\b/.test(text);
  return {
    ...(path === undefined ? {} : { path }),
    ...(packageName === undefined ? {} : { packageName }),
    ...(workspace ? { workspace: true } : {}),
  };
}

// ---------------------------------------------------------------------------
// The workspace model

export interface CargoMemberModel {
  readonly name: string;
  /** Member directory; "." for a root package. */
  readonly dir: string;
  /** The name `use` paths spell: `[lib] name`, else the package name with `-` → `_`. */
  readonly useName: string;
  readonly edition?: string;
  readonly roles: readonly MapCrateRole[];
  readonly entryPoints: readonly MapCrateEntryPoint[];
  readonly features: readonly string[];
  readonly dependencies: readonly { readonly name: string; readonly kind?: "dev" | "build" }[];
  readonly externalDependencies: number;
}

export interface CargoWorkspaceModel {
  readonly kind: "workspace" | "crate";
  readonly members: readonly CargoMemberModel[];
  readonly notes: readonly string[];
}

interface MemberManifest {
  readonly dir: string;
  readonly manifest: string;
  readonly sections: readonly Section[];
  readonly name: string;
}

const CONVENTION_KIND: readonly { readonly re: RegExp; readonly kind: MapCrateEntryPointKind }[] = [
  { re: /^src\/lib\.rs$/, kind: "lib" },
  { re: /^src\/main\.rs$/, kind: "bin" },
  { re: /^src\/bin\/[^/]+\.rs$|^src\/bin\/[^/]+\/main\.rs$/, kind: "bin" },
  { re: /^examples\/[^/]+\.rs$|^examples\/[^/]+\/main\.rs$/, kind: "example" },
  { re: /^tests\/[^/]+\.rs$|^tests\/[^/]+\/main\.rs$/, kind: "test" },
  { re: /^benches\/[^/]+\.rs$|^benches\/[^/]+\/main\.rs$/, kind: "bench" },
  { re: /^build\.rs$/, kind: "build" },
];

function conventionKind(rel: string): MapCrateEntryPointKind | undefined {
  return CONVENTION_KIND.find((c) => c.re.test(rel))?.kind;
}

/** `src/bin/foo.rs` → foo; `src/bin/foo/main.rs` → foo; `examples/e.rs` → e. */
function conventionName(rel: string): string | undefined {
  const parts = rel.split("/");
  const last = parts[parts.length - 1] as string;
  if (last === "main.rs" && parts.length >= 3) return parts[parts.length - 2];
  if (parts.length >= 2 && parts[0] !== "src") return last.replace(/\.rs$/, "");
  if (parts[1] === "bin" && parts.length === 3) return last.replace(/\.rs$/, "");
  return undefined;
}

const TARGET_KIND: Readonly<Record<string, MapCrateEntryPointKind>> = {
  lib: "lib",
  bin: "bin",
  example: "example",
  test: "test",
  bench: "bench",
};

const wsPath = (dir: string, rel: string): string => (dir === "." ? rel : `${dir}/${rel}`);

/** Entry points: manifest targets first (with names), then conventions. */
function entryPointsOf(
  member: MemberManifest,
  crateFiles: readonly string[],
): { readonly entryPoints: MapCrateEntryPoint[]; readonly roles: MapCrateRole[] } {
  const entries: MapCrateEntryPoint[] = [];
  const seen = new Set<string>();
  const push = (entry: MapCrateEntryPoint): void => {
    if (seen.has(entry.path)) return;
    seen.add(entry.path);
    entries.push(entry);
  };
  let procMacro = false;
  for (const section of member.sections) {
    const kind = TARGET_KIND[section.name];
    if (kind === undefined) continue;
    if (section.name === "lib" && boolOf(section.body, "proc-macro") === true) procMacro = true;
    const name = stringOf(section.body, "name");
    const path = stringOf(section.body, "path");
    const rel =
      path !== undefined
        ? normalizeRel(path)
        : section.name === "lib"
          ? "src/lib.rs"
          : name === undefined
            ? undefined
            : `${section.name === "bin" ? "src/bin" : `${section.name}s`}/${name}.rs`;
    if (rel === undefined) continue;
    push({ path: wsPath(member.dir, rel), kind, ...(name === undefined ? {} : { name }) });
  }
  const targets = new Set(manifestTargetPaths(member.manifest));
  for (const rel of [...crateFiles].sort()) {
    if (!isRootFile(rel, targets)) continue;
    const kind = conventionKind(rel);
    if (kind === undefined) continue;
    const name = conventionName(rel);
    push({ path: wsPath(member.dir, rel), kind, ...(name === undefined ? {} : { name }) });
  }
  const roles: MapCrateRole[] = [];
  if (entries.some((e) => e.kind === "lib")) roles.push(procMacro ? "proc-macro" : "lib");
  if (entries.some((e) => e.kind === "bin")) roles.push("bin");
  if (entries.some((e) => e.kind === "build")) roles.push("build");
  return { entryPoints: entries, roles };
}

function featuresOf(sections: readonly Section[]): string[] {
  const features = sections.find((s) => s.name === "features");
  if (features === undefined) return [];
  const keys = keysOf(features.body);
  return keys.includes("default") ? ["default", ...keys.filter((k) => k !== "default")] : keys;
}

/**
 * Read the workspace: members from the root manifest's `[workspace]` table
 * (`<dir>/*` globs expanded against the walk, `exclude` honored, a root
 * `[package]` included), each member's manifest read only when the walk
 * listed it. Returns undefined when the walk has no root Cargo.toml or it
 * cannot be read — the non-Cargo (and unreadable) answer is silence.
 */
export async function readCargoWorkspace(
  files: readonly string[],
  read: (path: string) => Promise<string>,
): Promise<CargoWorkspaceModel | undefined> {
  const present = new Set(files);
  if (!present.has("Cargo.toml")) return undefined;
  let rootManifest: string;
  try {
    rootManifest = await read("Cargo.toml");
  } catch {
    return undefined;
  }
  const rootSections = sectionsOf(rootManifest);
  const workspace = rootSections.find((s) => s.name === "workspace");
  const notes: string[] = [];

  // Member directories.
  const dirs: string[] = [];
  const skipped: string[] = [];
  if (workspace !== undefined) {
    const excluded = new Set((stringArrayOf(workspace.body, "exclude") ?? []).map(normalizeRel));
    for (const raw of stringArrayOf(workspace.body, "members") ?? []) {
      const entry = normalizeRel(raw);
      if (entry.endsWith("/*") && !/[*?[]/.test(entry.slice(0, -2))) {
        const base = entry.slice(0, -2);
        const children = new Set<string>();
        for (const path of files) {
          if (!path.startsWith(`${base}/`)) continue;
          const rest = path.slice(base.length + 1).split("/");
          if (rest.length === 2 && rest[1] === "Cargo.toml") children.add(rest[0] as string);
        }
        for (const child of [...children].sort()) dirs.push(`${base}/${child}`);
      } else if (/[*?[]/.test(entry)) {
        skipped.push(raw);
      } else if (present.has(`${entry}/Cargo.toml`)) {
        dirs.push(entry);
      }
    }
    for (let i = dirs.length - 1; i >= 0; i--) {
      if (excluded.has(dirs[i] as string)) dirs.splice(i, 1);
    }
  }
  if (skipped.length > 0) {
    notes.push(
      `Cargo members ${skipped.map((s) => `\`${s}\``).join(", ")} not expanded (only <dir>/* globs are)`,
    );
  }

  // Manifests.
  const manifests: MemberManifest[] = [];
  const rootName = stringOf(rootSections.find((s) => s.name === "package")?.body ?? "", "name");
  if (rootName !== undefined) {
    manifests.push({ dir: ".", manifest: rootManifest, sections: rootSections, name: rootName });
  }
  const uniqueDirs = [...new Set(dirs)];
  const omittedManifests = Math.max(0, uniqueDirs.length - MAP_CRATE_MANIFESTS_MAX);
  for (const dir of uniqueDirs.slice(0, MAP_CRATE_MANIFESTS_MAX)) {
    let manifest: string;
    try {
      manifest = await read(`${dir}/Cargo.toml`);
    } catch {
      continue;
    }
    const sections = sectionsOf(manifest);
    const name = stringOf(sections.find((s) => s.name === "package")?.body ?? "", "name");
    if (name === undefined) continue;
    manifests.push({ dir, manifest, sections, name });
  }
  if (omittedManifests > 0) {
    notes.push(
      `${omittedManifests} Cargo member manifests beyond the ${MAP_CRATE_MANIFESTS_MAX} read; those members are not shown`,
    );
  }
  if (manifests.length === 0) return undefined;

  // Files per member: the longest member directory prefixing a path owns it;
  // the root package owns what no other member does.
  const memberDirs = manifests.map((m) => m.dir).filter((d) => d !== ".");
  const byLength = [...memberDirs].sort((a, b) => b.length - a.length);
  const crateFiles = new Map<string, string[]>();
  for (const m of manifests) crateFiles.set(m.dir, []);
  for (const path of files) {
    const owner = byLength.find((d) => path.startsWith(`${d}/`));
    if (owner !== undefined) {
      crateFiles.get(owner)?.push(path.slice(owner.length + 1));
    } else if (crateFiles.has(".")) {
      crateFiles.get(".")?.push(path);
    }
  }

  // Members.
  const byName = new Map(manifests.map((m) => [m.name, m] as const));
  const byDir = new Map(manifests.map((m) => [m.dir, m] as const));
  const workspaceDeps = rootSections.find((s) => s.name === "workspace.dependencies");
  const members: CargoMemberModel[] = manifests.map((m) => {
    const lib = m.sections.find((s) => s.name === "lib");
    const libName = lib === undefined ? undefined : stringOf(lib.body, "name");
    const pkg = m.sections.find((s) => s.name === "package")?.body ?? "";
    const edition = stringOf(pkg, "edition");
    const { entryPoints, roles } = entryPointsOf(m, crateFiles.get(m.dir) ?? []);
    const seenDeps = new Set<string>();
    const dependencies: { name: string; kind?: "dev" | "build" }[] = [];
    let externalDependencies = 0;
    for (const dep of dependenciesOf(m.sections)) {
      let target: MemberManifest | undefined;
      if (dep.path !== undefined) {
        target = byDir.get(resolveRel(m.dir, dep.path));
      } else if (dep.workspace === true && workspaceDeps !== undefined) {
        const declared = pick(
          workspaceDeps.body
            .split("\n")
            .find((l) => new RegExp(`^[ \\t]*"?${dep.key}"?[ \\t]*=`).test(l)) ?? "",
        );
        target =
          declared.path !== undefined
            ? byDir.get(resolveRel(".", declared.path))
            : byName.get(declared.packageName ?? dep.key);
      } else {
        target = byName.get(dep.packageName ?? dep.key);
      }
      if (target === undefined || target.name === m.name) {
        if (target === undefined) externalDependencies += 1;
        continue;
      }
      const key = `${target.name} ${dep.kind ?? ""}`;
      if (seenDeps.has(key)) continue;
      seenDeps.add(key);
      dependencies.push({
        name: target.name,
        ...(dep.kind === undefined ? {} : { kind: dep.kind }),
      });
    }
    return {
      name: m.name,
      dir: m.dir,
      useName: libName ?? m.name.replace(/-/g, "_"),
      ...(edition === undefined ? {} : { edition }),
      roles,
      entryPoints,
      features: featuresOf(m.sections),
      dependencies,
      externalDependencies,
    };
  });

  return { kind: workspace === undefined ? "crate" : "workspace", members, notes };
}

/** What map asks the index for: one scope per member (R6). */
export function crateScopes(model: CargoWorkspaceModel): CrateScope[] {
  return model.members.map((m) => ({ name: m.useName, dir: m.dir }));
}

// ---------------------------------------------------------------------------
// Rendering: rows ranked and capped, the summary over the whole workspace

export interface RenderedCargo {
  readonly summary: MapCargoSummary;
  readonly rows: readonly MapCrate[];
  readonly notes: readonly string[];
}

/**
 * Turn the model (plus the index's per-crate edges, when it answered) into
 * the overview summary and the crate rows under `prefix`.
 */
export function renderCargo(
  model: CargoWorkspaceModel,
  prefix: string,
  edges: readonly CrateImportEdge[] | undefined,
): RenderedCargo {
  const dependents = new Map<string, number>();
  let edgeCount = 0;
  for (const m of model.members) {
    for (const dep of m.dependencies) {
      dependents.set(dep.name, (dependents.get(dep.name) ?? 0) + 1);
      edgeCount += 1;
    }
  }
  const useNameToMember = new Map(model.members.map((m) => [m.useName, m] as const));
  const filesFor = (dependentDir: string, depName: string): number | undefined => {
    if (edges === undefined) return undefined;
    const dep = model.members.find((m) => m.name === depName);
    if (dep === undefined) return undefined;
    return edges
      .filter((e) => e.from === dependentDir && e.to === dep.useName && useNameToMember.has(e.to))
      .reduce((sum, e) => sum + e.files, 0);
  };
  let unusedDeps = 0;
  const rows: MapCrate[] = model.members.map((m) => {
    const dependencies: MapCrateDependency[] = m.dependencies.map((dep) => {
      const files = filesFor(m.dir, dep.name);
      if (files === 0) unusedDeps += 1;
      return { ...dep, ...(files === undefined ? {} : { files }) };
    });
    const entryPoints = m.entryPoints.slice(0, MAP_CRATE_ENTRY_POINTS_MAX);
    const features = m.features.slice(0, MAP_CRATE_FEATURES_MAX);
    return {
      kind: "crate",
      path: m.dir,
      name: m.name,
      roles: m.roles,
      ...(m.edition === undefined ? {} : { edition: m.edition }),
      entryPoints,
      ...(m.entryPoints.length > entryPoints.length
        ? { moreEntryPoints: m.entryPoints.length - entryPoints.length }
        : {}),
      features,
      ...(m.features.length > features.length
        ? { moreFeatures: m.features.length - features.length }
        : {}),
      dependencies,
      dependents: dependents.get(m.name) ?? 0,
      externalDependencies: m.externalDependencies,
    };
  });
  rows.sort(
    (a, b) => b.dependents - a.dependents || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0),
  );

  const scoped = rows.filter(
    (row) => prefix === "" || row.path === prefix || row.path.startsWith(`${prefix}/`),
  );
  const kept = scoped.slice(0, MAP_CRATE_ROWS_MAX);
  const omitted = rows.length - kept.length;
  const notes: string[] = [...model.notes];
  if (scoped.length > kept.length) {
    notes.push(
      `${scoped.length - kept.length} workspace members beyond the ${MAP_CRATE_ROWS_MAX} crate rows shown; ` +
        "map path:<member dir> lists a member",
    );
  }
  const summary: MapCargoSummary = {
    kind: model.kind,
    members: model.members.length,
    edges: edgeCount,
    ...(omitted > 0 ? { omitted } : {}),
    ...(edges === undefined ? {} : { unusedDeps }),
    top: rows.slice(0, SUMMARY_TOP).map((r) => ({ name: r.name, dependents: r.dependents })),
  };
  return { summary, rows: kept, notes };
}
