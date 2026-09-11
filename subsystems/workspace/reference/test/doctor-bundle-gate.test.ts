/**
 * The M6 release-gate scenario (SDD §11.3, chunk:m6-doctor AC1, SC-001):
 * a seeded corrupt-index workspace is diagnosed and RESOLVED from a doctor
 * bundle alone.
 *
 * The support engineer is encoded as code that sees ONLY the extracted
 * bundle members — the derivation step takes the member map and nothing
 * else, so "no repo access" is structural, not procedural. The remediation
 * it derives is applied mechanically (the store path comes out of the
 * bundle's own text), and recovery plus zero user-data loss are verified.
 */

import { Database } from "bun:sqlite";
import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const MAIN = path.resolve(import.meta.dir, "../src/main.ts");

/** Independent minimal ustar reader (headers + offsets), like doctor.test.ts. */
function readTar(bytes: Uint8Array): Map<string, string> {
  const members = new Map<string, string>();
  const decoder = new TextDecoder();
  let offset = 0;
  while (offset + 512 <= bytes.length) {
    const header = bytes.subarray(offset, offset + 512);
    if (header.every((b) => b === 0)) break;
    const name = decoder.decode(header.subarray(0, 100)).replace(/\0.*$/, "");
    const size = Number.parseInt(decoder.decode(header.subarray(124, 136)).replace(/\0.*$/, ""), 8);
    members.set(name, decoder.decode(bytes.subarray(offset + 512, offset + 512 + size)));
    offset += 512 + Math.ceil(size / 512) * 512;
  }
  return members;
}

/**
 * The support-engineer step: input is the member map ONLY. Returns the
 * workspace-relative path to remove, derived from the bundle's own
 * remediation text, after checking the finding says this is safe
 * (rebuildable cache data).
 */
function deriveRemediation(members: Map<string, string>): string {
  const index = JSON.parse(members.get("workbench-doctor/index.json") ?? "{}") as {
    findings: {
      store: string;
      dataClass: string;
      severity: string;
      symptom: string;
      remediation?: string;
    }[];
  };
  const finding = index.findings.find((f) => f.severity === "unhealthy");
  expect(finding).toBeDefined();
  if (finding === undefined) throw new Error("unreachable");
  // The bundle names the store, its data class, and the remediation (FR-006).
  expect(finding.store).toBe("index.db");
  expect(finding.dataClass).toBe("cache");
  expect(finding.remediation ?? "").toContain("rebuild");
  const match = (finding.remediation ?? "").match(/\.workbench\/[\w./-]+/);
  expect(match).not.toBeNull();
  if (match === null) throw new Error("unreachable");
  return match[0];
}

describe("M6 gate — corrupt index resolved from the bundle alone", () => {
  test("seed → corrupt → bundle → derive from members only → apply → recovered", () => {
    const root = mkdtempSync(path.join(tmpdir(), "doctor-gate-"));
    writeFileSync(path.join(root, "a.ts"), "export const a = 1;\n");
    const stateDir = path.join(root, ".workbench");
    mkdirSync(stateDir, { recursive: true });

    // Durable user data that MUST survive untouched (§10.2).
    const workspaceDb = path.join(stateDir, "workspace.db");
    const journalDb = path.join(stateDir, "journal.db");
    writeFileSync(workspaceDb, "durable-user-data-workspace");
    writeFileSync(journalDb, "durable-user-data-journal");

    // A real index that then gets corrupted on disk.
    const indexPath = path.join(stateDir, "index.db");
    const db = new Database(indexPath);
    db.run("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
    db.run("PRAGMA user_version = 1");
    db.close();
    writeFileSync(indexPath, "garbage-not-a-sqlite-database");

    // Produce the bundle through the real CLI (exit 1: findings present).
    const target = path.join(root, "..", `gate-bundle-${path.basename(root)}.tar.gz`);
    rmSync(target, { force: true });
    const produced = Bun.spawnSync(
      [process.execPath, MAIN, "doctor", "bundle", root, "--out", target],
      { cwd: root, stdout: "pipe", stderr: "pipe" },
    );
    expect(produced.exitCode).toBe(1);

    // ---- Support-engineer boundary: from here, ONLY the bundle. ----
    const members = readTar(Bun.gunzipSync(readFileSync(target)));
    const relativeStorePath = deriveRemediation(members);
    // ---- End boundary: the user applies the instruction locally. ----

    expect(relativeStorePath).toBe(".workbench/index.db");
    rmSync(path.join(root, relativeStorePath));

    // Recovery: the workspace is healthy again (fresh-index state), and the
    // durable stores are byte-identical — zero user-data loss.
    const after = Bun.spawnSync([process.execPath, MAIN, "doctor", root], {
      cwd: root,
      stdout: "pipe",
      stderr: "pipe",
    });
    expect(after.exitCode).toBe(0);
    expect(after.stdout.toString()).toContain("no index yet");
    expect(readFileSync(workspaceDb, "utf8")).toBe("durable-user-data-workspace");
    expect(readFileSync(journalDb, "utf8")).toBe("durable-user-data-journal");
  });
});
