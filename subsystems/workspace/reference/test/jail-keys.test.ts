/**
 * Per-workspace key lifecycle (SDD §7.1): generation at init, 0600
 * permissions, rotation, and the MRTR confirm-token HMAC built on it.
 *
 * The 0600/0700 assertions are POSIX facts and run only there; Windows ACLs
 * do not speak POSIX mode bits, so asserting them on NTFS would test the
 * emulation, not the security property. The mode is *requested* on every
 * platform either way (see keys.ts).
 */

import { describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  confirmToken,
  ensureWorkspaceKey,
  KEY_LENGTH,
  keyPath,
  rotateWorkspaceKey,
  verifyConfirmToken,
} from "../src/jail";

const POSIX = process.platform !== "win32";

function tmpStateDir(): string {
  return mkdtempSync(path.join(tmpdir(), "wb-keys-"));
}

describe("workspace key generation", () => {
  test("first use generates 32 bytes under keys/workspace.key", () => {
    const stateDir = tmpStateDir();
    const key = ensureWorkspaceKey(stateDir);
    expect(key.length).toBe(KEY_LENGTH);
    expect(existsSync(keyPath(stateDir))).toBe(true);
    expect(readFileSync(keyPath(stateDir)).equals(key)).toBe(true);
  });

  test("second use loads the same key, not a fresh one", () => {
    const stateDir = tmpStateDir();
    const first = ensureWorkspaceKey(stateDir);
    const second = ensureWorkspaceKey(stateDir);
    expect(second.equals(first)).toBe(true);
  });

  test("a malformed key file throws instead of silently regenerating", () => {
    const stateDir = tmpStateDir();
    ensureWorkspaceKey(stateDir);
    writeFileSync(keyPath(stateDir), Buffer.alloc(5));
    expect(() => ensureWorkspaceKey(stateDir)).toThrow(/expected 32/);
  });

  test.skipIf(!POSIX)("the key file is 0600 and the keys dir is 0700 (POSIX)", () => {
    const stateDir = tmpStateDir();
    ensureWorkspaceKey(stateDir);
    expect(statSync(keyPath(stateDir)).mode & 0o777).toBe(0o600);
    expect(statSync(path.dirname(keyPath(stateDir))).mode & 0o777).toBe(0o700);
  });
});

describe("workspace key rotation", () => {
  test("rotation replaces the key with fresh 32 bytes", () => {
    const stateDir = tmpStateDir();
    const before = ensureWorkspaceKey(stateDir);
    const after = rotateWorkspaceKey(stateDir);
    expect(after.length).toBe(KEY_LENGTH);
    expect(after.equals(before)).toBe(false);
    expect(readFileSync(keyPath(stateDir)).equals(after)).toBe(true);
  });

  test.skipIf(!POSIX)("the rotated key file keeps 0600 (POSIX)", () => {
    const stateDir = tmpStateDir();
    ensureWorkspaceKey(stateDir);
    rotateWorkspaceKey(stateDir);
    expect(statSync(keyPath(stateDir)).mode & 0o777).toBe(0o600);
  });

  test("rotation voids every outstanding confirm token", () => {
    const stateDir = tmpStateDir();
    const before = ensureWorkspaceKey(stateDir);
    const token = confirmToken(before, "diff-hash", "content-hash");
    const after = rotateWorkspaceKey(stateDir);
    expect(verifyConfirmToken(after, "diff-hash", "content-hash", token)).toBe(false);
  });
});

describe("MRTR confirm token", () => {
  const key = Buffer.alloc(KEY_LENGTH, 42);

  test("deterministic hex HMAC over (diff hash, content hash)", () => {
    const a = confirmToken(key, "d1", "c1");
    expect(a).toBe(confirmToken(key, "d1", "c1"));
    expect(a).toMatch(/^[0-9a-f]{64}$/);
  });

  test("verifies the exact (diff, content) pair", () => {
    const token = confirmToken(key, "d1", "c1");
    expect(verifyConfirmToken(key, "d1", "c1", token)).toBe(true);
  });

  test("a stale anchor fails closed: changed content hash rejects", () => {
    const token = confirmToken(key, "d1", "c1");
    expect(verifyConfirmToken(key, "d1", "c-moved", token)).toBe(false);
  });

  test("a changed diff hash rejects", () => {
    const token = confirmToken(key, "d1", "c1");
    expect(verifyConfirmToken(key, "d-other", "c1", token)).toBe(false);
  });

  test("tampered, truncated, garbage and empty tokens are false, never throws", () => {
    const token = confirmToken(key, "d1", "c1");
    const flipped = (token[0] === "0" ? "1" : "0") + token.slice(1);
    expect(verifyConfirmToken(key, "d1", "c1", flipped)).toBe(false);
    expect(verifyConfirmToken(key, "d1", "c1", token.slice(0, 32))).toBe(false);
    expect(verifyConfirmToken(key, "d1", "c1", "not-hex-at-all")).toBe(false);
    expect(verifyConfirmToken(key, "d1", "c1", "")).toBe(false);
  });

  test("the ambient separator cannot be confused: (ab, c) != (a, bc)", () => {
    expect(confirmToken(key, "ab", "c")).not.toBe(confirmToken(key, "a", "bc"));
  });
});
