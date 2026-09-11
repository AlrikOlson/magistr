/**
 * BiomeFormatter — the Biome-backed FormatterPort adapter (SDD §6.4, §7.5).
 *
 * One spawn of the platform's Biome CLI per format call: the candidate
 * content rides stdin, the formatted result rides stdout
 * (`biome format --stdin-file-path=<path>`), and the workspace root is the
 * child's cwd so a user biome.json is discovered exactly the way Biome
 * itself would discover it. No daemon, no state (Constitution I).
 *
 * Failure is a throw, never a guess: an unresolvable platform binary, a
 * non-zero exit, a spawn error, or a hung process (killed after 10s) all
 * reject, and the §7.5 pipeline converts the rejection into "applied the
 * verified unformatted content" with a note — degradation stays visible
 * instead of silent (Constitution II). Files outside Biome's coverage
 * (anything but the js/ts family and json) pass through unchanged without
 * spawning at all.
 */

import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import type { FormatRequest, FormatResult, FormatterPort } from "../../ports";
import { PORT_VERSIONS } from "../../ports";

/** Extensions Biome formats here; everything else is a no-spawn passthrough. */
const BIOME_EXTENSIONS = new Set([
  "ts",
  "tsx",
  "mts",
  "cts",
  "js",
  "jsx",
  "mjs",
  "cjs",
  "json",
  "jsonc",
]);

/**
 * platform → arch → candidate binary specifiers, tried in order (linux lists
 * the glibc build before the musl one — whichever the install laid down
 * resolves). Mirrors the platform map in Biome's own bin shim.
 */
const PLATFORM_BINARIES: Readonly<Record<string, Readonly<Record<string, readonly string[]>>>> = {
  win32: {
    x64: ["@biomejs/cli-win32-x64/biome.exe"],
    arm64: ["@biomejs/cli-win32-arm64/biome.exe"],
  },
  darwin: {
    x64: ["@biomejs/cli-darwin-x64/biome"],
    arm64: ["@biomejs/cli-darwin-arm64/biome"],
  },
  linux: {
    x64: ["@biomejs/cli-linux-x64/biome", "@biomejs/cli-linux-x64-musl/biome"],
    arm64: ["@biomejs/cli-linux-arm64/biome", "@biomejs/cli-linux-arm64-musl/biome"],
  },
};

/** A formatter that outlives this has hung; the write must not hang with it. */
const FORMAT_TIMEOUT_MS = 10_000;

export interface BiomeFormatterOptions {
  /** Workspace root — the spawn cwd, so Biome's config discovery matches the user's. */
  readonly root: string;
  /** Explicit binary path: skips platform resolution (test seam). */
  readonly binary?: string;
}

export class BiomeFormatter implements FormatterPort {
  readonly version = PORT_VERSIONS.formatter;
  readonly #root: string;
  readonly #explicitBinary: string | undefined;
  /** Memoized platform resolution: undefined = untried, null = unresolvable. */
  #resolved: string | null | undefined;

  constructor(options: BiomeFormatterOptions) {
    this.#root = options.root;
    this.#explicitBinary = options.binary;
  }

  async format(request: FormatRequest): Promise<FormatResult> {
    const ext = request.path.slice(request.path.lastIndexOf(".") + 1).toLowerCase();
    if (!BIOME_EXTENSIONS.has(ext)) return { content: request.content, changed: false };
    const binary = this.#binary();
    if (binary === null) {
      throw new Error("the Biome platform binary is not installed for this platform/arch");
    }
    const content = await runBiomeFormat(binary, this.#root, request);
    return { content, changed: content !== request.content };
  }

  #binary(): string | null {
    if (this.#explicitBinary !== undefined) return this.#explicitBinary;
    if (this.#resolved !== undefined) return this.#resolved;
    const require = createRequire(import.meta.url);
    this.#resolved = null;
    for (const specifier of PLATFORM_BINARIES[process.platform]?.[process.arch] ?? []) {
      try {
        this.#resolved = require.resolve(specifier);
        break;
      } catch {
        // Try the next candidate; null stands if none resolve.
      }
    }
    return this.#resolved;
  }
}

function runBiomeFormat(binary: string, cwd: string, request: FormatRequest): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, ["format", `--stdin-file-path=${request.path}`], {
      cwd,
      stdio: ["pipe", "pipe", "pipe"],
      timeout: FORMAT_TIMEOUT_MS,
      windowsHide: true,
    });
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    child.stdout.on("data", (chunk: Buffer) => stdout.push(chunk));
    child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));
    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (code === 0) {
        resolve(Buffer.concat(stdout).toString("utf8"));
        return;
      }
      const detail = Buffer.concat(stderr).toString("utf8").trim().split("\n")[0] ?? "";
      reject(
        new Error(
          signal !== null
            ? `biome format was killed by ${signal} after ${FORMAT_TIMEOUT_MS}ms`
            : `biome format exited ${code}${detail === "" ? "" : `: ${detail}`}`,
        ),
      );
    });
    // EPIPE from a child that died before draining stdin; `close` reports it.
    child.stdin.on("error", () => {});
    child.stdin.end(request.content, "utf8");
  });
}
