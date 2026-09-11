/**
 * Doctor probes (SDD §11.2, feature 004-doctor-bundles): each probe reads
 * one shipped artifact or environment fact, read-only, and yields a section
 * of the report. A probe never repairs, never takes the workspace lock, and
 * never creates a file — a missing or unreadable store is a FINDING, not an
 * error (FR-006). Data-class remediation vocabulary follows SDD §10.2.
 */

import { Database } from "bun:sqlite";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { defaultArtifactsDir, nativeArtifactKey } from "../adapters/core/loader";
import { INDEX_SCHEMA_VERSION } from "../adapters/engine-ipc";
import { readWatcherStatus } from "../adapters/fswatch";
import { renderDoctorConfig, resolveConfig } from "../config";
import { UnsupportedVersionError } from "../data/versioned-db";
import { loadPluginInventory } from "../protocol/server";
import { detectNetworkFs } from "../protocol/workspace";

export type DoctorDataClass = "cache" | "durable" | "n/a";
export type DoctorSeverity = "healthy" | "info" | "unhealthy";

export interface DoctorFinding {
  /** What was probed: `index.db`, `quarantine.jsonl`, `config`, … */
  readonly store: string;
  readonly dataClass: DoctorDataClass;
  readonly severity: DoctorSeverity;
  readonly symptom: string;
  /** Required on every `unhealthy` finding (FR-007). */
  readonly remediation?: string;
}

export interface DoctorSection {
  readonly id: "environment" | "index" | "config" | "plugins" | "quarantine" | "crashes";
  /** The on-disk artifact or mechanism this section was read from (FR-002). */
  readonly source: string;
  readonly findings: readonly DoctorFinding[];
  readonly detail: Record<string, unknown>;
}

export interface DoctorReport {
  readonly workspaceRoot: string;
  readonly generatedAt: string;
  readonly toolVersion: string;
  readonly sections: readonly DoctorSection[];
  /** True when any finding is `unhealthy` — the exit-1 signal (FR-008). */
  readonly findingsPresent: boolean;
}

const healthy = (store: string, symptom: string): DoctorFinding => ({
  store,
  dataClass: "n/a",
  severity: "healthy",
  symptom,
});

/** One human line for the watcher record (feature 011, SC-004). */
function describeWatcher(view: ReturnType<typeof readWatcherStatus>): string {
  switch (view.state) {
    case "live": {
      const r = view.record;
      const last =
        r.lastEventAtMs === undefined
          ? "no events yet"
          : `last event ${new Date(r.lastEventAtMs).toISOString()}`;
      return (
        `watcher live: ${r.adapter} (mode ${r.mode}) in pid ${r.pid} since ` +
        `${new Date(r.startedAtMs).toISOString()}; ${r.events} event(s), ` +
        `${r.refreshes} refresh(es), ${r.removals} removal(s), ${r.failures} failure(s); ${last}`
      );
    }
    case "last-session": {
      const r = view.record;
      const ended =
        r.stoppedAtMs === undefined
          ? "process gone"
          : `stopped ${new Date(r.stoppedAtMs).toISOString()}`;
      return (
        `watcher last session: ${r.adapter} (mode ${r.mode}) in pid ${r.pid}, ${ended}; ` +
        `${r.events} event(s), ${r.failures} failure(s)`
      );
    }
    case "not-active":
      return `watcher not active: ${view.reason}`;
    case "unreadable":
      return (
        `watcher record unreadable (${view.file}): ${view.error} — set the file aside; ` +
        "it is cache-class and rewritten by the next server"
      );
  }
}

function probeEnvironment(root: string, env: Record<string, string | undefined>): DoctorSection {
  const detection = detectNetworkFs(root, env);
  const nativeArtifact = join(defaultArtifactsDir(), `workbench-core.${nativeArtifactKey()}.node`);
  const native = existsSync(nativeArtifact);
  // Feature 011: the server publishes .workbench/watcher.json; doctor reads it
  // (read-only, no lock) and says live / last session / not active / unreadable.
  const watcher = readWatcherStatus(join(root, ".workbench"));
  const detail = {
    networkFs: {
      detected: detection.networkFs,
      method: detection.method,
      ...(detection.fstype === undefined ? {} : { fstype: detection.fstype }),
    },
    expectedJournalMode: detection.networkFs ? "TRUNCATE" : "WAL",
    engine: { native, artifactKey: nativeArtifactKey() },
    watcher,
  };
  const findings: DoctorFinding[] = [
    healthy(
      "environment",
      `filesystem ${detection.networkFs ? "network" : "local"} (via ${detection.method}); ` +
        `engine ${native ? "native binding" : "wasm32 fallback"}`,
    ),
    watcher.state === "unreadable"
      ? {
          store: "watcher.json",
          dataClass: "cache",
          severity: "info",
          symptom: describeWatcher(watcher),
        }
      : healthy("watcher.json", describeWatcher(watcher)),
  ];
  return {
    id: "environment",
    source: "filesystem detection + core artifacts dir + .workbench/watcher.json",
    findings,
    detail,
  };
}

function probeIndex(stateDir: string): DoctorSection {
  const path = join(stateDir, "index.db");
  const source = ".workbench/index.db (read-only)";
  if (!existsSync(path)) {
    return {
      id: "index",
      source,
      findings: [
        {
          store: "index.db",
          dataClass: "cache",
          severity: "info",
          symptom:
            "no index yet (fresh workspace): Tier-0 lexical answers only until a server builds it",
        },
      ],
      detail: { present: false },
    };
  }
  try {
    const db = new Database(path, { readonly: true });
    try {
      const userVersion = (db.query("PRAGMA user_version").get() as { user_version: number })
        .user_version;
      if (userVersion > INDEX_SCHEMA_VERSION) {
        return {
          id: "index",
          source,
          findings: [
            {
              store: "index.db",
              dataClass: "cache",
              severity: "unhealthy",
              symptom: `index schema ${userVersion} is newer than this binary supports (${INDEX_SCHEMA_VERSION})`,
              remediation:
                "upgrade workbench-mcp; a newer binary wrote this index and this one refuses to misread it (SDD §10.2)",
            },
          ],
          detail: { present: true, userVersion },
        };
      }
      const quick = db.query("PRAGMA quick_check").all() as { quick_check: string }[];
      const quickCheck = quick.map((r) => r.quick_check).join("; ");
      const meta: Record<string, string> = {};
      try {
        for (const row of db.query("SELECT key, value FROM meta").all() as {
          key: string;
          value: string;
        }[]) {
          meta[row.key] = row.value;
        }
      } catch {
        // A fresh or pre-meta database: nothing recorded yet.
      }
      const count = (table: string): number | undefined => {
        try {
          return (db.query(`SELECT COUNT(*) AS n FROM ${table}`).get() as { n: number }).n;
        } catch {
          return undefined;
        }
      };
      const detail = {
        present: true,
        userVersion,
        schemaVersion: meta["schema_version"],
        generation: meta["generation"],
        fileCount: count("files"),
        symbolCount: count("symbols"),
        quickCheck,
      };
      if (quickCheck !== "ok") {
        return {
          id: "index",
          source,
          findings: [
            {
              store: "index.db",
              dataClass: "cache",
              severity: "unhealthy",
              symptom: `integrity check failed: ${quickCheck}`,
              remediation:
                "delete .workbench/index.db; the index is rebuildable cache data (SDD §10.2) and the next server start rebuilds it — no user data is involved",
            },
          ],
          detail,
        };
      }
      return {
        id: "index",
        source,
        findings: [healthy("index.db", `schema ${userVersion}, quick_check ok`)],
        detail,
      };
    } finally {
      db.close();
    }
  } catch (error) {
    return {
      id: "index",
      source,
      findings: [
        {
          store: "index.db",
          dataClass: "cache",
          severity: "unhealthy",
          symptom: `unreadable as a database: ${error instanceof Error ? error.message : String(error)}`,
          remediation:
            "delete .workbench/index.db; the index is rebuildable cache data (SDD §10.2) and the next server start rebuilds it — no user data is involved",
        },
      ],
      detail: { present: true },
    };
  }
}

function probeConfig(
  root: string,
  env: Record<string, string | undefined>,
  sessionFlags: Readonly<Record<string, unknown>>,
): { section: DoctorSection; renderedText: string } {
  try {
    const config = resolveConfig({ root, env, sessionFlags });
    const renderedText = renderDoctorConfig(config, root);
    return {
      section: {
        id: "config",
        source:
          "five-layer resolution (defaults → org → user → workbench.toml → session), SDD §7.2",
        findings: [
          healthy(
            "config",
            `${config.trace.length} keys resolved; ${config.warnings.length} warning(s)`,
          ),
        ],
        detail: {
          trace: config.trace.map((entry) => ({
            key: entry.key,
            value: entry.value,
            decidedBy: entry.decidedBy,
            ...(entry.pinnedBy === undefined ? {} : { pinnedBy: entry.pinnedBy }),
          })),
          warnings: config.warnings,
        },
      },
      renderedText,
    };
  } catch (error) {
    // A config layer from a newer binary is a FINDING in the full report
    // (research R6); `doctor config` alone keeps its exit-65 behavior.
    const fromFuture = error instanceof UnsupportedVersionError;
    return {
      section: {
        id: "config",
        source:
          "five-layer resolution (defaults → org → user → workbench.toml → session), SDD §7.2",
        findings: [
          {
            store: "config",
            dataClass: "durable",
            severity: "unhealthy",
            symptom: error instanceof Error ? error.message : String(error),
            remediation: fromFuture
              ? "upgrade workbench-mcp; a config layer was written by a newer binary and this one fails closed rather than misreading it (SDD §10.2)"
              : "fix the named config layer file; resolution failed before any key was decided",
          },
        ],
        detail: {},
      },
      renderedText: "",
    };
  }
}

async function probePlugins(root: string, stateDir: string): Promise<DoctorSection> {
  const source = ".workbench/plugins.json + .workbench/plugin-health.json";
  const findings: DoctorFinding[] = [];
  let inventory: {
    name: string;
    /** Trust label: the registry tier when present, else the source. */
    trust: string;
    source: string;
    dir?: string;
    manifestPresent?: boolean;
  }[] = [];
  try {
    const records = await loadPluginInventory(root);
    inventory = records.map((record) => ({
      name: record.name,
      trust: record.tier ?? record.source,
      source: record.source,
      ...(record.dir === undefined
        ? {}
        : {
            dir: record.dir,
            manifestPresent: existsSync(join(root, record.dir, "plugin.toml")),
          }),
    }));
    for (const p of inventory) {
      if (p.manifestPresent === false) {
        findings.push({
          store: "plugins.json",
          dataClass: "durable",
          severity: "unhealthy",
          symptom: `plugin ${p.name}: inventory points at ${p.dir ?? "?"} but no plugin.toml is there`,
          remediation: "reinstall the plugin, or remove its entry from .workbench/plugins.json",
        });
      }
    }
  } catch (error) {
    findings.push({
      store: "plugins.json",
      dataClass: "durable",
      severity: "unhealthy",
      symptom: `unreadable inventory: ${error instanceof Error ? error.message : String(error)}`,
      remediation:
        "fix or remove .workbench/plugins.json (durable data: set it aside rather than deleting if unsure)",
    });
  }
  let unhealthy: { plugin: string; componentHash: string; reason: string; at: string }[] = [];
  const healthPath = join(stateDir, "plugin-health.json");
  if (existsSync(healthPath)) {
    try {
      const parsed = JSON.parse(readFileSync(healthPath, "utf8")) as {
        breaches?: { plugin: string; componentHash: string; reason: string; at: string }[];
      };
      unhealthy = parsed.breaches ?? [];
      for (const breach of unhealthy) {
        findings.push({
          store: "plugin-health.json",
          dataClass: "durable",
          severity: "unhealthy",
          symptom: `plugin ${breach.plugin} breakered since ${breach.at}: ${breach.reason}`,
          remediation:
            "update or reinstall the plugin — a changed component hash clears the breaker (SDD §8.1)",
        });
      }
    } catch (error) {
      findings.push({
        store: "plugin-health.json",
        dataClass: "durable",
        severity: "unhealthy",
        symptom: `unreadable: ${error instanceof Error ? error.message : String(error)}`,
        remediation:
          "set .workbench/plugin-health.json aside; the breaker record rebuilds on the next breach",
      });
    }
  }
  if (findings.length === 0) {
    findings.push(
      healthy("plugins", `${inventory.length} installed plugin(s); no breaker records`),
    );
  }
  return { id: "plugins", source, findings, detail: { inventory, unhealthy } };
}

interface QuarantineLogEntry {
  readonly path: string;
  readonly strikes: number;
  readonly quarantined: boolean;
  readonly lastCrash: string;
  readonly at: number;
}

function probeQuarantine(stateDir: string): {
  section: DoctorSection;
  entries: QuarantineLogEntry[];
} {
  const source = ".workbench/quarantine.jsonl (SDD §6.2 durable home)";
  const path = join(stateDir, "quarantine.jsonl");
  if (!existsSync(path)) {
    return {
      section: {
        id: "quarantine",
        source,
        findings: [healthy("quarantine.jsonl", "no quarantine records")],
        detail: { entries: [], malformedLines: 0 },
      },
      entries: [],
    };
  }
  const entries: QuarantineLogEntry[] = [];
  let malformedLines = 0;
  for (const line of readFileSync(path, "utf8").split("\n")) {
    if (line.trim().length === 0) continue;
    try {
      const parsed = JSON.parse(line) as Partial<QuarantineLogEntry>;
      if (typeof parsed.path !== "string") throw new Error("no path");
      entries.push({
        path: parsed.path,
        strikes: typeof parsed.strikes === "number" ? parsed.strikes : 0,
        quarantined: parsed.quarantined === true,
        lastCrash: typeof parsed.lastCrash === "string" ? parsed.lastCrash : "unknown",
        at: typeof parsed.at === "number" ? parsed.at : 0,
      });
    } catch {
      // Append-only logs can tear; a bad line is counted, never fatal.
      malformedLines++;
    }
  }
  const findings: DoctorFinding[] = entries
    .filter((entry) => entry.quarantined)
    .map((entry) => ({
      store: "quarantine.jsonl",
      dataClass: "n/a" as const,
      severity: "unhealthy" as const,
      symptom: `${entry.path} quarantined after ${entry.strikes} strike(s); last crash: ${entry.lastCrash}`,
      remediation:
        "inspect, fix, or remove the file — INPUT_QUARANTINED clears when the file changes (SDD §6.2)",
    }));
  if (findings.length === 0) {
    findings.push(
      healthy("quarantine.jsonl", `${entries.length} record(s), none currently quarantined`),
    );
  }
  return {
    section: { id: "quarantine", source, findings, detail: { entries, malformedLines } },
    entries,
  };
}

function crashesSection(entries: readonly QuarantineLogEntry[]): DoctorSection {
  // Honest sourcing (research R7): quarantine strikes are the only persisted
  // crash evidence today; richer sources are m6-tracing / m6-crash-reports.
  const evidence = entries.map((entry) => ({
    path: entry.path,
    lastCrash: entry.lastCrash,
    at: entry.at,
  }));
  return {
    id: "crashes",
    source:
      "derived from .workbench/quarantine.jsonl strikes (no standalone crash ledger in this build)",
    findings: [
      evidence.length === 0
        ? healthy("crashes", "no engine-crash evidence recorded")
        : {
            store: "crashes",
            dataClass: "n/a",
            severity: "info",
            symptom: `${evidence.length} crash strike record(s); see quarantine section for the affected inputs`,
          },
    ],
    detail: {
      evidence,
      note: "crash evidence is derived from quarantine strikes; a dedicated trace/crash ledger ships with m6-tracing/m6-crash-reports",
    },
  };
}

export interface DoctorRunOptions {
  readonly root: string;
  readonly env: Record<string, string | undefined>;
  readonly sessionFlags: Readonly<Record<string, unknown>>;
  readonly toolVersion: string;
  readonly now: () => Date;
}

export interface DoctorRun {
  readonly report: DoctorReport;
  /** The `renderDoctorConfig` text, embedded verbatim (FR-009). */
  readonly configText: string;
}

/** Compose every probe into the report. Read-only by construction. */
export async function runDoctor(options: DoctorRunOptions): Promise<DoctorRun> {
  const stateDir = join(options.root, ".workbench");
  const config = probeConfig(options.root, options.env, options.sessionFlags);
  const quarantine = probeQuarantine(stateDir);
  const sections: DoctorSection[] = [
    probeEnvironment(options.root, options.env),
    probeIndex(stateDir),
    config.section,
    await probePlugins(options.root, stateDir),
    quarantine.section,
    crashesSection(quarantine.entries),
  ];
  const findingsPresent = sections.some((section) =>
    section.findings.some((finding) => finding.severity === "unhealthy"),
  );
  return {
    report: {
      workspaceRoot: options.root,
      generatedAt: options.now().toISOString(),
      toolVersion: options.toolVersion,
      sections,
      findingsPresent,
    },
    configText: config.renderedText,
  };
}
