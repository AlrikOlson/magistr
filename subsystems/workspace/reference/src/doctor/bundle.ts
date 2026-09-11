/**
 * Doctor bundle serialization (feature 004-doctor-bundles, research R3/R4):
 * the report's sections as machine-readable members plus a manifest, every
 * member passing the single `redact()` choke point, packed as tar.gz by the
 * in-tree ustar writer. Structure and symptoms, never repo file contents —
 * the probes never read repo files, and redaction is defense in depth on
 * top of that structural guarantee.
 */

import type { DoctorRun } from "./probes";
import { renderDoctorReport } from "./report";
import { type TarMember, tarGz } from "./tar";

export const BUNDLE_FORMAT_VERSION = 1;
export const BUNDLE_PREFIX = "workbench-doctor";

export interface BundleManifest {
  readonly bundleFormatVersion: typeof BUNDLE_FORMAT_VERSION;
  readonly toolVersion: string;
  readonly createdAt: string;
  /** Placeholder identity, never the absolute workspace path. */
  readonly workspaceLabel: string;
  readonly members: readonly string[];
}

/**
 * The redaction choke point (research R4). Applied to every member's text:
 * the workspace root (in native, forward-slash, and JSON-escaped spellings)
 * becomes `<workspace>`, and any remaining absolute path is reduced to an
 * `<outside-workspace>/…basename` placeholder. Environment variable values
 * never enter members because no probe reads them.
 */
export function redact(text: string, workspaceRoot: string): string {
  let out = text;
  const spellings = new Set<string>([
    workspaceRoot,
    workspaceRoot.replaceAll("\\", "/"),
    workspaceRoot.replaceAll("/", "\\"),
    // Inside JSON members backslashes are escaped.
    workspaceRoot.replaceAll("\\", "\\\\"),
    workspaceRoot.replaceAll("/", "\\\\"),
  ]);
  for (const spelling of spellings) {
    if (spelling.length > 0) out = out.replaceAll(spelling, "<workspace>");
  }
  // Remaining absolute paths (POSIX or drive-letter, raw or JSON-escaped):
  // keep only the basename behind a placeholder. Anchored to a boundary so a
  // mid-word slash (".workbench/index.db") is never mistaken for a path root.
  const absolute =
    /(^|[\s"'`(=[])((?:[A-Za-z]:(?:\\\\|\\|\/)|\/)(?:[^\s"'`|<>]*(?:\\\\|\\|\/))*[^\s"'`|<>\\/]+)/gm;
  out = out.replaceAll(absolute, (match, prefix: string, full: string) => {
    if (full.includes("<workspace>")) return match;
    const base = full.split(/[\\/]+/).at(-1) ?? "";
    return `${prefix}<outside-workspace>/${base}`;
  });
  return out;
}

export interface BuiltBundle {
  readonly bytes: Uint8Array<ArrayBuffer>;
  readonly members: readonly string[];
}

export function buildBundle(run: DoctorRun): BuiltBundle {
  const { report, configText } = run;
  const memberBodies = new Map<string, string>();
  memberBodies.set("report.txt", renderDoctorReport(run));
  memberBodies.set("config.txt", configText);
  for (const section of report.sections) {
    if (section.id === "config") continue; // config.txt carries the provenance text
    memberBodies.set(
      `${section.id}.json`,
      JSON.stringify(
        { source: section.source, findings: section.findings, detail: section.detail },
        null,
        2,
      ),
    );
  }
  const memberNames = [...memberBodies.keys()].map((name) => `${BUNDLE_PREFIX}/${name}`);
  const manifest: BundleManifest = {
    bundleFormatVersion: BUNDLE_FORMAT_VERSION,
    toolVersion: report.toolVersion,
    createdAt: report.generatedAt,
    workspaceLabel: "<workspace>",
    members: [`${BUNDLE_PREFIX}/manifest.json`, ...memberNames],
  };
  const encoder = new TextEncoder();
  const members: TarMember[] = [
    {
      name: `${BUNDLE_PREFIX}/manifest.json`,
      data: encoder.encode(JSON.stringify(manifest, null, 2)),
    },
    ...[...memberBodies.entries()].map(([name, body]) => ({
      name: `${BUNDLE_PREFIX}/${name}`,
      data: encoder.encode(redact(body, report.workspaceRoot)),
    })),
  ];
  const mtimeSeconds = Math.floor(Date.parse(report.generatedAt) / 1000);
  return { bytes: tarGz(members, mtimeSeconds), members: manifest.members };
}
