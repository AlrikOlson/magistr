/**
 * The import-graph lint (SDD §6.4) — makes the dependency rule mechanical:
 *
 *   protocol → tools → ports ← adapters, nothing reaches around a port.
 *
 * Layers are the top-level directories under `src/`. Every layer must appear
 * in the edge table below; a file in an unlisted directory is a hard failure,
 * so a new layer cannot exist without declaring what it may depend on. Only
 * relative imports are classified — package imports carry no layer.
 *
 * CI runs this as `bun run lint:imports`; the checker itself is pure and unit
 * tested in `test/import-graph.test.ts`.
 */

/** Top-level directories under src/, plus "app" for files directly in src/. */
export type Layer =
  | "app"
  | "protocol"
  | "tools"
  | "ports"
  | "adapters"
  | "envelope"
  | "generated"
  | "jail"
  | "data"
  | "config"
  | "doctor";

/**
 * Which layers each layer may import from (itself always allowed). The
 * tool-layer line is the acceptance criterion: no `adapters`, no `generated`
 * bypass of the port vocabulary's own re-exports — tools see ports, the
 * envelope contract, and nothing else.
 */
export const ALLOWED_EDGES: Record<Layer, readonly Layer[]> = {
  app: [
    "protocol",
    "tools",
    "ports",
    "adapters",
    "envelope",
    "generated",
    "jail",
    "data",
    "config",
    "doctor",
  ],
  // The doctor CLI plane (SDD §11.2): composition like `app`, read-only by
  // policy — it consults protocol/adapters/config seams, never tools.
  doctor: ["protocol", "adapters", "ports", "envelope", "generated", "jail", "data", "config"],
  protocol: ["tools", "ports", "envelope", "generated", "jail", "data", "config"],
  tools: ["ports", "envelope"],
  ports: ["envelope", "generated"],
  adapters: ["ports", "envelope", "generated", "data"],
  envelope: ["generated"],
  generated: [],
  // The security jail (SDD §7.1) sits at the bottom like ports: everyone may
  // consult it, it depends on nothing — so no layer can smuggle policy into it.
  jail: [],
  // The data-lifecycle runner (SDD §10.2) likewise sits at the bottom: the
  // layers that open databases (protocol, adapters) consult it, it depends on
  // nothing — versioning policy cannot grow tendrils into upper layers.
  data: [],
  // The config subsystem (SDD §7.2) resolves the five layers as a pure
  // function of disk + env; it consults the data-lifecycle rules (§10.2)
  // for fail-closed versioning and nothing above itself.
  config: ["data"],
};

export interface SourceModule {
  /** Repo-relative posix path, e.g. "src/tools/search.ts". */
  readonly path: string;
  /** Raw import specifiers as written in the file. */
  readonly specifiers: readonly string[];
}

export interface Violation {
  readonly file: string;
  readonly specifier: string;
  readonly reason: string;
}

const IMPORT_PATTERNS = [
  /^\s*(?:import|export)\b[^"'`]*?\bfrom\s*["']([^"']+)["']/gm,
  /^\s*import\s*["']([^"']+)["']/gm,
  /\bimport\s*\(\s*["']([^"']+)["']\s*\)/g,
];

/** Static import/export-from/dynamic-import specifiers, in source order. */
export function extractImportSpecifiers(source: string): readonly string[] {
  const specifiers: string[] = [];
  for (const pattern of IMPORT_PATTERNS) {
    for (const match of source.matchAll(pattern)) {
      const specifier = match[1];
      if (specifier !== undefined) specifiers.push(specifier);
    }
  }
  return specifiers;
}

function normalize(path: string): string {
  const parts: string[] = [];
  for (const part of path.replaceAll("\\", "/").split("/")) {
    if (part === "" || part === ".") continue;
    if (part === "..") parts.pop();
    else parts.push(part);
  }
  return parts.join("/");
}

function layerOf(path: string): Layer | undefined {
  const parts = normalize(path).split("/");
  if (parts[0] !== "src" || parts.length < 2) return undefined;
  const second = parts[1];
  if (second !== undefined && second in ALLOWED_EDGES) return second as Layer;
  return parts.length === 2 ? "app" : undefined;
}

/** Check every module's relative imports against the layer edge table. */
export function checkImportGraph(modules: readonly SourceModule[]): readonly Violation[] {
  const violations: Violation[] = [];
  for (const module of modules) {
    const fromLayer = layerOf(module.path);
    if (fromLayer === undefined) {
      violations.push({
        file: module.path,
        specifier: "",
        reason: "file is outside every declared layer — add its directory to ALLOWED_EDGES",
      });
      continue;
    }
    const dir = normalize(module.path).split("/").slice(0, -1).join("/");
    for (const specifier of module.specifiers) {
      if (!specifier.startsWith(".")) continue;
      const target = normalize(`${dir}/${specifier}`);
      const toLayer = layerOf(target) ?? layerOf(`${target}/index.ts`);
      if (toLayer === undefined) {
        violations.push({
          file: module.path,
          specifier,
          reason: `resolves outside every declared layer (${target})`,
        });
      } else if (toLayer !== fromLayer && !ALLOWED_EDGES[fromLayer].includes(toLayer)) {
        violations.push({
          file: module.path,
          specifier,
          reason: `forbidden edge ${fromLayer} → ${toLayer}`,
        });
      }
    }
  }
  return violations;
}

if (import.meta.main) {
  const modules: SourceModule[] = [];
  for await (const file of new Bun.Glob("src/**/*.ts").scan(".")) {
    const path = file.replaceAll("\\", "/");
    modules.push({ path, specifiers: extractImportSpecifiers(await Bun.file(file).text()) });
  }
  const violations = checkImportGraph(modules);
  if (violations.length > 0) {
    console.error(`import-graph: ${violations.length} violation(s)`);
    for (const v of violations) {
      console.error(`  ${v.file}: "${v.specifier}" — ${v.reason}`);
    }
    process.exit(1);
  }
  console.log(`import-graph: ${modules.length} modules clean`);
}
