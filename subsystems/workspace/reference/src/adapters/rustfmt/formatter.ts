/**
 * RustfmtFormatter — the rustfmt-backed FormatterPort adapter for `.rs`
 * files (SDD §6.4, §7.5; specs/012-rust-rustfmt-formatter).
 *
 * One spawn per format call: the candidate content rides stdin, the
 * formatted result rides stdout. Three things make the output match what
 * the project's own `cargo fmt` would produce:
 *
 *   - the BINARY is the workspace's: `rustup which rustfmt` with the
 *     workspace root as cwd honors `rust-toolchain.toml` and only succeeds
 *     when the component is installed for that toolchain; a PATH `rustfmt`
 *     confirmed by `--version` is the fallback ONLY when the workspace has
 *     no toolchain pin (a pin that rustup cannot satisfy must not be quietly
 *     replaced by some other toolchain's rustfmt);
 *   - the EDITION is passed: standalone rustfmt does not read Cargo.toml, so
 *     `--edition` comes from the nearest manifest's `[package] edition`, or
 *     the root `[workspace.package]` when inherited, default 2021;
 *   - the CWD is the file's directory, so `rustfmt.toml` discovery from stdin
 *     input walks up the way it would for the file itself.
 *
 * Failure is a throw with a message the agent can act on, never a guess: an
 * unresolvable binary, a non-zero exit (rustfmt rejected the content), a
 * spawn error, or a hung process (killed after the timeout) all reject, and
 * the §7.5 pipeline converts the rejection into "applied the verified
 * unformatted content" with a note carrying that message. Files that are not
 * `.rs` pass through unchanged without spawning (the router normally keeps
 * them away from this adapter entirely).
 */

import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import type { FormatRequest, FormatResult, FormatterPort } from "../../ports";
import { PORT_VERSIONS } from "../../ports";

/** A formatter that outlives this has hung; the write must not hang with it. */
export const RUSTFMT_TIMEOUT_MS = 10_000;

export const RUST_EDITIONS = ["2015", "2018", "2021", "2024"] as const;
export type RustEdition = (typeof RUST_EDITIONS)[number];
export const RUST_EDITION_DEFAULT: RustEdition = "2021";

/** Resolve the workspace's rustfmt to something that runs, or null (injectable). */
export type RustfmtResolver = (root: string) => string | null;

/** Does `bin --version` exit 0? Rejects a rustup proxy whose toolchain lacks the component. */
function runs(bin: string): boolean {
  try {
    return Bun.spawnSync([bin, "--version"], { stdout: "pipe", stderr: "pipe" }).exitCode === 0;
  } catch {
    return false;
  }
}

/** Does the workspace pin its toolchain (rust-toolchain.toml or the legacy rust-toolchain)? */
export function hasToolchainPin(root: string): boolean {
  return existsSync(join(root, "rust-toolchain.toml")) || existsSync(join(root, "rust-toolchain"));
}

/**
 * Resolve rustfmt for a workspace (research R2): the toolchain manager first
 * (`rustup which rustfmt`, cwd = root, honoring the pin), then — only when
 * the workspace has NO pin — a runnable PATH `rustfmt`. Null means "not
 * installed for this workspace's toolchain".
 */
export function resolveRustfmt(root: string): string | null {
  try {
    const result = Bun.spawnSync(["rustup", "which", "rustfmt"], {
      cwd: root,
      stdout: "pipe",
      stderr: "pipe",
    });
    if (result.exitCode === 0) {
      const path = new TextDecoder().decode(result.stdout).trim();
      if (path.length > 0 && runs(path)) return path;
    }
  } catch {
    // rustup absent: fall through to the PATH probe (unpinned workspaces only).
  }
  if (hasToolchainPin(root)) return null;
  const onPath = Bun.which("rustfmt");
  return onPath !== null && runs(onPath) ? onPath : null;
}

function normalizeRel(path: string): string {
  return path
    .replaceAll("\\", "/")
    .split("/")
    .filter((p) => p !== "" && p !== ".")
    .join("/");
}

function dirOf(rel: string): string {
  const slash = rel.lastIndexOf("/");
  return slash < 0 ? "" : rel.slice(0, slash);
}

/** `edition = "…"` inside the first matching table of a manifest, textually. */
function editionInTable(manifest: string, table: string): string | null {
  for (const section of manifest.split(/^(?=[ \t]*\[)/m)) {
    if (!section.trimStart().startsWith(`[${table}]`)) continue;
    const edition = /^[ \t]*edition[ \t]*=[ \t]*"(\d{4})"/m.exec(section)?.[1];
    return edition ?? null;
  }
  return null;
}

function inheritsEdition(manifest: string): boolean {
  for (const section of manifest.split(/^(?=[ \t]*\[)/m)) {
    if (!section.trimStart().startsWith("[package]")) continue;
    return (
      /^[ \t]*edition\.workspace[ \t]*=[ \t]*true/m.test(section) ||
      /^[ \t]*edition[ \t]*=[ \t]*\{[^}]*workspace[ \t]*=[ \t]*true/m.test(section)
    );
  }
  return false;
}

/**
 * The edition rustfmt must be told (research R3): the nearest `Cargo.toml`
 * above `path` (within the root) — its `[package] edition`, or when absent
 * or `edition.workspace = true`, the root `[workspace.package] edition` —
 * else the default. `read` answers null for a missing manifest; pure, so it
 * is testable over in-memory files.
 */
export function editionFor(path: string, read: (rel: string) => string | null): RustEdition {
  const rel = normalizeRel(path);
  let dir = dirOf(rel);
  for (;;) {
    const manifest = read(dir === "" ? "Cargo.toml" : `${dir}/Cargo.toml`);
    if (manifest !== null) {
      const own = inheritsEdition(manifest) ? null : editionInTable(manifest, "package");
      if (own !== null) return asEdition(own);
      const rootManifest = dir === "" ? manifest : read("Cargo.toml");
      const inherited =
        rootManifest === null ? null : editionInTable(rootManifest, "workspace.package");
      if (inherited !== null) return asEdition(inherited);
      // A manifest that says nothing: keep walking (an enclosing crate or the
      // root workspace may say); at the root, the default.
      if (dir === "") return RUST_EDITION_DEFAULT;
    }
    if (dir === "") return RUST_EDITION_DEFAULT;
    dir = dirOf(dir);
  }
}

function asEdition(value: string): RustEdition {
  return (RUST_EDITIONS as readonly string[]).includes(value)
    ? (value as RustEdition)
    : RUST_EDITION_DEFAULT;
}

export interface RustfmtFormatterOptions {
  /** Workspace root — rustup's cwd for pin resolution; manifests are read under it. */
  readonly root: string;
  /** Explicit binary path: skips resolution (test seam). */
  readonly binary?: string;
  /** Arguments placed BEFORE `--edition` (test seam for script-backed fakes). */
  readonly binaryArgs?: readonly string[];
  /** Injectable resolution (defaults to `resolveRustfmt`). */
  readonly resolver?: RustfmtResolver;
  /** Kill a hung rustfmt after this long (default 10 s). */
  readonly timeoutMs?: number;
}

export class RustfmtFormatter implements FormatterPort {
  readonly version = PORT_VERSIONS.formatter;
  readonly #root: string;
  readonly #explicitBinary: string | undefined;
  readonly #binaryArgs: readonly string[];
  readonly #resolver: RustfmtResolver;
  readonly #timeoutMs: number;
  /** Memoized resolution: undefined = untried, null = unresolvable. */
  #resolved: string | null | undefined;
  readonly #manifests = new Map<string, string | null>();

  constructor(options: RustfmtFormatterOptions) {
    this.#root = options.root;
    this.#explicitBinary = options.binary;
    this.#binaryArgs = options.binaryArgs ?? [];
    this.#resolver = options.resolver ?? resolveRustfmt;
    this.#timeoutMs = options.timeoutMs ?? RUSTFMT_TIMEOUT_MS;
  }

  async format(request: FormatRequest): Promise<FormatResult> {
    const ext = request.path.slice(request.path.lastIndexOf(".") + 1).toLowerCase();
    if (ext !== "rs") return { content: request.content, changed: false };
    const binary = this.#binary();
    if (binary === null) {
      throw new Error(
        "rustfmt is not installed for this workspace's toolchain (rustup component add rustfmt)",
      );
    }
    const edition = editionFor(request.path, (rel) => this.#manifest(rel));
    const cwd = resolve(this.#root, dirname(normalizeRel(request.path)) || ".");
    const content = await runRustfmt(
      binary,
      [...this.#binaryArgs, "--edition", edition],
      existsSync(cwd) ? cwd : this.#root,
      request.content,
      this.#timeoutMs,
    );
    return { content, changed: content !== request.content };
  }

  /** The edition this adapter would pass for `path` (tests, doctor). */
  editionOf(path: string): RustEdition {
    return editionFor(path, (rel) => this.#manifest(rel));
  }

  #binary(): string | null {
    if (this.#explicitBinary !== undefined) return this.#explicitBinary;
    if (this.#resolved !== undefined) return this.#resolved;
    this.#resolved = this.#resolver(this.#root);
    return this.#resolved;
  }

  #manifest(rel: string): string | null {
    const cached = this.#manifests.get(rel);
    if (cached !== undefined) return cached;
    let text: string | null;
    try {
      text = readFileSync(join(this.#root, rel), "utf8");
    } catch {
      text = null;
    }
    this.#manifests.set(rel, text);
    return text;
  }
}

function runRustfmt(
  binary: string,
  args: readonly string[],
  cwd: string,
  input: string,
  timeoutMs: number,
): Promise<string> {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(binary, [...args], {
      cwd,
      stdio: ["pipe", "pipe", "pipe"],
      timeout: timeoutMs,
      windowsHide: true,
    });
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    child.stdout.on("data", (chunk: Buffer) => stdout.push(chunk));
    child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));
    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (code === 0) {
        resolvePromise(Buffer.concat(stdout).toString("utf8"));
        return;
      }
      const detail = Buffer.concat(stderr).toString("utf8").trim().split("\n")[0] ?? "";
      reject(
        new Error(
          signal !== null
            ? `rustfmt was killed by ${signal} after ${timeoutMs}ms`
            : `rustfmt exited ${code}${detail === "" ? "" : `: ${detail}`}`,
        ),
      );
    });
    // EPIPE from a child that died before draining stdin; `close` reports it.
    child.stdin.on("error", () => {});
    child.stdin.end(input, "utf8");
  });
}
