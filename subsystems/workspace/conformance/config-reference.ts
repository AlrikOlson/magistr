/** Shared config/admission corpus against the pinned reference, no server. */
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { resolveConfig, renderDoctorConfig } from "../reference/src/config";
import { detectCommands } from "../reference/src/config/detect";
import { guardCommandLine } from "../reference/src/tools/run";
import corpus from "./config.json";

for (const fixture of corpus.cases) {
  const root = mkdtempSync(join(tmpdir(), "magistr-config-parity-"));
  try {
    const env = { WORKBENCH_ORG_POLICY: join(root, "org.toml"), WORKBENCH_USER_CONFIG: join(root, "user.toml") };
    for (const [key, file] of [["org", "org.toml"], ["user", "user.toml"], ["workspace", "workbench.toml"]] as const) {
      if (fixture[key] !== undefined) writeFileSync(join(root, file), fixture[key]);
    }
    if (fixture.unsupported) {
      assert.throws(() => resolveConfig({ root, env }), /supports 1/);
      continue;
    }
    const config = resolveConfig({ root, env, sessionFlags: fixture.session });
    for (const [key, expected] of Object.entries(fixture.effective)) assert.deepEqual(config.effective[key], expected, `${fixture.name}: ${key}`);
    assert.deepEqual(config.commands, fixture.commands, fixture.name);
    assert.deepEqual(config.violations.map(v => [v.key, v.attemptedBy, v.pinnedBy]), fixture.violations, fixture.name);
    assert.equal(config.warnings.length, fixture.warnings.length, fixture.name);
    fixture.warnings.forEach((fragment, index) => assert.ok(config.warnings[index].includes(fragment), `${fixture.name}: ${fragment}`));
    for (const [key, expected] of Object.entries(fixture.trace)) {
      const entry = config.trace.find(entry => entry.key === key);
      assert.deepEqual([entry?.decidedBy, entry?.pinnedBy ?? null], expected, fixture.name);
    }
    assert.ok(renderDoctorConfig(config, root).includes("security.redaction ="));
  } finally { rmSync(root, { recursive: true, force: true }); }
}
for (const fixture of corpus.guards) {
  // Reviewed native safety repairs retain the historical result explicitly.
  assert.equal(guardCommandLine(fixture.line, fixture.args) === undefined, fixture.reference_allowed ?? fixture.allowed, fixture.line);
  if (fixture.argv) assert.deepEqual([...fixture.line.split(/\s+/).filter(Boolean), ...fixture.args], fixture.argv);
}
for (const fixture of corpus.detection) {
  const root = mkdtempSync(join(tmpdir(), "magistr-detection-parity-"));
  try {
    for (const [file, body] of Object.entries(fixture.files)) {
      mkdirSync(dirname(join(root, file)), { recursive: true });
      writeFileSync(join(root, file), body);
    }
    assert.deepEqual(detectCommands(root), fixture.commands, fixture.name);
  } finally { rmSync(root, { recursive: true, force: true }); }
}
console.log(`config parity: ${corpus.cases.length} resolution, ${corpus.guards.length} admission, ${corpus.detection.length} detection fixtures passed`);
