/**
 * `workbench doctor config` (SDD §7.2, §11.2): render the merged effective
 * configuration with per-key layer provenance — each key's value and which
 * layer decided it, plus pins, warnings, and policy denials. Pure string
 * rendering over a resolution, so the report is testable without a process
 * and identical on every transport.
 */

import type { ResolvedConfig } from "./resolve";

function renderValue(value: unknown): string {
  return Array.isArray(value) ? JSON.stringify(value) : String(value);
}

/** The full `doctor config` report, one string, trailing-newline-free. */
export function renderDoctorConfig(config: ResolvedConfig, root: string): string {
  const lines: string[] = [];
  lines.push(`workbench doctor config — workspace ${root}`);
  lines.push(`config_version supported: ${config.configVersion}`);
  lines.push("");
  lines.push("layer files (lowest precedence first):");
  for (const layer of config.layers) {
    const where = layer.source === undefined ? "" : `  ${layer.source}`;
    const status = layer.error !== undefined ? "unreadable" : layer.present ? "read" : "absent";
    lines.push(`  ${layer.layer.padEnd(10)} ${status}${where}`);
  }
  lines.push("");
  lines.push("effective config (value ← deciding layer):");
  for (const entry of config.trace) {
    const pin = entry.pinnedBy === undefined ? "" : `, pinned by ${entry.pinnedBy}`;
    lines.push(`  ${entry.key} = ${renderValue(entry.value)}  (${entry.decidedBy}${pin})`);
  }
  if (config.warnings.length > 0) {
    lines.push("");
    lines.push("warnings:");
    for (const warning of config.warnings) lines.push(`  ${warning}`);
  }
  if (config.violations.length > 0) {
    lines.push("");
    lines.push("policy denials (CONFIG_POLICY_DENIED):");
    for (const violation of config.violations) lines.push(`  ${violation.message}`);
  }
  return lines.join("\n");
}
