import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const folder = resolve(repo, process.argv[2] ?? "");
assert(folder.startsWith(resolve(repo, "docs/evidence/desktop-iterations") + "/"));
const report = JSON.parse(readFileSync(resolve(folder, "docs-browser-checks.json"), "utf8"));
const review = JSON.parse(readFileSync(resolve(folder, "review.json"), "utf8"));
// Browser assertions are recorded by the separate required WebKit check.
// A later assertion failure does not invalidate an already captured, reviewed frame.
assert.equal(report.renderer, "Playwright WebKit");
assert.equal(review.verdict, "pass");
const hash = path => createHash("sha256").update(readFileSync(path)).digest("hex");
// Preserve capture-harness hashes as provenance. Freshness depends on the sources
// that render the interface, not on subsequent changes to test assertions.
const renderSources = Object.entries(report.sourceHashes).filter(([path]) => !path.startsWith("apps/website/tests/"));
assert(renderSources.some(([path]) => path === "apps/website/src/layouts/Docs.astro"));
for (const [path, expected] of renderSources) assert.equal(hash(resolve(repo, path)), expected, "Rendered source changed: " + path);
const entries = new Map(review.images.map(image => [image.file, image]));
assert.equal(entries.size, report.captures.length, "Every capture has one inspection record");
for (const capture of report.captures) {
  assert.equal(hash(resolve(folder, capture.file)), capture.sha256, "Capture bytes: " + capture.file);
  const inspected = entries.get(capture.file);
  assert(inspected, "Missing inspection: " + capture.file);
  assert.equal(inspected.verdict, "pass", "Unresolved visual defect: " + capture.file);
  assert(inspected.observations?.length > 40 && inspected.task, "Concrete observation and task: " + capture.file);
}
const ids = readdirSync(resolve(repo, "apps/website/src/content/docs")).filter(file => file.endsWith(".mdx")).map(file => file.slice(0, -4));
for (const id of ids) for (const size of ["desktop", "compact"]) for (const appearance of ["light", "dark"]) {
  assert(entries.has(`page-${id}-${size}-${appearance}.png`), "Missing page appearance/size");
}
console.log("Documentation evidence is current: " + report.captures.length + " captured images have recorded passing reviews. See REVIEW.md for visual judgments and limits.");
