/** Current contributor guidance only; dated evidence retains its original references. */
import assert from "node:assert/strict";
import { readFileSync, readdirSync, existsSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const files = [
  "README.md", "CONTRIBUTING.md", "docs/index.md", "docs/getting-started.md",
  "docs/development.md", "docs/release-inventory.md", "docs/releasing-cli.md", "docs/release-security.md",
  "apps/website/README.md",
  ...["docs/reference", "docs/architecture"].flatMap(dir => readdirSync(resolve(repo, dir)).filter(file => file.endsWith(".md") && file !== "desktop-foundation.md").map(file => dir + "/" + file)),
];
function anchors(text) {
  const counts = new Map();
  return [...text.matchAll(/^#{1,6}\s+(.+?)\s*#*$/gm)].map(match => {
    const base = match[1].toLowerCase().replace(/<[^>]*>/g, "").replace(/[^\p{L}\p{N}_\-\s]/gu, "").replace(/\s/g, "-");
    const seen = counts.get(base) ?? 0; counts.set(base, seen + 1);
    return seen ? base + "-" + seen : base;
  });
}
const failures = [];
for (const file of files) {
  const text = readFileSync(resolve(repo, file), "utf8");
  for (const match of text.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
    const target = match[1].split(/\s+"/)[0].replace(/^<|>$/g, "");
    if (/^(https?:|mailto:)/.test(target)) {
      if (target.startsWith("https://magistr.dev/docs")) {
        const url = new URL(target);
        const path = resolve(repo, "apps/website/dist", "." + url.pathname, "index.html");
        if (!existsSync(path)) failures.push(file + ": " + target);
        else if (url.hash && !readFileSync(path, "utf8").includes('id="' + url.hash.slice(1) + '"')) failures.push(file + ": " + target);
      }
      continue;
    }
    const [path, hash] = target.split("#");
    const absolute = path ? resolve(repo, dirname(file), decodeURIComponent(path)) : resolve(repo, file);
    if (!existsSync(absolute)) { failures.push(file + ": " + target); continue; }
    if (hash && absolute.endsWith(".md") && !anchors(readFileSync(absolute, "utf8")).includes(decodeURIComponent(hash))) failures.push(file + ": " + target);
  }
}
assert.deepEqual(failures, [], "Current repository guidance has valid file, manual and fragment links");
console.log("Repository documentation links passed: " + files.length + " current guides; dated evidence excluded.");
