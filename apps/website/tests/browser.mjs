import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { readFile, readdir, mkdir, writeFile, stat } from "node:fs/promises";
import { dirname, extname, resolve, sep, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { webkit } from "playwright";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = resolve(root, "dist");
const repo = resolve(root, "../..");
const evidenceArg = process.argv.indexOf("--evidence-dir");
const evidence = evidenceArg < 0 ? undefined : resolve(repo, process.argv[evidenceArg + 1]);
if (evidence && !evidence.startsWith(resolve(repo, "docs/evidence/desktop-iterations") + sep)) {
  throw new Error("Save captures under docs/evidence/desktop-iterations/<dated-iteration>.");
}
if (evidence) {
  await mkdir(evidence, { recursive: true });
  assert(!(await readdir(evidence)).some((name) => name.endsWith(".png")), "Use a new evidence directory; never overwrite captures.");
}
const mime = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".png": "image/png", ".svg": "image/svg+xml", ".json": "application/json", ".wasm": "application/wasm" };
const server = createServer(async (request, response) => {
  try {
    const path = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
    let file = resolve(dist, "." + path);
    assert(file.startsWith(dist + sep) || file === dist);
    if ((await stat(file)).isDirectory()) file = resolve(file, "index.html");
    response.writeHead(200, { "Content-Type": mime[extname(file)] ?? "application/octet-stream" });
    response.end(await readFile(file));
  } catch {
    response.writeHead(404);
    response.end("Not found");
  }
});
await new Promise((done) => server.listen(0, "127.0.0.1", done));
const baseArg = process.argv.indexOf("--base-url");
const origin = baseArg < 0 ? "http://127.0.0.1:" + server.address().port : new URL(process.argv[baseArg + 1]).origin;
const browser = await webkit.launch({ headless: true });
const checks = [];
const captures = [];
const paths = [];
const collect = async (directory) => {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const file = resolve(directory, entry.name);
    if (entry.isDirectory()) await collect(file);
    else paths.push(file);
  }
};
await collect(resolve(root, "src"));
for (const name of [
  "astro.config.mjs", "package.json", "package-lock.json", "tests/browser.mjs",
  "public/og-image.svg", "public/og-image.png", "public/site.webmanifest",
  "public/screenshots/captures.json", "scripts/generate-favicons.mjs",
]) paths.push(resolve(root, name));
const sourceHashes = Object.fromEntries(await Promise.all(paths.sort().map(async (file) => [
  relative(repo, file), createHash("sha256").update(await readFile(file)).digest("hex"),
])));
const writeReport = async (status, failure) => {
  if (!evidence) return;
  const report = {
    completedAt: new Date().toISOString(), status, failure,
    renderer: "Playwright WebKit", rendererVersion: browser.version(),
    platform: process.platform + "-" + process.arch, node: process.version, origin,
    motion: "reduced", scale: 1, checks, sourceHashes, captures,
    limitations: [
      "Headless browser only; no native OS-input or assistive-technology testing.",
      "Screenshots require separate recorded visual review.",
      ...(baseArg < 0 ? ["Local build; no deployed-site or registry publication claim."] : ["Deployed website only; no registry publication claim."]),
    ],
  };
  await writeFile(resolve(evidence, "browser-checks.json"), JSON.stringify(report, null, 2) + "\n");
};
const capture = async (page, name, state, fullPage = false) => {
  if (!evidence) return;
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
  const file = name + ".png";
  await page.screenshot({ path: resolve(evidence, file), fullPage, animations: "disabled" });
  const bytes = await readFile(resolve(evidence, file));
  captures.push({
    file, state, url: new URL(page.url()).pathname, capturedAt: new Date().toISOString(),
    viewport: page.viewportSize(), appearance: await page.evaluate(() => document.documentElement.classList.contains("dark") ? "dark" : "light"),
    fullPage, renderer: "Playwright WebKit", rendererVersion: browser.version(),
    width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20),
    sha256: createHash("sha256").update(bytes).digest("hex"), review: "pending",
  });
  await writeReport("running");
};
const noOverflow = async (page) => {
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), "No horizontal page overflow");
  assert(await page.locator("main h1").count() === 1, "One primary heading");
  assert(await page.locator("img").evaluateAll((images) => images.every((image) => image.complete && image.naturalWidth > 0)), "All images loaded");
};
const legibility = async (page) => {
  const results = await page.evaluate(() => {
    const luminance = (color) => color.match(/[\d.]+/g).slice(0, 3)
      .map(Number).map((value) => value / 255)
      .map((value) => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4)
      .reduce((sum, value, index) => sum + value * [0.2126, 0.7152, 0.0722][index], 0);
    const ratio = (a, b) => (Math.max(luminance(a), luminance(b)) + 0.05) / (Math.min(luminance(a), luminance(b)) + 0.05);
    const primary = [...document.querySelectorAll(".button-primary")].map((element) => {
      const style = getComputedStyle(element);
      return ratio(style.color, style.backgroundColor);
    });
    const theme = getComputedStyle(document.querySelector("#theme-toggle"));
    const nav = getComputedStyle(document.querySelector(".site-nav"));
    const controls = [...document.querySelectorAll(".theme-toggle, .menu-toggle, .github-link")].filter((element) => element.getClientRects().length).map((element) => {
      const rect = element.getBoundingClientRect();
      return { width: rect.width, height: rect.height };
    });
    const code = [...document.querySelectorAll("pre")].map((element) => element.scrollWidth - element.clientWidth);
    const arrows = [...document.querySelectorAll(".doc-links a")].map((element) => {
      const title = element.querySelector("strong").getBoundingClientRect();
      const arrow = element.querySelector("[aria-hidden]").getBoundingClientRect();
      return Math.abs(title.y + title.height / 2 - arrow.y - arrow.height / 2);
    });
    return { primary, theme: ratio(theme.color, nav.backgroundColor), code, arrows, controls };
  });
  assert(results.controls.every(({ width, height }) => width >= 44 && height >= 44), "Navigation icon targets are at least 44 px");
  assert(results.primary.every((contrast) => contrast >= 4.5), "Primary text contrast: " + JSON.stringify(results));
  assert(results.theme >= 3, "Theme icon contrast: " + JSON.stringify(results));
  assert(results.code.every((overflow) => overflow <= 1), "Code is readable without horizontal clipping");
  assert(results.arrows.every((offset) => offset < 3), "Documentation arrows align with their titles");
};
const scrollHeading = async (page, selector) => {
  await page.locator(selector).evaluate((element) => scrollTo({ top: element.getBoundingClientRect().top + scrollY - 90, behavior: "instant" }));
};
try {
  for (const viewport of [{ width: 1440, height: 1000 }, { width: 390, height: 844 }]) {
    for (const colorScheme of ["light", "dark"]) {
      const layout = viewport.width > 800 ? "wide" : "compact";
      const suffix = layout + "-" + colorScheme;
      const context = await browser.newContext({ viewport, deviceScaleFactor: 1, colorScheme, reducedMotion: "reduce" });
      await context.route("https://plausible.io/**", (route) => route.fulfill({ status: 204 }));
      const page = await context.newPage();
      const errors = [];
      page.on("pageerror", (error) => errors.push(error.message));
      for (const [name, path] of [["home", "/"], ["installation", "/docs/installation/"], ["concepts", "/docs/concepts/"], ["not-found", "/404.html"]]) {
        const response = await page.goto(origin + path);
        assert.equal(response.status(), 200);
        await page.evaluate(() => document.fonts.ready);
        // Scroll lazy images into view before awaiting their actual load.
        for (const image of await page.locator("img").all()) {
          if (await image.isVisible()) await image.scrollIntoViewIfNeeded();
          await page.waitForFunction((element) => element.complete && element.naturalWidth > 0,
            await image.elementHandle(), { timeout: 10000 });
          await image.evaluate((element) => element.decode());
        }
        await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
        await noOverflow(page);
        await legibility(page);
        await capture(page, name + "-" + suffix, name + " full page", true);
        if (name === "home") {
          for (const link of await page.locator(".image-link").all()) {
            const response = await context.request.get(new URL(await link.getAttribute("href"), origin).href);
            assert.equal(response.status(), 200, "Full session link opens its original image");
            assert.match(response.headers()["content-type"], /image\/png/);
          }
          await capture(page, "home-intro-" + suffix, "Introduction and desktop preview");
          for (const [area, selector] of [
            ["roadmap", "#roadmap"], ["agents", "#agents .section-heading"], ["footer", ".docs-index"],
          ]) {
            await scrollHeading(page, selector);
            await capture(page, "home-" + area + "-" + suffix, area + " section");
          }
          await page.locator("#install-btn").click();
          await scrollHeading(page, "#install .section-heading");
          assert.match(await page.locator("#install").innerText(), /Setup does not start implementation/);
          await capture(page, "home-install-" + suffix, "Preview installation and planning handoff");
          // Theme switches persist across a new document.
          await page.getByRole("button", { name: "Toggle dark mode" }).click();
          assert.equal(await page.evaluate(() => document.documentElement.classList.contains("dark")), colorScheme !== "dark");
          await page.reload();
          assert.equal(await page.evaluate(() => document.documentElement.classList.contains("dark")), colorScheme !== "dark");
          await page.getByRole("button", { name: "Toggle dark mode" }).click();
          await legibility(page);
          if (layout === "compact") {
            const toggle = page.locator("[data-menu-toggle]");
            await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
            await toggle.click();
            assert.equal(await toggle.getAttribute("aria-expanded"), "true");
            await capture(page, "home-menu-" + suffix, "Expanded compact navigation");
            const firstLink = page.locator("#mobile-menu").getByRole("link", { name: "How it works", exact: true });
            await firstLink.focus();
            await page.keyboard.press("Escape");
            assert.equal(await toggle.getAttribute("aria-expanded"), "false");
            assert(await toggle.evaluate((element) => element === document.activeElement));
            await capture(page, "home-menu-dismissed-" + suffix, "Escape closes menu and restores visible focus");
            await toggle.click();
            await firstLink.click();
            assert.equal(await toggle.getAttribute("aria-expanded"), "false");
            assert.equal(new URL(page.url()).hash, "#roadmap");
            assert(await page.locator("#roadmap").evaluate((element) => element === document.activeElement));
            await toggle.click();
            await firstLink.focus();
            await page.setViewportSize({ width: 1024, height: 1000 });
            await page.waitForFunction(() => document.querySelector("[data-menu-toggle]").getAttribute("aria-expanded") === "false");
            assert.equal(await toggle.getAttribute("aria-expanded"), "false");
            assert(await page.locator("[data-nav-links] a").first().evaluate((element) => element === document.activeElement));
            await page.setViewportSize(viewport);
            assert.equal(await toggle.getAttribute("aria-expanded"), "false");
            await toggle.click();
            await page.locator("#mobile-menu").getByRole("link", { name: "Docs", exact: true }).click();
            assert.equal(new URL(page.url()).pathname.replace(/\/$/, ""), "/docs/installation");
          }
        }
        if (name === "concepts") {
          for (const heading of ["your-plan", "your-work-history", "checking-your-work", "agent-sessions", "the-desktop-preview", "where-your-work-is-saved"]) {
            await scrollHeading(page, "#" + heading);
            await capture(page, "concepts-" + heading + "-" + suffix, heading.replaceAll("-", " "));
          }
        }
        if (name === "installation") {
          const body = await page.locator("article").innerText();
          assert.match(body, /Setup adds files only for the agents you choose/);
          assert.match(body, /magistr init \. --agent codex,claude --dry-run/);
          assert.match(body, /magistr init \. --agent portable --yes/);
          assert.match(body, /magistr remove \. --dry-run/);
          for (const heading of [
            "install-the-preview", "set-up-your-project", "connect-codex", "connect-claude-code",
            "connect-another-agent", "check-or-repair-setup", "upgrade-the-preview",
            "remove-setup-or-uninstall", "start-work-when-you-are-ready",
          ]) {
            await scrollHeading(page, "#" + heading);
            await capture(page, "installation-" + heading + "-" + suffix, heading.replaceAll("-", " "));
          }
          await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
          const search = page.getByRole("searchbox", { name: "Search documentation" });
          await search.fill("repair");
          await page.locator(".docs-search-results a").first().waitFor();
          assert.match(await page.locator(".docs-search-results").innerText(), /Installation/);
          await capture(page, "installation-search-" + suffix, "Documentation search for repair");
          await page.locator(".docs-search-results a").first().click();
          assert.equal(new URL(page.url()).pathname.replace(/\/$/, ""), "/docs/installation");
        }
      }
      // The 320 px reflow check catches overflow hidden by the normal mobile viewport.
      await page.setViewportSize({ width: 320, height: 844 });
      await page.goto(origin + "/docs/installation/");
      await noOverflow(page);
      await page.goto(origin + "/");
      await noOverflow(page);
      // A keyboard user can reach content without traversing the navigation.
      const skip = page.getByRole("link", { name: "Skip to content" });
      await skip.focus();
      await page.waitForFunction(() => document.querySelector(".skip-link").getBoundingClientRect().y >= 0, null, { timeout: 2000 });
      const skipState = await page.evaluate(() => ({ active: document.activeElement?.outerHTML, y: document.querySelector(".skip-link").getBoundingClientRect().y, scrollY }));
      assert(skipState.y >= 0, "Focused skip link: " + JSON.stringify(skipState));
      await page.keyboard.press("Enter");
      assert.equal(new URL(page.url()).hash, "#main-content");
      assert.deepEqual(errors, [], "No uncaught page errors");
      checks.push({ layout, appearance: colorScheme, passed: true, scenarios: ["four static routes", "primary and theme-control contrast", "wrapped code and aligned documentation arrows", "reflow at 320 px", "images", "install anchor", "theme persistence", "selected agent instructions", "search and result navigation", "keyboard skip link", ...(layout === "compact" ? ["mobile menu navigation, Escape, anchor focus and resize recovery"] : [])] });
      await context.close();
    }
  }
  await writeReport("passed");
  console.log(JSON.stringify({ passed: true, profiles: checks.length, screenshots: captures.length, evidence }, null, 2));
} catch (error) {
  await writeReport("failed", error.message);
  throw error;
} finally {
  await browser.close();
  await new Promise((done) => server.close(done));
}