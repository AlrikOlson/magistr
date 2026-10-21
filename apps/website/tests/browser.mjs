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
const scopeArg = process.argv.indexOf("--evidence-scope");
const evidenceScope = scopeArg < 0 ? "all" : process.argv[scopeArg + 1];
assert(["all", "search", "recovery", "refinement", "branding", "copy"].includes(evidenceScope), "Evidence scope must be all, search, recovery, refinement, branding or copy");
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
  "public/og-image.svg", "public/og-image.png", "public/github-social.png", "public/favicon.svg", "public/icon-192.png", "public/site.webmanifest",
  "public/screenshots/captures.json", "scripts/generate-favicons.mjs", "scripts/asset-url.mjs",
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
    motion: "reduced", scale: 1, evidenceScope, checks, sourceHashes, captures,
    limitations: [
      "Headless browser only; no native OS-input or assistive-technology testing.",
      "Screenshots require separate recorded visual review.",
      ...(baseArg < 0 ? ["Local build; no deployed-site or registry publication claim."] : ["Deployed website only; no registry publication claim."]),
    ],
  };
  await writeFile(resolve(evidence, "browser-checks.json"), JSON.stringify(report, null, 2) + "\n");
};
const capture = async (page, name, state, fullPage = false) => {
  if (!evidence || (evidenceScope === "search" && !name.includes("search"))) return;
  if (evidenceScope === "recovery" && !name.includes("search") && !/^not-found-(wide|compact)-(light|dark)$/.test(name)) return;
  if (evidenceScope === "copy") {
    // Full pages cover all four profiles; detail frames use a complementary
    // desktop/light and compact/dark pair to keep the review readable.
    const profile = /-wide-light$|-compact-dark$/.test(name);
    const overview = /^(home|installation|workflow|concepts|not-found)-(wide|compact)-(light|dark)$/.test(name);
    const detail = /^(home-(intro|product|footer|install|workflow-[0-3]|evidence-[0-2]|menu|copy-unavailable)|(?:installation|workflow|concepts)-(?!search)[a-z-]+)-(wide|compact)-(light|dark)$/.test(name);
    if (!overview && !(profile && detail) && !name.startsWith("workflow-320-")) return;
  }
  if (evidenceScope === "branding" && !name.includes("-intro-") && !name.startsWith("home-320-")) return;
  if (evidenceScope === "refinement" && !/^(home-(wide|compact|connection|workflow|capture|transcript)|home-evidence-focus|.*-320)/.test(name)) return;
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
  assert(await page.locator("img").evaluateAll((images) => images.filter((image) => image.getClientRects().length).every((image) => image.complete && image.naturalWidth > 0)), "All images loaded");
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
    return { primary, theme: ratio(theme.color, nav.backgroundColor), code, controls };
  });
  assert(results.controls.every(({ width, height }) => width >= 44 && height >= 44), "Navigation icon targets are at least 44 px");
  assert(results.primary.every((contrast) => contrast >= 4.5), "Primary text contrast: " + JSON.stringify(results));
  assert(results.theme >= 3, "Theme icon contrast: " + JSON.stringify(results));
  assert(results.code.every((overflow) => overflow <= 1), "Code is readable without horizontal clipping");
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
      const searchRequests = [];
      page.on("request", (request) => { if (new URL(request.url()).pathname.startsWith("/pagefind/")) searchRequests.push(request.url()); });
      page.on("pageerror", (error) => errors.push(error.message));
      for (const [name, path] of [["home", "/"], ["installation", "/docs/installation/"], ["concepts", "/docs/concepts/"], ["workflow", "/docs/workflow/"], ["not-found", "/404.html"]]) {
        const response = await page.goto(origin + path);
        assert.equal(response.status(), 200);
        await page.evaluate(() => document.fonts.ready);
        // Scroll lazy images into view before awaiting their actual load.
        for (const image of await page.locator("img").all()) {
          if (!(await image.isVisible())) continue;
          await image.scrollIntoViewIfNeeded();
          await page.waitForFunction((element) => element.complete && element.naturalWidth > 0,
            await image.elementHandle(), { timeout: 10000 });
          await image.evaluate((element) => element.decode());
        }
        await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
        await noOverflow(page);
        await legibility(page);
        await capture(page, name + "-" + suffix, name + " full page", true);
        if (name !== "home") await capture(page, name + "-intro-" + suffix, name + " navigation and opening");
        if (name === "home") {
          for (const link of await page.locator(".image-link").all()) {
            const response = await context.request.get(new URL(await link.getAttribute("href"), origin).href);
            assert.equal(response.status(), 200, "Full session link opens its original image");
            assert.match(response.headers()["content-type"], /image\/png/);
          }
          await capture(page, "home-intro-" + suffix, "Product description, setup action and project guide");
          for (const [area, selector] of [
            ["evidence", "#evidence"], ["workflow", "#workflow"], ["product", "#product"], ["footer", ".site-footer"],
          ]) {
            await scrollHeading(page, selector);
            await capture(page, "home-" + area + "-" + suffix, area + " section");
          }
          for (const [groupName, selector, labels] of [
            ["workflow", ".walkthrough", ["Plan", "Change", "Check", "Continue"]],
            ["evidence", ".product-evidence", ["Codex", "Claude Code", "Magistr desktop"]],
          ]) {
            const group = page.locator(selector);
            const tabs = group.getByRole("tab");
            assert.equal(await tabs.count(), labels.length);
            for (let index = 0; index < labels.length; index++) {
              await tabs.nth(index).click();
              assert.equal(await tabs.nth(index).getAttribute("aria-selected"), "true");
              assert.equal(await group.getByRole("tabpanel").count(), 1, "Only the selected panel is exposed");
              const panel = group.getByRole("tabpanel");
              for (const img of await panel.locator("img").all()) {
                if (!(await img.isVisible())) continue;
                await img.scrollIntoViewIfNeeded();
                await img.evaluate((element) => element.decode());
              }
              await noOverflow(page);
              await scrollHeading(page, selector);
              await capture(page, "home-" + groupName + "-" + index + "-" + suffix, labels[index] + " selected");
              if (groupName === "evidence") {
                if (layout === "compact") {
                  await scrollHeading(page, "#" + await panel.getAttribute("id") + " .capture-excerpt");
                  await capture(page, "home-transcript-" + index + "-" + suffix, labels[index] + " transcript for compact reading");
                }
                await panel.locator(".capture").scrollIntoViewIfNeeded();
                await capture(page, "home-capture-" + index + "-" + suffix, labels[index] + " actual capture");
              }
            }
            await tabs.first().focus();
            await page.keyboard.press("End");
            assert.equal(await tabs.last().getAttribute("aria-selected"), "true");
            assert(await tabs.last().evaluate((element) => element === document.activeElement));
            await page.keyboard.press("ArrowRight");
            assert.equal(await tabs.first().getAttribute("aria-selected"), "true", "Arrow navigation wraps");
            await page.keyboard.press("ArrowLeft");
            assert.equal(await tabs.last().getAttribute("aria-selected"), "true");
            await page.keyboard.press("Home");
            assert.equal(await tabs.first().getAttribute("aria-selected"), "true");
            await capture(page, "home-" + groupName + "-focus-" + suffix, "Keyboard selection and visible tab focus");
          }
          await page.locator("#install-btn").click();
          await scrollHeading(page, "#install .section-heading");
          assert.match(await page.locator("#install").innerText(), /Setup does not start implementation/);
          await capture(page, "home-install-" + suffix, "Preview installation and planning handoff");
          await page.evaluate(() => {
            Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (value) => { window.__copiedCommands = value; } } });
          });
          await page.getByRole("button", { name: "Copy commands" }).click();
          await page.getByRole("status").filter({ hasText: "Commands copied." }).waitFor();
          assert((await page.evaluate(() => window.__copiedCommands)).includes("cargo install --locked --path crates/magistr-cli"));
          await capture(page, "home-copy-success-" + suffix, "Copy confirms the exact source commands");
          await page.evaluate(() => { navigator.clipboard.writeText = async () => { throw new Error("unavailable"); }; });
          await page.getByRole("button", { name: "Copy commands" }).click();
          await page.getByText("Copy is unavailable. Select the commands above to copy them.", { exact: true }).waitFor();
          await capture(page, "home-copy-unavailable-" + suffix, "Denied clipboard offers manual selection");
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
            const firstLink = page.locator("#mobile-menu").getByRole("link", { name: "Example", exact: true });
            await firstLink.focus();
            await page.keyboard.press("Escape");
            assert.equal(await toggle.getAttribute("aria-expanded"), "false");
            assert(await toggle.evaluate((element) => element === document.activeElement));
            await capture(page, "home-menu-dismissed-" + suffix, "Escape closes menu and restores visible focus");
            await toggle.click();
            await firstLink.click();
            assert.equal(await toggle.getAttribute("aria-expanded"), "false");
            assert.equal(new URL(page.url()).hash, "#workflow");
            assert(await page.locator("#workflow").evaluate((element) => element === document.activeElement));
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
        if (["installation", "concepts", "workflow"].includes(name)) {
          for (const heading of await page.locator("article h2").all()) {
            const id = await heading.getAttribute("id");
            await scrollHeading(page, "#" + id);
            await capture(page, name + "-" + id + "-" + suffix, await heading.innerText());
          }
        }
        if (name === "concepts") {
          await capture(page, "concepts-search-idle-" + suffix, "Search is ready without starting its index");
        }
        if (name === "installation") {
          const body = await page.locator("article").innerText();
          assert.match(body, /Setup adds files only for the agents you choose/);
          assert.match(body, /magistr init \. --agent codex,claude --dry-run/);
          assert.match(body, /magistr init \. --agent portable --yes/);
          assert.match(body, /magistr remove \. --dry-run/);
          await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
          const search = page.getByRole("searchbox", { name: "Search documentation" });
          assert.deepEqual(searchRequests, [], "Reading docs does not start search downloads");
          await search.fill("repair");
          await page.locator(".docs-search-results a").first().waitFor();
          assert.match(await page.locator(".docs-search-results").innerText(), /Installation/);
          await capture(page, "installation-search-" + suffix, "Documentation search for repair");
          await search.fill("qzxvbnabcdef");
          await page.getByText("No results. Try a different word.", { exact: true }).waitFor();
          await capture(page, "installation-search-empty-" + suffix, "No results preserve a clear way to search again");
          await search.fill("repair");
          await search.fill("");
          assert.equal(await page.locator(".docs-search-results").innerText(), "", "Clearing cancels pending results");
          await search.fill("repair");
          await page.locator(".docs-search-results a").first().waitFor();
          await page.locator(".docs-search-results a").first().click();
          assert.equal(new URL(page.url()).pathname.replace(/\/$/, ""), "/docs/installation");
          // A failed module download must leave readable recovery and usable guide links.
          await context.route("**/pagefind/pagefind.js", (route) => route.fulfill({ status: 503, contentType: "text/javascript", body: "" }));
          await page.reload();
          await page.getByRole("searchbox", { name: "Search documentation" }).fill("repair");
          await page.getByText("Search couldn’t load. Try reloading, or use the guide links below.", { exact: true }).waitFor();
          await capture(page, "installation-search-unavailable-" + suffix, "Search failure retains navigation and explains recovery");
          await context.unroute("**/pagefind/pagefind.js");
          await page.reload();
          await page.getByRole("searchbox", { name: "Search documentation" }).fill("repair");
          await page.locator(".docs-search-results a").first().waitFor();
          assert.match(await page.locator(".docs-search-results").innerText(), /Installation/);
        }
      }
      // The 320 px reflow check catches overflow hidden by the normal mobile viewport.
      await page.setViewportSize({ width: 320, height: 844 });
      await page.goto(origin + "/docs/installation/");
      await noOverflow(page);
      if (layout === "compact") {
        await capture(page, "installation-320-" + colorScheme, "Installation at 320 CSS pixels", true);
        for (const heading of ["install-the-preview", "connect-another-agent", "start-work-when-you-are-ready"]) {
          await scrollHeading(page, "#" + heading);
          await capture(page, "installation-320-" + heading + "-" + colorScheme, "320 px " + heading);
        }
      }
      await page.goto(origin + "/docs/workflow/");
      await noOverflow(page);
      await legibility(page);
      if (layout === "compact") {
        await capture(page, "workflow-320-" + colorScheme, "Everyday-use guide at 320 CSS pixels", true);
        await scrollHeading(page, "#choose-the-right-starting-point");
        await capture(page, "workflow-320-shortcuts-" + colorScheme, "Skill table at 320 CSS pixels");
      }
      await page.goto(origin + "/");
      await noOverflow(page);
      if (layout === "compact") {
        await capture(page, "home-320-" + colorScheme, "Home at 320 CSS pixels", true);
        for (const [index, tab] of (await page.locator(".walkthrough").getByRole("tab").all()).entries()) {
          await tab.click();
          await noOverflow(page);
          await scrollHeading(page, ".workflow-panel:not([hidden])");
          await capture(page, "home-workflow-320-" + index + "-" + colorScheme, "320 px walkthrough " + index);
        }
        for (const [index, tab] of (await page.locator(".product-evidence").getByRole("tab").all()).entries()) {
          await tab.click();
          const panel = page.locator(".evidence-panel:not([hidden])");
          await scrollHeading(page, "#" + await panel.getAttribute("id") + " .capture-excerpt");
          await capture(page, "home-transcript-320-" + index + "-" + colorScheme, "320 px transcript " + index);
        }
        await scrollHeading(page, ".walkthrough");
        await capture(page, "home-workflow-320-" + colorScheme, "Selected walkthrough at 320 CSS pixels");
      }
      // A keyboard user can reach content without traversing the navigation.
      const skip = page.getByRole("link", { name: "Skip to content" });
      await skip.focus();
      await page.waitForFunction(() => document.querySelector(".skip-link").getBoundingClientRect().y >= 0, null, { timeout: 2000 });
      const skipState = await page.evaluate(() => ({ active: document.activeElement?.outerHTML, y: document.querySelector(".skip-link").getBoundingClientRect().y, scrollY }));
      assert(skipState.y >= 0, "Focused skip link: " + JSON.stringify(skipState));
      await page.keyboard.press("Enter");
      assert.equal(new URL(page.url()).hash, "#main-content");
      assert.deepEqual(errors, [], "No uncaught page errors");
      checks.push({ layout, appearance: colorScheme, passed: true, scenarios: ["five static routes", "primary and theme-control contrast", "wrapped code and aligned documentation arrows", "walkthrough and evidence tabs: pointer, arrows, Home, End, focus", "clipboard success and denial (browser stub)", "reflow at 320 px", "images", "install anchor", "theme persistence", "selected agent instructions", "lazy search, results, empty and failure recovery, cleared query", "keyboard skip link", ...(layout === "compact" ? ["mobile menu navigation, Escape, anchor focus and resize recovery"] : [])] });
      await context.close();
    }
  }
  const staticContext = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 390, height: 844 } });
  const staticPage = await staticContext.newPage();
  await staticPage.goto(origin + "/");
  assert.equal(await staticPage.locator("[data-panel]:visible").count(), 7, "Without JavaScript every explanation and actual capture remains in reading order");
  assert.equal(await staticPage.locator("[data-tablist]:visible").count(), 0, "No inert tab controls without JavaScript");
  await staticContext.close();
  await writeReport("passed");
  console.log(JSON.stringify({ passed: true, profiles: checks.length, screenshots: captures.length, evidence }, null, 2));
} catch (error) {
  await writeReport("failed", error.message);
  throw error;
} finally {
  await browser.close();
  await new Promise((done) => server.close(done));
}