/**
 * Doctor suite (feature 004-doctor-bundles, chunk:m6-doctor):
 *
 *  - ustar conformance: the bundle parses with an INDEPENDENT minimal tar
 *    reader (header fields, checksum, offsets) — nothing from src/doctor.
 *  - report sections: all six present with sources; quarantined files,
 *    breakered plugins, and per-key config provenance all visible (the
 *    chunk's acceptance criteria 2 and 3).
 *  - privacy: a sentinel planted in workspace files never reaches any
 *    bundle member; the workspace root is redacted to <workspace>.
 *  - CLI: exit 0 healthy / 1 findings / 64 refused --out; `doctor config`
 *    unchanged (FR-009); read-only beside a simulated live server (US4).
 *  - timing: a cold report stays under the SC-003 budget.
 */

import { Database } from "bun:sqlite";
import { describe, expect, test } from "bun:test";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { buildBundle, redact } from "../src/doctor/bundle";
import { runDoctor } from "../src/doctor/probes";
import { renderDoctorReport } from "../src/doctor/report";
import { tarArchive } from "../src/doctor/tar";

const MAIN = path.resolve(import.meta.dir, "../src/main.ts");

function makeWorkspace(): string {
  const root = mkdtempSync(path.join(tmpdir(), "doctor-ws-"));
  writeFileSync(path.join(root, "a.ts"), "export const a = 1;\n");
  return root;
}

async function doctorRun(root: string, sessionFlags: Record<string, unknown> = {}) {
  return runDoctor({
    root,
    env: process.env as Record<string, string | undefined>,
    sessionFlags,
    toolVersion: "0.0.0-test",
    now: () => new Date("2026-08-22T00:00:00.000Z"),
  });
}

/** Independent minimal ustar reader — deliberately NOT the writer's inverse. */
function readTar(bytes: Uint8Array): Map<string, Uint8Array> {
  const members = new Map<string, Uint8Array>();
  const decoder = new TextDecoder();
  let offset = 0;
  while (offset + 512 <= bytes.length) {
    const header = bytes.subarray(offset, offset + 512);
    if (header.every((b) => b === 0)) break; // end-of-archive zero block
    const name = decoder.decode(header.subarray(0, 100)).replace(/\0.*$/, "");
    const size = Number.parseInt(decoder.decode(header.subarray(124, 136)).replace(/\0.*$/, ""), 8);
    const storedChecksum = Number.parseInt(
      decoder.decode(header.subarray(148, 156)).replace(/[\0 ].*$/, ""),
      8,
    );
    let sum = 0;
    for (let i = 0; i < 512; i++) {
      const inChecksumField = i >= 148 && i < 156;
      sum += inChecksumField ? 0x20 : (header[i] as number);
    }
    expect(sum).toBe(storedChecksum);
    expect(decoder.decode(header.subarray(257, 262))).toBe("ustar");
    members.set(name, bytes.subarray(offset + 512, offset + 512 + size));
    offset += 512 + Math.ceil(size / 512) * 512;
  }
  return members;
}

function gunzip(bytes: Uint8Array<ArrayBuffer>): Uint8Array {
  return Bun.gunzipSync(bytes);
}

function seedStateDir(root: string): string {
  const stateDir = path.join(root, ".workbench");
  mkdirSync(stateDir, { recursive: true });
  // A valid empty-schema index.db (the shape ChildIndexer stamps).
  const db = new Database(path.join(stateDir, "index.db"));
  db.run("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
  db.run("INSERT INTO meta (key, value) VALUES ('schema_version', '1'), ('generation', '3')");
  db.run("CREATE TABLE files (path TEXT PRIMARY KEY)");
  db.run("CREATE TABLE symbols (path TEXT NOT NULL, name TEXT NOT NULL)");
  db.run("PRAGMA user_version = 1");
  db.close();
  writeFileSync(
    path.join(stateDir, "quarantine.jsonl"),
    `${JSON.stringify({ path: "weird.bin", strikes: 3, quarantined: true, lastCrash: "SIGSEGV", at: 1755000000000 })}\n` +
      "this line is torn and not json\n",
  );
  writeFileSync(
    path.join(stateDir, "plugin-health.json"),
    `${JSON.stringify({ breaches: [{ plugin: "acme.bad", componentHash: "deadbeef", reason: "guest trap", at: "2026-08-21T00:00:00Z" }] }, null, 2)}\n`,
  );
  writeFileSync(
    path.join(root, ".workbench", "plugins.json"),
    `${JSON.stringify({ plugins: [{ name: "acme.bad", source: "sideload" }] }, null, 2)}\n`,
  );
  return stateDir;
}

describe("doctor report (US2)", () => {
  test("all six sections, sources, provenance, quarantine + breaker visibility", async () => {
    const root = makeWorkspace();
    seedStateDir(root);
    const run = await doctorRun(root, { "budgets.default": 9999 });
    const ids = run.report.sections.map((s) => s.id);
    expect(ids).toEqual(["environment", "index", "config", "plugins", "quarantine", "crashes"]);
    for (const section of run.report.sections) {
      expect(section.source.length).toBeGreaterThan(0);
    }
    const text = renderDoctorReport(run);
    // Chunk AC2: per-key layer attribution, embedded renderDoctorConfig.
    expect(text).toContain("effective config (value ← deciding layer):");
    expect(text).toContain("budgets.default = 9999  (session)");
    // Chunk AC3: quarantined file and unhealthy plugin, with recovery rules.
    expect(text).toContain("weird.bin quarantined after 3 strike(s)");
    expect(text).toContain("INPUT_QUARANTINED clears when the file changes");
    expect(text).toContain("acme.bad breakered since");
    expect(text).toContain("changed component hash clears the breaker");
    // Healthy index seeded above; crash evidence derived from strikes.
    expect(text).toContain("schema 1, quick_check ok");
    expect(text).toContain("crash strike record(s)");
    // Unhealthy findings present ⇒ exit-1 signal.
    expect(run.report.findingsPresent).toBe(true);
    // Every unhealthy finding carries a remediation (FR-007).
    for (const section of run.report.sections) {
      for (const finding of section.findings) {
        if (finding.severity === "unhealthy") {
          expect(finding.remediation ?? "").not.toBe("");
        }
      }
    }
    // Malformed quarantine line tolerated, counted.
    const quarantine = run.report.sections.find((s) => s.id === "quarantine");
    expect(quarantine).toBeDefined();
    expect((quarantine?.detail as { malformedLines: number } | undefined)?.malformedLines).toBe(1);
  });

  test("fresh workspace: no index is a normal finding, exit signal healthy", async () => {
    const root = makeWorkspace();
    const run = await doctorRun(root);
    expect(run.report.findingsPresent).toBe(false);
    const index = run.report.sections.find((s) => s.id === "index");
    expect(index?.findings[0]?.severity).toBe("info");
    expect(index?.findings[0]?.symptom).toContain("no index yet");
    // The probe must not create the state dir (read-only, FR-005).
    expect(existsSync(path.join(root, ".workbench"))).toBe(false);
  });

  test("cold report stays inside the SC-003 budget", async () => {
    const root = makeWorkspace();
    const startedAt = performance.now();
    await doctorRun(root);
    expect(performance.now() - startedAt).toBeLessThan(5000);
  });
});

describe("doctor bundle (US1/US3)", () => {
  test("ustar conformance via an independent reader; manifest lists every member", async () => {
    const root = makeWorkspace();
    seedStateDir(root);
    const bundle = buildBundle(await doctorRun(root));
    const members = readTar(gunzip(bundle.bytes));
    expect([...members.keys()].sort()).toEqual([...bundle.members].sort());
    const manifest = JSON.parse(
      new TextDecoder().decode(members.get("workbench-doctor/manifest.json")),
    ) as { bundleFormatVersion: number; members: string[]; workspaceLabel: string };
    expect(manifest.bundleFormatVersion).toBe(1);
    expect(manifest.workspaceLabel).toBe("<workspace>");
    expect([...members.keys()].sort()).toEqual([...manifest.members].sort());
  });

  test("privacy: sentinel never leaks, workspace root redacted (SC-002)", async () => {
    const root = makeWorkspace();
    const sentinel = "SENTINEL-do-not-leak-9f3a";
    writeFileSync(path.join(root, "secret.ts"), `export const s = "${sentinel}";\n`);
    seedStateDir(root);
    const bundle = buildBundle(await doctorRun(root));
    const members = readTar(gunzip(bundle.bytes));
    const decoder = new TextDecoder();
    for (const [name, data] of members) {
      const body = decoder.decode(data);
      expect(body).not.toContain(sentinel);
      expect(body).not.toContain(root);
      expect(body).not.toContain(root.replaceAll("\\", "/"));
      expect(name.startsWith("workbench-doctor/")).toBe(true);
    }
    expect(decoder.decode(members.get("workbench-doctor/report.txt"))).toContain("<workspace>");
  });

  test("redact reduces foreign absolute paths to placeholders", () => {
    const out = redact(
      'seen at C:\\Users\\someone\\secrets\\token.txt and /home/someone/key.pem inside <ws>"',
      "D:\\nowhere\\ws",
    );
    expect(out).toContain("<outside-workspace>/token.txt");
    expect(out).toContain("<outside-workspace>/key.pem");
    expect(out).not.toContain("C:\\Users\\someone");
    expect(out).not.toContain("/home/someone");
  });

  test("tar writer round-trips arbitrary member bytes", () => {
    const data = new Uint8Array(700).map((_, i) => i % 251);
    const members = readTar(tarArchive([{ name: "workbench-doctor/x.bin", data }], 1700000000));
    expect([...(members.get("workbench-doctor/x.bin") ?? [])]).toEqual([...data]);
  });
});

describe("doctor CLI (US4 + exit codes)", () => {
  function cli(args: string[], cwd: string) {
    return Bun.spawnSync([process.execPath, MAIN, ...args], {
      cwd,
      stdout: "pipe",
      stderr: "pipe",
    });
  }

  test("read-only beside a live server; exit codes per contract", () => {
    const root = makeWorkspace();
    seedStateDir(root);
    // Simulated live server: the advisory lock is present.
    writeFileSync(path.join(root, ".workbench", "server.lock"), String(process.pid));
    const snapshot = () => {
      const stateDir = path.join(root, ".workbench");
      const out = new Map<string, string>();
      for (const name of readdirSync(stateDir)) {
        const full = path.join(stateDir, name);
        if (statSync(full).isFile()) {
          out.set(name, Bun.hash(readFileSync(full)).toString());
        }
      }
      return out;
    };
    const before = snapshot();
    // Findings are seeded (quarantine + breach) ⇒ exit 1, report still full.
    const report = cli(["doctor", root], root);
    expect(report.exitCode).toBe(1);
    expect(report.stdout.toString()).toContain("[quarantine]");
    const target = path.join(root, "..", `bundle-${path.basename(root)}.tar.gz`);
    const bundle = cli(["doctor", "bundle", root, "--out", target], root);
    expect(bundle.exitCode).toBe(1);
    expect(existsSync(target)).toBe(true);
    // Refuse to overwrite a prior diagnostic.
    const again = cli(["doctor", "bundle", root, "--out", target], root);
    expect(again.exitCode).toBe(64);
    // State dir byte-identical after both invocations (FR-005/SC-004).
    expect(snapshot()).toEqual(before);
  });

  test("healthy workspace exits 0; doctor config unchanged (FR-009)", () => {
    const root = makeWorkspace();
    const healthy = cli(["doctor", root], root);
    expect(healthy.exitCode).toBe(0);
    const config = cli(["doctor", "config", root], root);
    expect(config.exitCode).toBe(0);
    expect(config.stdout.toString()).toContain("workbench doctor config — workspace");
    expect(config.stdout.toString()).toContain("effective config (value ← deciding layer):");
    const usage = cli(["doctor", root, "--out", "x.tar.gz"], root);
    expect(usage.exitCode).toBe(64); // --out is bundle-only
  });
});

describe("doctor watcher line (feature 011, SC-004)", () => {
  function record(overrides: Record<string, unknown>): string {
    return `${JSON.stringify({
      v: 1,
      pid: process.pid,
      mode: "auto",
      adapter: "fs.watch",
      root: "/ws",
      startedAtMs: 1_700_000_000_000,
      updatedAtMs: 1_700_000_001_000,
      events: 12,
      refreshes: 10,
      removals: 2,
      failures: 0,
      lastEventAtMs: 1_700_000_000_900,
      ...overrides,
    })}\n`;
  }

  test("not active when no record exists", async () => {
    const root = makeWorkspace();
    const run = await doctorRun(root);
    expect(renderDoctorReport(run)).toContain("watcher not active: no watcher record");
  });

  test("live when the record's pid is alive; last session after stop or when the pid is gone", async () => {
    const root = makeWorkspace();
    const stateDir = path.join(root, ".workbench");
    mkdirSync(stateDir, { recursive: true });
    writeFileSync(path.join(stateDir, "watcher.json"), record({}));
    let text = renderDoctorReport(await doctorRun(root));
    expect(text).toContain(`watcher live: fs.watch (mode auto) in pid ${process.pid}`);
    expect(text).toContain("12 event(s), 10 refresh(es), 2 removal(s), 0 failure(s)");
    writeFileSync(path.join(stateDir, "watcher.json"), record({ stoppedAtMs: 1_700_000_002_000 }));
    text = renderDoctorReport(await doctorRun(root));
    expect(text).toContain("watcher last session: fs.watch (mode auto)");
    expect(text).toContain("stopped 2023-11-14T22:13:22.000Z");
    writeFileSync(
      path.join(stateDir, "watcher.json"),
      record({ pid: 999_999_9, adapter: "polling" }),
    );
    text = renderDoctorReport(await doctorRun(root));
    expect(text).toContain(
      "watcher last session: polling (mode auto) in pid 9999999, process gone",
    );
  });

  test("unreadable is a named info finding, never a crash; the report still renders", async () => {
    const root = makeWorkspace();
    const stateDir = path.join(root, ".workbench");
    mkdirSync(stateDir, { recursive: true });
    writeFileSync(path.join(stateDir, "watcher.json"), "{nope");
    const run = await doctorRun(root);
    const text = renderDoctorReport(run);
    expect(text).toContain("watcher record unreadable (.workbench/watcher.json)");
    const env = run.report.sections.find((s) => s.id === "environment");
    expect(env?.findings.some((f) => f.store === "watcher.json" && f.severity === "info")).toBe(
      true,
    );
    // Cache-class: never an unhealthy finding on its own.
    expect(env?.findings.every((f) => f.severity !== "unhealthy")).toBe(true);
  });
});
