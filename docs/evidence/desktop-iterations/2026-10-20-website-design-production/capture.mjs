import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import { webkit } from "../../../../apps/website/node_modules/playwright/index.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../../../..");
const origin = "https://magistr.dev";
const deployment = process.argv[2];
assert(deployment?.endsWith(".magistr.pages.dev"), "Pass the actual deployment hostname");
const source = execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
const sourceHashes = JSON.parse(await readFile(resolve(directory, "../2026-10-20-website-search-04/browser-checks.json"), "utf8")).sourceHashes;
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
assert(!(await readdir(directory)).some((name) => name.endsWith(".png")), "Never overwrite reviewed captures");
for (const [file, expected] of Object.entries(sourceHashes)) {
  assert.equal(hash(await readFile(resolve(root, file))), expected, "Reviewed source: " + file);
}
const browser = await webkit.launch({ headless: true });
const captures = [];
const assets = new Map();
let status = "running";
let failure;
const save = async () => writeFile(resolve(directory, "deployment-checks.json"), JSON.stringify({
  status, failure, checkedAt: new Date().toISOString(), source, origin,
  deployment, renderer: "Playwright WebKit",
  rendererVersion: browser.version(), platform: process.platform + "-" + process.arch,
  motion: "reduced", scale: 1, sourceHashes, assets: [...assets.values()], captures,
  limitations: ["No native OS-input or assistive-technology testing.", "Visual review recorded separately.", "No registry publication."],
}, null, 2) + "\n");
const verify = async (request, url, localFile) => {
  if (assets.has(url)) return;
  const response = await request.get(url);
  assert.equal(response.status(), 200, url);
  const remote = hash(await response.body());
  const local = hash(await readFile(resolve(root, "apps/website/dist", localFile)));
  assert.equal(remote, local, "Deployed bytes must match reviewed build: " + url);
  assets.set(url, { url, localFile, sha256: remote, status: response.status() });
};
const settle = async (page) => {
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
};
const capture = async (page, name, state) => {
  await settle(page);
  const file = name + ".png";
  await page.screenshot({ path: resolve(directory, file), animations: "disabled" });
  const bytes = await readFile(resolve(directory, file));
  captures.push({ file, state, url: page.url(), capturedAt: new Date().toISOString(),
    viewport: page.viewportSize(), appearance: await page.evaluate(() => document.documentElement.classList.contains("dark") ? "dark" : "light"),
    width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20), sha256: hash(bytes), review: "pending" });
  await save();
};
const scroll = async (page, selector) => page.locator(selector).evaluate((element) =>
  scrollTo({ top: element.getBoundingClientRect().top + scrollY - 90, behavior: "instant" }));
try {
  const request = await browser.newContext();
  for (const [path, local] of [["/", "index.html"], ["/docs/installation/", "docs/installation/index.html"], ["/docs/concepts/", "docs/concepts/index.html"], ["/404.html", "404.html"]]) {
    await verify(request.request, origin + path, local);
  }
  await request.close();
  for (const viewport of [{ width: 1440, height: 1000 }, { width: 390, height: 844 }]) {
    for (const colorScheme of ["light", "dark"]) {
      const compact = viewport.width < 800;
      const suffix = (compact ? "compact" : "wide") + "-" + colorScheme;
      const context = await browser.newContext({ viewport, colorScheme, deviceScaleFactor: 1, reducedMotion: "reduce" });
      await context.route("https://plausible.io/**", (route) => route.fulfill({ status: 204 }));
      const page = await context.newPage();
      await page.goto(origin);
      for (const image of await page.locator("img").all()) {
        if (await image.isVisible()) await image.scrollIntoViewIfNeeded();
        await image.evaluate((element) => element.decode());
      }
      const urls = await page.evaluate(() => [
        ...[...document.querySelectorAll("link[rel=stylesheet], link[rel=icon], link[rel=manifest]")].map((e) => e.href),
        ...[...document.querySelectorAll("script[src], img[src]")].map((e) => e.src),
        document.querySelector("meta[property='og:image']").content,
      ].filter((url) => new URL(url).origin === location.origin));
      for (const url of urls) await verify(context.request, url, new URL(url).pathname.slice(1));
      await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
      await capture(page, "home-intro-" + suffix, "Read the introduction and see the desktop preview");
      await scroll(page, compact ? ".agent-session:last-child" : "#agents .section-heading");
      await capture(page, "home-agents-" + suffix, compact ? "Inspect Claude Code and open its full session" : "Compare the Codex and Claude Code sessions");
      await page.locator("#install-btn").click();
      await scroll(page, "#install .section-heading");
      await capture(page, "home-install-" + suffix, "Find installation and the planning handoff");
      if (compact) {
        await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
        await page.getByRole("button", { name: "Open menu", exact: true }).click();
        await capture(page, "home-menu-" + suffix, "Use the mobile navigation");
        await page.goto(origin + "/docs/installation/");
        await capture(page, "installation-intro-" + suffix, "Read the preview availability and permissions");
      }
      await context.close();
    }
  }
  status = "passed";
} catch (error) {
  status = "failed"; failure = error.message;
  throw error;
} finally {
  await save();
  await browser.close();
}
console.log(JSON.stringify({ passed: status === "passed", source, matchingAssets: assets.size, captures: captures.length }));
