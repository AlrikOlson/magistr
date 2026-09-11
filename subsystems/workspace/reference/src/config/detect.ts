/**
 * Project-command detection (SDD §9.6): lockfile/manifest detection that
 * seeds the `[commands]` run allowlist at init. Deliberately HERE and not in
 * `map` (deferral recorded on chunk:m1-map-enrichment): a read-only overview
 * must never be the source of an execution allowlist. Detection only ever
 * SUGGESTS; the human-invoked `workbench-mcp init` writes the suggestions
 * into workbench.toml once, and from then on the table is amendable only by
 * a human — the served surface cannot touch it (workbench.toml is a
 * PROTECTED_FILE, constitution IV).
 *
 * Pure reads: manifests and lockfiles are consulted, nothing is executed.
 *
 * Cargo (specs/013 R8): a WORKSPACE is seeded with a whole-workspace test run
 * that keeps going past the first failing binary (a bare `cargo test` stops
 * there and hides the rest), one `cargo test -p <member>` per member, and
 * nextest/clippy commands only when the project DECLARES them (a
 * `.config/nextest.toml`, a `clippy` component in `rust-toolchain.toml`) —
 * detection may not run `cargo nextest --version` to find out. A single
 * crate is seeded exactly as before.
 */

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

export interface DetectedCommand {
  /** The `[commands]` key, e.g. "test". */
  readonly name: string;
  /** The command line, e.g. "bun test". */
  readonly line: string;
  /** What convicted it, e.g. "package.json scripts.test + bun.lock". */
  readonly provenance: string;
}

/** The package.json script names worth seeding, in declaration order. */
const SCRIPT_NAMES = ["test", "build", "lint", "typecheck", "check", "format", "dev"] as const;

function nodeRunnerFor(root: string): { runner: string; provenance: string } {
  if (existsSync(join(root, "bun.lock")) || existsSync(join(root, "bun.lockb"))) {
    return { runner: "bun run", provenance: "bun.lock" };
  }
  if (existsSync(join(root, "pnpm-lock.yaml"))) {
    return { runner: "pnpm run", provenance: "pnpm-lock.yaml" };
  }
  if (existsSync(join(root, "yarn.lock"))) {
    return { runner: "yarn run", provenance: "yarn.lock" };
  }
  return { runner: "npm run", provenance: "package-lock.json or default" };
}

// ---------------------------------------------------------------------------
// Cargo layout (textual manifest reads, no TOML parser — the idiom of map,
// specs/010 and specs/012)

export interface CargoMember {
  readonly name: string;
  /** Workspace-relative directory, forward slashes; "." for a root package. */
  readonly dir: string;
}

export interface CargoLayout {
  readonly kind: "workspace" | "crate";
  /** Members in name order (a root `[package]` included); empty for a crate. */
  readonly members: readonly CargoMember[];
  /** `members` entries this detection could not expand (globs beyond `<dir>/*`). */
  readonly skipped: readonly string[];
  /** `.config/nextest.toml` exists. */
  readonly nextest: boolean;
  /** `rust-toolchain.toml` lists the `clippy` component. */
  readonly clippy: boolean;
}

/** Per-member seeds are capped so a 200-crate workspace does not become a
 * 200-line table; the remainder is reported in the last seed's provenance. */
export const CARGO_MEMBER_SEED_CAP = 32;

/** The body of `[name]` up to the next `[section]` header, or undefined. */
function sectionOf(manifest: string, name: string): string | undefined {
  const lines = manifest.replace(/\r\n/g, "\n").split("\n");
  const start = lines.findIndex((l) => l.trim() === `[${name}]`);
  if (start < 0) return undefined;
  const body: string[] = [];
  for (const line of lines.slice(start + 1)) {
    if (/^\s*\[/.test(line)) break;
    body.push(line);
  }
  return body.join("\n");
}

/** `key = ["a", "b"]` (possibly spanning lines) → ["a", "b"]. */
function stringArrayOf(section: string, key: string): readonly string[] | undefined {
  const match = section.match(new RegExp(`^\\s*${key}\\s*=\\s*\\[([^\\]]*)\\]`, "m"));
  if (match === null) return undefined;
  return [...(match[1] as string).matchAll(/"([^"]*)"|'([^']*)'/g)].map(
    (m) => (m[1] ?? m[2]) as string,
  );
}

function stringOf(section: string, key: string): string | undefined {
  const match = section.match(new RegExp(`^\\s*${key}\\s*=\\s*(?:"([^"]*)"|'([^']*)')`, "m"));
  return match === null ? undefined : (match[1] ?? match[2]);
}

function readText(path: string): string | undefined {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return undefined;
  }
}

function isDirectory(path: string): boolean {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
}

const normalizeRel = (entry: string): string => entry.replace(/\\/g, "/").replace(/\/+$/, "");

/** Read the Cargo layout under `root`, or undefined when there is no manifest. */
export function cargoLayout(root: string): CargoLayout | undefined {
  const manifest = readText(join(root, "Cargo.toml"));
  if (manifest === undefined) return undefined;
  const nextest = existsSync(join(root, ".config", "nextest.toml"));
  const toolchain = readText(join(root, "rust-toolchain.toml"));
  const clippy =
    toolchain !== undefined &&
    (stringArrayOf(sectionOf(toolchain, "toolchain") ?? toolchain, "components") ?? []).includes(
      "clippy",
    );
  const workspace = sectionOf(manifest, "workspace");
  if (workspace === undefined) {
    return { kind: "crate", members: [], skipped: [], nextest, clippy };
  }

  const excluded = new Set((stringArrayOf(workspace, "exclude") ?? []).map(normalizeRel));
  const skipped: string[] = [];
  const candidates: string[] = [];
  for (const raw of stringArrayOf(workspace, "members") ?? []) {
    const entry = normalizeRel(raw);
    if (entry.endsWith("/*") && !/[*?[]/.test(entry.slice(0, -2))) {
      const dir = entry.slice(0, -2);
      let children: string[] = [];
      try {
        children = readdirSync(join(root, dir));
      } catch {
        children = [];
      }
      for (const child of children.sort()) {
        const rel = `${dir}/${child}`;
        if (isDirectory(join(root, rel)) && existsSync(join(root, rel, "Cargo.toml"))) {
          candidates.push(rel);
        }
      }
    } else if (/[*?[]/.test(entry)) {
      skipped.push(raw);
    } else if (existsSync(join(root, entry, "Cargo.toml"))) {
      candidates.push(entry);
    }
  }

  const members = new Map<string, CargoMember>();
  const rootName = stringOf(sectionOf(manifest, "package") ?? "", "name");
  if (rootName !== undefined) members.set(rootName, { name: rootName, dir: "." });
  for (const rel of candidates) {
    if (excluded.has(rel)) continue;
    const text = readText(join(root, rel, "Cargo.toml"));
    if (text === undefined) continue;
    const name = stringOf(sectionOf(text, "package") ?? "", "name");
    if (name !== undefined && !members.has(name)) members.set(name, { name, dir: rel });
  }
  return {
    kind: "workspace",
    members: [...members.values()].sort((a, b) => a.name.localeCompare(b.name)),
    skipped,
    nextest,
    clippy,
  };
}

// ---------------------------------------------------------------------------

/**
 * Detect runnable project commands from manifests and lockfiles. Returns
 * suggestions in a stable order (first detection of a name wins); never
 * executes anything and never writes anything.
 */
export function detectCommands(root: string): readonly DetectedCommand[] {
  const detected: DetectedCommand[] = [];
  const seen = new Set<string>();
  const add = (name: string, line: string, provenance: string): void => {
    if (seen.has(name)) return;
    seen.add(name);
    detected.push({ name, line, provenance });
  };

  // package.json scripts, discriminated by lockfile.
  const packageJson = join(root, "package.json");
  if (existsSync(packageJson)) {
    try {
      const parsed = JSON.parse(readFileSync(packageJson, "utf8")) as {
        scripts?: Record<string, unknown>;
      };
      const scripts = parsed.scripts ?? {};
      const { runner, provenance } = nodeRunnerFor(root);
      for (const name of SCRIPT_NAMES) {
        if (typeof scripts[name] === "string") {
          add(name, `${runner} ${name}`, `package.json scripts.${name} + ${provenance}`);
        }
      }
    } catch {
      // An unparseable package.json detects nothing — init reports what it
      // found, never what it guessed.
    }
  }

  // Cargo workspace or crate (specs/013 R8).
  const cargo = cargoLayout(root);
  if (cargo !== undefined) {
    const ws = cargo.kind === "workspace";
    const scope = ws ? " --workspace" : "";
    const manifest = ws ? "Cargo.toml [workspace]" : "Cargo.toml";
    const unexpanded =
      cargo.skipped.length === 0
        ? ""
        : `; members ${cargo.skipped.map((s) => `\`${s}\``).join(", ")} not expanded (only <dir>/* globs are)`;
    add(
      "cargo-test",
      ws ? "cargo test --workspace --no-fail-fast" : "cargo test",
      manifest + unexpanded,
    );
    add("cargo-build", `cargo build${scope}`, manifest);
    if (cargo.nextest) {
      add("cargo-nextest", `cargo nextest run${scope} --no-fail-fast`, ".config/nextest.toml");
    }
    if (cargo.clippy) {
      add("cargo-clippy", `cargo clippy${scope} --all-targets`, "rust-toolchain.toml components");
    }
    const seeded = cargo.members.slice(0, CARGO_MEMBER_SEED_CAP);
    const remainder = cargo.members.length - seeded.length;
    seeded.forEach((member, i) => {
      const note =
        i === seeded.length - 1 && remainder > 0
          ? `; ${remainder} more members not seeded (cap ${CARGO_MEMBER_SEED_CAP}, add by hand)`
          : "";
      add(
        `cargo-test-${member.name}`,
        `cargo test -p ${member.name}`,
        `${member.dir}/Cargo.toml [package]${note}`,
      );
    });
  }

  // Go module.
  if (existsSync(join(root, "go.mod"))) {
    add("go-test", "go test ./...", "go.mod");
    add("go-build", "go build ./...", "go.mod");
  }

  // Python: pytest when the project declares it.
  const pyproject = join(root, "pyproject.toml");
  const hasPytestConfig =
    existsSync(join(root, "pytest.ini")) ||
    (existsSync(pyproject) &&
      (() => {
        try {
          return readFileSync(pyproject, "utf8").includes("pytest");
        } catch {
          return false;
        }
      })());
  if (hasPytestConfig) {
    add("pytest", "pytest", existsSync(join(root, "pytest.ini")) ? "pytest.ini" : "pyproject.toml");
  }

  return detected;
}

/**
 * Render detections as a workbench.toml `[commands]` table block, with
 * provenance comments so the human sees why each line was seeded.
 */
export function renderCommandsTable(detected: readonly DetectedCommand[]): string {
  const lines = [
    "# Seeded by `workbench-mcp init` from lockfile/manifest detection (SDD §9.6).",
    "# Amend by hand: this table is run's allowlist and only a human may change it.",
    "[commands]",
  ];
  for (const d of detected) {
    lines.push(`${JSON.stringify(d.name)} = ${JSON.stringify(d.line)} # ${d.provenance}`);
  }
  return `${lines.join("\n")}\n`;
}
