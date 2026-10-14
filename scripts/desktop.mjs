#!/usr/bin/env node
// Build and open the desktop through its native packaging owner.
import { spawn, spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const desktop = join(root, "crates", "magistr-desktop");
const [action = "open", ...args] = process.argv.slice(2);

function command(program, arguments_, capture = false) {
  const result = spawnSync(program, arguments_, {
    cwd: desktop,
    env: process.env,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
    windowsHide: true,
  });
  if (result.error) throw new Error(`Could not run ${program}: ${result.error.message}`);
  if (result.status !== 0) {
    if (capture && result.stderr) process.stderr.write(result.stderr);
    throw new Error(`${program} failed. Magistr was not opened.`);
  }
  return result.stdout;
}

function ensureTools() {
  const manifest = JSON.parse(readFileSync(join(desktop, "package.json"), "utf8"));
  const installed = join(desktop, "node_modules", "@tauri-apps", "cli", "package.json");
  let version;
  try { version = JSON.parse(readFileSync(installed, "utf8")).version; } catch {}
  if (version !== manifest.devDependencies["@tauri-apps/cli"]) {
    console.log("Installing the desktop build tool…");
    // npm.cmd needs cmd.exe on Windows; this command contains no user arguments.
    const result = spawnSync("npm ci --no-audit --no-fund", {
      cwd: desktop, stdio: "inherit", shell: true, windowsHide: true,
    });
    if (result.error || result.status !== 0) throw new Error("Could not install the desktop build tool. Check that npm is installed and try again.");
  }
  return join(desktop, "node_modules", "@tauri-apps", "cli", "tauri.js");
}

async function main() {
  if (action === "--help" || action === "help") {
    console.log("Usage: node scripts/desktop.mjs open [desktop arguments]\n       node scripts/desktop.mjs package [--debug]\n\nOpen builds the current checkout, then opens Magistr. Package creates a Mac app/disk image or Windows installer.");
    return;
  }
  if (!["open", "package"].includes(action)) throw new Error("Choose open or package. Use --help for instructions.");
  if (!["darwin", "win32"].includes(process.platform)) throw new Error("Use a macOS or Windows computer to build these desktop apps.");
  if (Number(process.versions.node.split(".")[0]) < 22) throw new Error("Install Node.js 22 or later, then try again.");
  if (action === "package" && args.some(arg => arg !== "--debug")) throw new Error("Package accepts only --debug. Desktop arguments belong after open.");

  const cargoBin = join(process.env.CARGO_HOME || join(homedir(), ".cargo"), "bin");
  process.env.PATH = cargoBin + delimiter + (process.env.PATH || "");
  command("cargo", ["--version"], true);
  const cli = ensureTools();
  const debug = action === "open" || args.includes("--debug");
  const buildArgs = [cli, "build", ...(debug ? ["--debug"] : [])];
  if (action === "open") {
    buildArgs.push(...(process.platform === "darwin" ? ["--bundles", "app"] : ["--no-bundle"]));
  }
  // The helper uses the host target explicitly so Cargo configuration cannot
  // silently produce an app for a different computer or an ambiguous output path.
  const host = command("rustc", ["-vV"], true).match(/^host: (.+)$/m)?.[1];
  if (!host) throw new Error("Rust did not report this computer's build target.");
  buildArgs.push("--target", host, "--", "--locked");
  console.log("Building Magistr. The first build can take several minutes…");
  command(process.execPath, buildArgs);
  const metadata = JSON.parse(command("cargo", ["metadata", "--no-deps", "--format-version", "1", "--locked"], true));
  const output = join(metadata.target_directory, host, debug ? "debug" : "release");
  if (action === "package") {
    console.log(`Your app package is in: ${join(output, "bundle")}`);
    return;
  }

  const launchArgs = args[0] === "--" ? args.slice(1) : [...args];
  for (let i = 0; i < launchArgs.length; i++) {
    if (launchArgs[i] === "--dir" && launchArgs[i + 1]) {
      i += 1;
      launchArgs[i] = resolve(launchArgs[i]);
    } else if (launchArgs[i].startsWith("--dir=")) {
      launchArgs[i] = "--dir=" + resolve(launchArgs[i].slice(6));
    }
  }
  console.log("Opening Magistr…");
  if (process.platform === "darwin") {
    command("/usr/bin/open", ["-a", join(output, "bundle", "macos", "Magistr.app"), ...(launchArgs.length ? ["--args", ...launchArgs] : [])]);
  } else {
    const child = spawn(join(output, "magistr-desktop.exe"), launchArgs, {
      cwd: process.cwd(), detached: true, stdio: "ignore", windowsHide: true,
    });
    await new Promise((resolve_, reject) => {
      child.once("error", reject);
      child.once("spawn", resolve_);
    });
    child.unref();
  }
}

main().catch(error => {
  console.error(`\n${error.message}\nSetup help: ${join(root, "docs", "development.md")}`);
  process.exitCode = 1;
});
