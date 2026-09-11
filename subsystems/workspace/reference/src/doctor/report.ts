/**
 * Doctor report rendering (feature 004-doctor-bundles, contract
 * cli-doctor.md): sections in contract order, a `source:` line on every
 * section (FR-002), a `remediation:` line on every unhealthy finding
 * (FR-007). The config section embeds the `renderDoctorConfig` text
 * verbatim rather than forking it (FR-009).
 */

import type { DoctorRun } from "./probes";

const SEVERITY_MARK = { healthy: "ok", info: "--", unhealthy: "!!" } as const;

export function renderDoctorReport(run: DoctorRun): string {
  const { report, configText } = run;
  const lines: string[] = [];
  lines.push(`workbench doctor — workspace ${report.workspaceRoot}`);
  lines.push(`generated ${report.generatedAt} by workbench-mcp ${report.toolVersion}`);
  for (const section of report.sections) {
    lines.push("");
    lines.push(`[${section.id}]`);
    lines.push(`  source: ${section.source}`);
    for (const finding of section.findings) {
      lines.push(`  ${SEVERITY_MARK[finding.severity]} ${finding.store}: ${finding.symptom}`);
      if (finding.remediation !== undefined) {
        lines.push(`     remediation: ${finding.remediation}`);
      }
    }
    if (section.id === "config" && configText.length > 0) {
      for (const configLine of configText.split("\n")) {
        lines.push(`  ${configLine}`);
      }
    }
  }
  lines.push("");
  lines.push(
    report.findingsPresent
      ? "result: findings present (exit 1)"
      : "result: all probes healthy (exit 0)",
  );
  return lines.join("\n");
}
