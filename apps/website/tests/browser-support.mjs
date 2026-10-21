import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFile, readdir, stat } from "node:fs/promises";
import { createHash } from "node:crypto";
import { resolve, dirname, extname, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const repo = resolve(root, "../..");
export const dist = resolve(root, "dist");
export const arg = name => { const i = process.argv.indexOf(name); return i < 0 ? null : process.argv[i + 1]; };
const mime = { ".html":"text/html", ".css":"text/css", ".js":"text/javascript", ".json":"application/json", ".png":"image/png", ".svg":"image/svg+xml", ".wasm":"application/wasm" };
export async function startServer() {
  const server = createServer(async (req, res) => {
    try {
      let path = resolve(dist, "." + decodeURIComponent(new URL(req.url, "http://localhost").pathname));
      assert(path.startsWith(dist + sep) || path === dist);
      if ((await stat(path)).isDirectory()) path = resolve(path, "index.html");
      res.setHeader("Content-Type", mime[extname(path)] ?? "application/octet-stream");
      res.end(await readFile(path));
    } catch { res.writeHead(404); res.end("Not found"); }
  });
  const external = arg("--base-url");
  if (!external) await new Promise(done => server.listen(0, "127.0.0.1", done));
  return { server, origin: external ?? "http://127.0.0.1:" + server.address().port };
}
const assets = new Map();
export async function configureNetwork(context) {
  await context.route("https://plausible.io/**", route => route.fulfill({ status:204, body:"" }));
  await context.route(/^https:\/\/fonts\.(?:googleapis|gstatic)\.com\//, async route => {
    const url = route.request().url();
    if (!assets.has(url)) assets.set(url, (async () => {
      const response = await route.fetch({ timeout:15000 });
      assert(response.ok(), "Font request failed: " + url);
      return { status:response.status(), headers:{ "content-type":response.headers()["content-type"], "access-control-allow-origin":"*" }, body:await response.body() };
    })());
    await route.fulfill(await assets.get(url));
  });
}
export const stable = async page => {
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(() => new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done))));
};
export async function noOverflow(page, label) {
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), label + " has no horizontal overflow");
}
export async function tree(path) {
  const paths = [];
  for (const entry of await readdir(resolve(repo, path), { withFileTypes:true })) {
    const name = path + "/" + entry.name;
    if (entry.isDirectory()) paths.push(...await tree(name));
    else if (/\.(astro|mjs|ts|css|mdx|json)$/.test(entry.name)) paths.push(name);
  }
  return paths;
}
export async function hashSources(paths) {
  return Object.fromEntries(await Promise.all(paths.map(async path => [path, createHash("sha256").update(await readFile(resolve(repo, path))).digest("hex")])));
}
