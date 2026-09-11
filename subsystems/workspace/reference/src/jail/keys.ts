/**
 * Per-workspace key management (SDD §7.1): a 32-byte secret generated at
 * init, stored under `.workbench/keys/` with `0600` permissions, used for
 * the MRTR confirm-token HMAC (§9.4), rotatable on demand via
 * `workbench-mcp key rotate`.
 *
 * A confirm token is an HMAC over (diff hash, file content hash) — so a
 * stale confirm fails closed when the file moved under the diff, and no
 * client can mint a token without the workspace key it is jailed away from
 * (`.workbench/**` is write-protected, §7.1).
 *
 * Honesty about 0600: the mode is requested on every platform, and on POSIX
 * it is asserted by tests. Windows ACLs do not speak POSIX mode bits — there
 * the request narrows what it can (read-only bit semantics) and the real
 * boundary is the directory ACL, which is the same honest limit every
 * key-on-disk scheme has on NTFS.
 */

import { createHmac, randomBytes, timingSafeEqual } from "node:crypto";
import { chmodSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export const KEY_LENGTH = 32;
export const KEY_DIR_MODE = 0o700;
export const KEY_FILE_MODE = 0o600;

/** `<stateDir>/keys/workspace.key` */
export function keyPath(stateDir: string): string {
  return join(stateDir, "keys", "workspace.key");
}

/**
 * Load the workspace key, generating it on first use (init). The key file
 * is created with mode 0600 inside a 0700 `keys/` directory. A present but
 * malformed key file throws — a truncated secret is a human problem, and
 * silently regenerating it would orphan every outstanding confirm token
 * without leaving a trace.
 */
export function ensureWorkspaceKey(stateDir: string): Buffer {
  const path = keyPath(stateDir);
  try {
    const key = readFileSync(path);
    if (key.length !== KEY_LENGTH) {
      throw new Error(
        `workspace key at ${path} is ${key.length} bytes, expected ${KEY_LENGTH} — ` +
          "rotate it with `workbench-mcp key rotate`",
      );
    }
    return key;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
  }
  mkdirSync(join(stateDir, "keys"), { recursive: true, mode: KEY_DIR_MODE });
  const key = randomBytes(KEY_LENGTH);
  writeAtomically(path, key);
  return key;
}

/**
 * Replace the workspace key with a fresh 32 bytes. Every outstanding confirm
 * token is invalidated by construction — that is what rotation is for.
 */
export function rotateWorkspaceKey(stateDir: string): Buffer {
  mkdirSync(join(stateDir, "keys"), { recursive: true, mode: KEY_DIR_MODE });
  const key = randomBytes(KEY_LENGTH);
  writeAtomically(keyPath(stateDir), key);
  return key;
}

/** Write via temp-file + rename so a crash never leaves a torn key. */
function writeAtomically(path: string, key: Buffer): void {
  const tmp = `${path}.tmp`;
  writeFileSync(tmp, key, { mode: KEY_FILE_MODE });
  renameSync(tmp, path);
  // Rename preserves the temp file's mode, but be explicit for the replaced-
  // file case where an old mode could survive on some filesystems.
  chmodSync(path, KEY_FILE_MODE);
}

/** The MRTR confirm token: HMAC-SHA256 over (diff hash, content hash). */
export function confirmToken(key: Buffer, diffHash: string, contentHash: string): string {
  return createHmac("sha256", key).update(`${diffHash}\n${contentHash}`).digest("hex");
}

/**
 * The wire confirm grant (§9.4): `wb1.<diffHash>.<contentHash>.<mac>`.
 * Self-describing on purpose — a stateless verifier re-derives the current
 * (diff, content) hashes and can then tell a STALE confirm (valid mac, moved
 * content) from a FORGED one (bad mac), with nothing but the on-disk key.
 * The embedded hashes are hashes of content the client already saw.
 */
export const CONFIRM_GRANT_PREFIX = "wb1";

export function mintConfirmGrant(key: Buffer, diffHash: string, contentHash: string): string {
  return `${CONFIRM_GRANT_PREFIX}.${diffHash}.${contentHash}.${confirmToken(key, diffHash, contentHash)}`;
}

/**
 * Parse and authenticate a wire grant. `{ok: false}` for anything that is
 * not a well-formed grant sealed by `key` — never throws on
 * attacker-controlled input. The caller compares the returned hashes against
 * the current state to decide STALE_ANCHOR vs apply.
 */
export function checkConfirmGrant(
  key: Buffer,
  grant: string,
): { ok: true; diffHash: string; contentHash: string } | { ok: false } {
  const parts = grant.split(".");
  if (parts.length !== 4 || parts[0] !== CONFIRM_GRANT_PREFIX) return { ok: false };
  const diffHash = parts[1] as string;
  const contentHash = parts[2] as string;
  const mac = parts[3] as string;
  if (!verifyConfirmToken(key, diffHash, contentHash, mac)) return { ok: false };
  return { ok: true, diffHash, contentHash };
}

/**
 * Constant-time verification. A token of the wrong shape is simply false —
 * verification must never throw on attacker-controlled input.
 */
export function verifyConfirmToken(
  key: Buffer,
  diffHash: string,
  contentHash: string,
  token: string,
): boolean {
  const expected = Buffer.from(confirmToken(key, diffHash, contentHash), "hex");
  let presented: Buffer;
  try {
    presented = Buffer.from(token, "hex");
  } catch {
    return false;
  }
  return presented.length === expected.length && timingSafeEqual(expected, presented);
}
