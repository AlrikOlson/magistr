import assert from "node:assert/strict";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, sep } from "node:path";
import { checkRead, checkWrite } from "../reference/src/jail/protected";
import fixtures from "./jail.json";

const root = mkdtempSync(join(tmpdir(), "magistr-jail-conformance-"));
const outside = mkdtempSync(join(tmpdir(), "magistr-jail-outside-"));
try {
  for (const fixture of fixtures) {
    const candidate = fixture.candidate.replace("$root", root).replace("$outside", outside);
    const decision = (fixture.write ? checkWrite : checkRead)(root, candidate);
    if (fixture.code) {
      assert.equal(decision.ok, false, fixture.candidate);
      if (!decision.ok) assert.equal(decision.code, fixture.code, fixture.candidate);
    } else {
      assert.equal(decision.ok, true, fixture.candidate);
      if (decision.ok) {
        assert.equal(relative(realpathSync(root), decision.path).split(sep).join("/"), fixture.relative, fixture.candidate);
      }
    }
  }
  console.log(`OK: ${fixtures.length} jail fixtures match pinned Workbench`);
} finally {
  rmSync(root, { recursive: true, force: true });
  rmSync(outside, { recursive: true, force: true });
}
