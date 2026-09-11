/**
 * Workspace runtime (SDD §5.2): advisory lock, network-FS detection, and the
 * measured journal-mode flip. The flip assertions read PRAGMA journal_mode
 * back from the real workspace db — never the config value that requested it.
 */

import { describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { hostname, tmpdir } from "node:os";
import path from "node:path";
import { statusEnvelope } from "../src/protocol/server";
import {
  detectNetworkFs,
  openWorkspace,
  WorkspaceLockedError,
  type WorkspaceRuntime,
} from "../src/protocol/workspace";

const LOCAL_ENV = { WORKBENCH_NETWORK_FS: "0" };
const NETWORK_ENV = { WORKBENCH_NETWORK_FS: "1" };

function tmpWorkspace(): string {
  return mkdtempSync(path.join(tmpdir(), "workbench-ws-"));
}

function withWorkspace(
  env: Record<string, string | undefined>,
  run: (workspace: WorkspaceRuntime, root: string) => void,
): void {
  const root = tmpWorkspace();
  const workspace = openWorkspace(root, env);
  try {
    run(workspace, root);
  } finally {
    workspace.close();
    rmSync(root, { recursive: true, force: true });
  }
}

describe("journal mode", () => {
  test("local workspace runs WAL and carries no degradation notes", () => {
    withWorkspace(LOCAL_ENV, (workspace) => {
      expect(workspace.journalMode).toBe("WAL");
      expect(workspace.appliedJournalMode).toBe("wal");
      expect(workspace.notes).toHaveLength(0);
    });
  });

  test("network filesystem flips WAL→TRUNCATE, measured via PRAGMA", () => {
    withWorkspace(NETWORK_ENV, (workspace) => {
      expect(workspace.journalMode).toBe("TRUNCATE");
      expect(workspace.appliedJournalMode).toBe("truncate");
      expect(workspace.notes.some((n) => n.includes("WAL→TRUNCATE"))).toBe(true);
    });
  });

  test("degradation note rides the status envelope — ok and error branches", () => {
    withWorkspace(NETWORK_ENV, (workspace) => {
      const ok = statusEnvelope(workspace);
      expect(ok.ok).toBe(true);
      expect(ok.notes?.some((n) => n.includes("network filesystem"))).toBe(true);

      const refused = statusEnvelope(workspace, { "workbench/maxEnvelopeVersion": "garbled" });
      expect(refused.ok).toBe(false);
      if (!refused.ok) expect(refused.error.code).toBe("UNSUPPORTED_VERSION");
      expect(refused.notes?.some((n) => n.includes("network filesystem"))).toBe(true);
    });
  });
});

describe("network-FS detection", () => {
  test("UNC paths are network without any override", () => {
    expect(detectNetworkFs("\\\\fileserver\\share\\repo", {})).toEqual({
      networkFs: true,
      method: "unc-path",
    });
  });

  test("env override wins in both directions", () => {
    expect(detectNetworkFs("\\\\fileserver\\share\\repo", LOCAL_ENV).networkFs).toBe(false);
    expect(detectNetworkFs(path.resolve("."), NETWORK_ENV).networkFs).toBe(true);
  });
});

describe("advisory lock", () => {
  test("a second instance on the same workspace is refused", () => {
    const root = tmpWorkspace();
    const first = openWorkspace(root, LOCAL_ENV);
    try {
      expect(() => openWorkspace(root, LOCAL_ENV)).toThrow(WorkspaceLockedError);
    } finally {
      first.close();
    }
    // Released lock admits the next instance.
    const second = openWorkspace(root, LOCAL_ENV);
    second.close();
    rmSync(root, { recursive: true, force: true });
  });

  test("a stale lock from a dead pid on this host is broken", () => {
    const root = tmpWorkspace();
    // A real pid that is certainly dead: a spawned process that already exited.
    const dead = Bun.spawnSync({ cmd: [process.execPath, "-e", ""] });
    const stalePid = dead.pid;
    const workbench = path.join(root, ".workbench");
    openWorkspace(root, LOCAL_ENV).close(); // create .workbench, then release
    writeFileSync(
      path.join(workbench, "server.lock"),
      JSON.stringify({ pid: stalePid, host: hostname() }),
    );
    const workspace = openWorkspace(root, LOCAL_ENV);
    workspace.close();
    rmSync(root, { recursive: true, force: true });
  });
});
