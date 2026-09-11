/**
 * Protected paths (SDD §7.1): `.git/**` (write), `.workbench/**`, and the
 * workspace config layer are read-only to the write engine, the sandbox, and
 * every plugin at every trust tier. The server will not edit its own
 * configuration, keys, or history — so an injected agent or malicious plugin
 * cannot amend the `run` allowlist, plant a git hook, or rewrite the MRTR
 * key it would need to forge confirm tokens.
 *
 * Config layers above the workspace (user config, org policy, §7.2) live
 * outside the workspace root, so the prefix check in `paths.ts` already
 * refuses them; this module only needs to protect what sits *inside* the
 * jail. Comparison is case-insensitive on every platform: on Windows `.GIT`
 * *is* `.git`, and on POSIX protecting a same-name-different-case cousin is
 * harmless overprotection, never a hole.
 */

import { sep } from "node:path";
import { canonicalizeWithin, type JailDecision } from "./paths";

/** Directories under the root that no mutating surface may write into. */
const PROTECTED_DIRS = [".git", ".workbench"] as const;

/** Files directly under the root that no mutating surface may write. */
const PROTECTED_FILES = ["workbench.toml"] as const;

/**
 * Decide a WRITE to `candidate`. Admission means the canonical path is
 * inside the workspace and outside every protected region; denial carries
 * `PATH_OUTSIDE_WORKSPACE` or `PROTECTED_PATH`.
 */
export function checkWrite(root: string, candidate: string): JailDecision {
  const decision = canonicalizeWithin(root, candidate);
  if (!decision.ok) return decision;
  const relative = relativeToRoot(root, decision.path);
  const lower = relative.toLowerCase();
  const segments = lower.split(sep);
  const first = segments[0] ?? "";
  for (const dir of PROTECTED_DIRS) {
    if (first === dir) {
      return {
        ok: false,
        code: "PROTECTED_PATH",
        reason: `${dir}/** is read-only to every mutating surface (SDD §7.1)`,
      };
    }
  }
  for (const file of PROTECTED_FILES) {
    if (lower === file) {
      return {
        ok: false,
        code: "PROTECTED_PATH",
        reason: `${file} is a config layer and read-only to every mutating surface (SDD §7.1)`,
      };
    }
  }
  return decision;
}

/**
 * Decide a READ of `candidate`. Protection is write-only — reads inside the
 * workspace are admitted everywhere, including `.git/**`.
 */
export function checkRead(root: string, candidate: string): JailDecision {
  return canonicalizeWithin(root, candidate);
}

/** The canonical path relative to the canonical root ("" for the root). */
function relativeToRoot(root: string, canonical: string): string {
  const decision = canonicalizeWithin(root, ".");
  const rootReal = decision.ok ? decision.path : root;
  if (canonical.length <= rootReal.length) return "";
  return canonical.slice(rootReal.length + 1);
}
