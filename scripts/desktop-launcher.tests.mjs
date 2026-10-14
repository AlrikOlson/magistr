import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const launcher = join(root, "scripts", "desktop.mjs");
const native = ["darwin", "win32"].includes(process.platform);

test("help explains launch and packaging without starting a build", () => {
  const result = spawnSync(process.execPath, [launcher, "--help"], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /open \[desktop arguments\]/);
  assert.match(result.stdout, /package \[--debug\]/);
});

test("invalid actions are refused before tool installation or build", () => {
  const result = spawnSync(process.execPath, [launcher, "not-an-action"], { encoding: "utf8" });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Choose open or package/);
  assert.doesNotMatch(result.stdout, /Installing|Building|Opening/);
});

test("missing Rust reports failure instead of opening an old build", { skip: !native }, () => {
  const directory = mkdtempSync(join(tmpdir(), "Magistr missing tools "));
  try {
    const result = spawnSync(process.execPath, [launcher, "open"], {
      encoding: "utf8",
      env: { ...process.env, PATH: directory, CARGO_HOME: directory },
    });
    assert.equal(result.status, 1);
    assert.match(result.stderr, /Could not run cargo/);
    assert.doesNotMatch(result.stdout, /Opening Magistr/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a failed build from a checkout with spaces never launches stale output", { skip: !native }, () => {
  const directory = mkdtempSync(join(tmpdir(), "Magistr checkout with spaces "));
  try {
    const scripts = join(directory, "scripts");
    const desktop = join(directory, "crates", "magistr-desktop");
    const cli = join(desktop, "node_modules", "@tauri-apps", "cli");
    mkdirSync(scripts, { recursive: true });
    mkdirSync(cli, { recursive: true });
    cpSync(launcher, join(scripts, "desktop.mjs"));
    const manifest = JSON.parse(readFileSync(join(root, "crates", "magistr-desktop", "package.json"), "utf8"));
    writeFileSync(join(desktop, "package.json"), JSON.stringify(manifest));
    writeFileSync(join(cli, "package.json"), JSON.stringify({ version: manifest.devDependencies["@tauri-apps/cli"] }));
    writeFileSync(join(cli, "tauri.js"), "process.stderr.write('intentional build failure\\n'); process.exit(23);\n");
    const result = spawnSync(process.execPath, [join(scripts, "desktop.mjs"), "open", "--dir", "a project"], {
      encoding: "utf8", cwd: directory,
    });
    assert.equal(result.status, 1);
    assert.match(result.stderr, /intentional build failure/);
    assert.match(result.stderr, /Magistr was not opened/);
    assert.doesNotMatch(result.stdout, /Opening Magistr/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
