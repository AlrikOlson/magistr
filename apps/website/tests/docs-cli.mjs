/** Opt-in checks against an installed CLI; all writes use disposable project/data folders. */
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, writeFileSync, existsSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sandbox = mkdtempSync(resolve(tmpdir(), "magistr-manual-"));
const project = resolve(sandbox, "project");
mkdirSync(project);
const env = {
  ...process.env,
  XDG_CONFIG_HOME:resolve(sandbox, "config"), XDG_DATA_HOME:resolve(sandbox, "data"),
  XDG_CACHE_HOME:resolve(sandbox, "cache"), XDG_RUNTIME_DIR:resolve(sandbox, "runtime"),
  MAGISTR_HOME:resolve(sandbox, "state"), MAGISTR_WORKFLOW_DATA_DIR:resolve(sandbox, "workflow"),
};
delete env.MAGISTR_PROJECT_ID;
delete env.MAGISTR_EXECUTION_ID;
const binary = process.env.MAGISTR_DOCS_BINARY || "magistr";
function run(args, cwd = project) {
  const result = spawnSync(binary, args, { cwd, env, encoding:"utf8", timeout:60000 });
  assert.equal(result.status, 0, args.join(" ") + "\n" + result.stdout + result.stderr);
  return result.stdout;
}
const split = line => (line.match(/"[^"]*"|'[^']*'|[^\s]+/g) ?? []).map(part => part.replace(/^["']|["']$/g, ""));
try {
  console.log(run(["--version"]).trim());
  const sources = readdirSync(resolve(root, "src/content/docs")).filter(file => file.endsWith(".mdx")).map(file => readFileSync(resolve(root, "src/content/docs", file), "utf8"));
  const commands = new Set(sources.flatMap(source => [...source.matchAll(/```sh\n([\s\S]*?)```/g)].flatMap(match => match[1].split("\n").filter(line => line.startsWith("magistr ")))));
  for (const command of commands) run([...split(command).slice(1), "--help"]);
  console.log("Validated " + commands.size + " documented CLI command forms without starting an agent.");
  const preview = run(["init", ".", "--agent", "codex,claude", "--dry-run", "--json"]);
  assert(preview.includes("codex") && preview.includes("claude"));
  assert(!existsSync(resolve(project, ".magistr/project.json")), "Preview does not apply setup");
  run(["init", ".", "--agent", "portable", "--yes"]);
  assert(existsSync(resolve(project, ".mcp.json")));
  assert(existsSync(resolve(project, ".magistr/instructions.md")));
  const configPath = resolve(project, ".magistr/project.json");
  const config = JSON.parse(readFileSync(configPath, "utf8"));
  const id = config.project.id;
  const configuration = readFileSync(resolve(root, "src/content/docs/configuration.mdx"), "utf8");
  const fragment = JSON.parse(configuration.match(/```json\n([\s\S]*?)```/)[1]);
  writeFileSync(configPath, JSON.stringify({ ...config, ...fragment }, null, 2));
  run(["config", "check", "."]);
  run(["config", "set", "agent", "codex", "--project"]);
  run(["config", "set", "model", "default", "--project"]);
  const selection = run(["next", "--show-config"]);
  assert(selection.includes("codex"));
  // Exercise a recognized older setting and preservation of its original.
  const older = JSON.parse(readFileSync(configPath, "utf8"));
  older.roadmap_backend = "think-and-ship";
  writeFileSync(configPath, JSON.stringify(older, null, 2));
  const before = readFileSync(configPath, "utf8");
  run(["repair", ".", "--dry-run", "--json"]);
  assert.equal(readFileSync(configPath, "utf8"), before, "Repair preview preserves configuration");
  run(["repair", ".", "--yes"]);
  assert.equal(JSON.parse(readFileSync(configPath, "utf8")).project.id, id, "Repair retains project identity");
  assert.equal(JSON.parse(readFileSync(configPath, "utf8")).roadmap_backend, "native", "Known older configuration converts to current form");
  assert(readdirSync(resolve(project, ".magistr/upgrades/configuration")).length > 0, "Upgrade retains recovery copies");
  run(["remove", ".", "--dry-run", "--json"]);
  assert(existsSync(resolve(project, ".mcp.json")), "Removal preview preserves the connection");
  run(["remove", ".", "--yes"]);
  assert(existsSync(configPath), "Removal retains project settings");
  assert.equal(JSON.parse(readFileSync(configPath, "utf8")).project.id, id);
  console.log("Disposable setup, check configuration, launch selection, repair and removal passed. No model turn, project reset or real user record was used.");
} finally { rmSync(sandbox, { recursive:true, force:true }); }