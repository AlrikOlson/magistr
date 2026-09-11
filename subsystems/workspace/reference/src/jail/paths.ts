/**
 * Path discipline (SDD §7.1): every path from any tool argument — or any
 * plugin capability call — is canonicalized (symlinks resolved) and
 * prefix-checked against the workspace root. Nothing reaches the filesystem
 * except through a decision made here.
 *
 * The order of operations is what makes this a jail rather than a filter:
 * the candidate is resolved lexically against the root, then the deepest
 * *existing* ancestor is passed through `realpath` so symlinks cannot smuggle
 * the path outside after the check, and only the canonical result is
 * prefix-compared. Encoded traversals (`%2e%2e`, fullwidth dots, `....//`)
 * are never decoded — they stay ordinary file names inside the root, which
 * is the correct defense: decoding on behalf of an attacker is how jails
 * acquire second parsers.
 */

import { realpathSync } from "node:fs";
import { basename, dirname, resolve, sep } from "node:path";

/** The two §4.3 codes a jail decision can carry. */
export type JailDenialCode = "PATH_OUTSIDE_WORKSPACE" | "PROTECTED_PATH";

export interface JailDenial {
  readonly ok: false;
  readonly code: JailDenialCode;
  readonly reason: string;
}

export interface JailAdmission {
  readonly ok: true;
  /** Canonical absolute path: symlinks resolved, inside the workspace root. */
  readonly path: string;
}

export type JailDecision = JailAdmission | JailDenial;

function denial(code: JailDenialCode, reason: string): JailDenial {
  return { ok: false, code, reason };
}

/** Windows filesystems compare case-insensitively; the jail must too. */
const CASE_FOLD = process.platform === "win32";

function fold(path: string): string {
  return CASE_FOLD ? path.toLowerCase() : path;
}

/** True when `path` is `root` itself or sits underneath it. */
function isWithin(root: string, path: string): boolean {
  const foldedRoot = fold(root);
  const foldedPath = fold(path);
  return foldedPath === foldedRoot || foldedPath.startsWith(foldedRoot + sep);
}

/**
 * Resolve the deepest existing ancestor of `absPath` through realpath and
 * re-attach the not-yet-existing remainder. Write targets usually do not
 * exist yet; their canonical location is decided by where their nearest
 * existing ancestor really lives.
 */
function canonicalizeExistingPrefix(absPath: string): string {
  let existing = absPath;
  const remainder: string[] = [];
  for (;;) {
    try {
      const real = realpathSync(existing);
      return remainder.length === 0 ? real : resolve(real, ...remainder);
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code;
      if (code !== "ENOENT" && code !== "ENOTDIR") throw error;
      const parent = dirname(existing);
      if (parent === existing) throw error;
      remainder.unshift(basename(existing));
      existing = parent;
    }
  }
}

/**
 * Canonicalize `candidate` (relative to `root`, or absolute) and admit it
 * only if the canonical result stays inside the workspace root. Denials are
 * `PATH_OUTSIDE_WORKSPACE` — including symlink loops, whose real target is
 * unknowable and therefore not provably inside.
 */
export function canonicalizeWithin(root: string, candidate: string): JailDecision {
  if (candidate.includes("\0")) {
    return denial("PATH_OUTSIDE_WORKSPACE", "path contains a NUL byte");
  }
  let rootReal: string;
  try {
    rootReal = realpathSync(root);
  } catch {
    return denial("PATH_OUTSIDE_WORKSPACE", `workspace root does not exist: ${root}`);
  }
  let canonical: string;
  try {
    canonical = canonicalizeExistingPrefix(resolve(rootReal, candidate));
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code;
    return denial(
      "PATH_OUTSIDE_WORKSPACE",
      code === "ELOOP"
        ? `symlink loop while canonicalizing ${candidate}`
        : `cannot canonicalize ${candidate}`,
    );
  }
  if (!isWithin(rootReal, canonical)) {
    return denial("PATH_OUTSIDE_WORKSPACE", `${candidate} resolves outside the workspace root`);
  }
  return { ok: true, path: canonical };
}
