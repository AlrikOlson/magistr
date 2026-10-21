import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
const args = process.argv.slice(2);
for (const [script, folder] of [["browser.mjs", "homepage"], ["docs-browser.mjs", "documentation"]]) {
  const forwarded = [...args];
  const index = forwarded.indexOf("--evidence-dir");
  if (index !== -1) forwarded[index + 1] = resolve(forwarded[index + 1], folder);
  const result = spawnSync(process.execPath, [fileURLToPath(new URL(script, import.meta.url)), ...forwarded], { stdio:"inherit" });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
