import assert from "node:assert/strict";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { resolve, sep } from "node:path";
import { webkit } from "playwright";
import { root, repo, arg, startServer, configureNetwork, stable, noOverflow, tree, hashSources } from "./browser-support.mjs";

// macOS WebKit uses Option-Tab for links unless full keyboard access is enabled.
// https://support.apple.com/guide/safari/cpsh003/mac
const linkTab = process.platform === "darwin" ? "Alt+Tab" : "Tab";
const ids = (await readdir(resolve(root, "src/content/docs"))).filter(file => file.endsWith(".mdx")).map(file => file.slice(0, -4)).sort();
const docUrl = id => id === "index" ? "/docs/" : `/docs/${id}/`;
const evidence = arg("--evidence-dir") ? resolve(repo, arg("--evidence-dir")) : null;
if (evidence) {
  assert(evidence.startsWith(resolve(repo, "docs/evidence/desktop-iterations") + sep));
  await mkdir(evidence, { recursive:true });
  assert(!(await readdir(evidence)).some(file => file.endsWith(".png")), "Use a fresh iteration folder");
}
const { server, origin } = await startServer();
const browser = await webkit.launch();
const captures = [], checks = [], errors = [];
const sourceHashes = evidence ? await hashSources([
  ...await tree("apps/website/src"), ...await tree("apps/website/scripts"), ...await tree("apps/website/tests"),
  "apps/website/astro.config.mjs", "apps/website/package.json", "apps/website/package-lock.json",
  "apps/website/public/_redirects",
]) : {};
async function capture(page, name, task, full = false) {
  if (!evidence) return;
  await stable(page);
  const file = name + ".png";
  await page.screenshot({ path:resolve(evidence, file), fullPage:full });
  captures.push({
    file, task, viewport:page.viewportSize(),
    appearance:await page.evaluate(() => document.documentElement.classList.contains("dark") ? "dark" : "light"),
    capturedAt:new Date().toISOString(), fullPage:full,
    sha256:createHash("sha256").update(await readFile(resolve(evidence, file))).digest("hex"),
  });
  await saveReport();
}
async function ready(page, url) {
  const response = await page.goto(origin + url);
  assert.equal(response.status(), 200, url);
  await stable(page);
}
async function contextFor(width, height, appearance, extra = {}) {
  const context = await browser.newContext({ viewport:{ width, height }, colorScheme:appearance, reducedMotion:"reduce", deviceScaleFactor:1, ...extra });
  context.setDefaultTimeout(15_000);
  await configureNetwork(context);
  await context.addInitScript(theme => localStorage.setItem("theme", theme), appearance);
  return context;
}
let status = "failed", failure = "";
async function saveReport() {
  if (evidence) await writeFile(resolve(evidence, "docs-browser-checks.json"), JSON.stringify({
    status, failure, capturedAt:new Date().toISOString(), renderer:"Playwright WebKit", rendererVersion:browser.version(),
    origin, sourceHashes, checks, captures,
    limitations:["Headless browser layout and DOM input only; native OS input and assistive technology were not tested.", "Screenshots require separate inspection and review verdicts."],
  }, null, 2) + "\n");
}
try {
  for (const [profile, width, height] of [["desktop", 1280, 1000], ["compact", 390, 844]]) {
    for (const appearance of ["light", "dark"]) {
      const context = await contextFor(width, height, appearance);
      const page = await context.newPage();
      page.on("pageerror", error => errors.push(String(error)));
      const suffix = profile + "-" + appearance;
      for (const id of ids) {
        await ready(page, docUrl(id));
        assert.equal(await page.locator("main h1").count(), 1);
        assert.equal(await page.locator("main").getAttribute("data-doc-id"), id);
        await noOverflow(page, id + " " + suffix);
        const nav = page.getByRole("navigation", { name:"Documentation pages", exact:true, includeHidden:true });
        assert.equal(await nav.locator("a").count(), ids.length);
        assert.equal(await nav.locator('[aria-current="page"]').count(), 1);
        assert.equal(await nav.locator('[aria-current="page"]').getAttribute("href"), docUrl(id).replace(/\/$/, ""));
        assert(await page.getByRole("searchbox", { name:"Search documentation" }).isVisible());
        assert.equal(await page.locator(".docs-guide-list").evaluate(node => node.open), width > 800);
        if (width > 800) {
          assert(await page.locator(".docs-guide-list").evaluate(node => {
            const bounds = node.getBoundingClientRect();
            const active = node.querySelector('[aria-current="page"]').getBoundingClientRect();
            return active.top >= bounds.top && active.bottom <= bounds.bottom;
          }), id + " active navigation stays in view");
        }
        const related = page.getByRole("navigation", { name:"Related pages", exact:true });
        assert(await related.locator("a").count() > 0);
        // Verify real related navigation and browser back from direct entry.
        await related.locator("a").first().click();
        assert.notEqual(new URL(page.url()).pathname.replace(/\/$/, ""), docUrl(id).replace(/\/$/, ""));
        await page.goBack(); await stable(page);
        await page.evaluate(() => scrollTo(0, 0));
        await capture(page, "page-" + id + "-" + suffix, "Read " + (await page.locator("main h1").textContent()) + " from direct entry", true);
        // Exercise every on-page target without relying on a specific heading list.
        await page.locator(".docs-contents summary").click();
        const anchors = await page.getByRole("navigation", { name:"On this page", exact:true }).locator("a").evaluateAll(nodes => nodes.map(node => node.getAttribute("href")));
        for (const href of anchors) {
          await page.getByRole("navigation", { name:"On this page", exact:true }).locator(`a[href="${href}"]`).click();
          assert.equal(decodeURIComponent(new URL(page.url()).hash), href);
          const heading = page.locator(href);
          assert(await heading.count());
          const y = await heading.evaluate(node => node.getBoundingClientRect().top);
          assert(y >= 0 && y < height, id + " anchor is in view");
        }
      }
      await ready(page, "/docs/configuration/");
      const summary = page.locator(".docs-guide-list>summary");
      if (width < 801) {
        await summary.focus(); await page.keyboard.press("Enter");
        assert(await page.locator(".docs-guide-list").evaluate(node => node.open));
        await page.keyboard.press(linkTab);
        assert(await page.getByRole("navigation", { name:"Documentation pages", exact:true }).evaluate(node => node.contains(document.activeElement)));
        await capture(page, "navigation-open-" + suffix, "Browse grouped documentation with a keyboard", true);
        await page.keyboard.press("Escape");
        assert(await summary.evaluate(node => node === document.activeElement));
        assert(!(await page.locator(".docs-guide-list").evaluate(node => node.open)));
      }
      const input = page.getByRole("searchbox", { name:"Search documentation" });
      for (const [query, expected] of [["configure checks", "configuration"], ["resume work", "sessions"], ["upgrade", "upgrades"]]) {
        await input.fill(query);
        await page.waitForFunction(() => document.querySelector(".docs-search-results")?.getAttribute("aria-busy") === "false" && document.querySelectorAll(".docs-search-results a").length > 0);
        const links = await page.locator(".docs-search-results a").evaluateAll(nodes => nodes.map(node => node.getAttribute("href")));
        assert(links.every(link => link.startsWith("/docs/") || link === "/docs"), "Search stays in documentation");
        assert(links.some(link => link.includes("/docs/" + expected)), query + " finds " + expected);
      }
      await capture(page, "search-results-" + suffix, "Find upgrade instructions from documentation search");
      await input.fill("zzzz-no-such-document");
      await page.getByText("No results. Try a different word.", { exact:true }).waitFor();
      await capture(page, "search-empty-" + suffix, "Recover from a documentation query with no results");
      await input.fill("resume work");
      await page.waitForFunction(() => document.querySelectorAll(".docs-search-results a").length > 0);
      await input.focus(); await page.keyboard.press(linkTab);
      assert(await page.locator(".docs-search-results").evaluate(node => node.contains(document.activeElement)), "Search keyboard focus: " + await page.evaluate(() => document.activeElement?.outerHTML));
      await page.keyboard.press("Enter"); await page.waitForLoadState();
      assert(new URL(page.url()).pathname.startsWith("/docs/"));
      await ready(page, "/docs/configuration/");
      await page.locator(".docs-contents summary").click();
      await capture(page, "contents-open-" + suffix, "Choose a section within configuration");
      await page.getByRole("navigation", { name:"On this page", exact:true }).getByRole("link", { name:"Configure checks", exact:true }).click();
      await capture(page, "configuration-detail-" + suffix, "Read and adapt the labeled JSON configuration fragment");
      await ready(page, "/docs/commands/");
      await page.locator("#work-and-verification").scrollIntoViewIfNeeded();
      await capture(page, "commands-detail-" + suffix, "Compare command options and effects in a reference table");
      checks.push("Direct entry to " + ids.length + " pages, active navigation, all anchors, related links/back, real search queries and keyboard use: " + suffix);
      await context.close();
      console.log("Documentation " + suffix + " passed");
    }
  }
  // Resizing preserves a useful focus destination in both directions.
  const resizeContext = await contextFor(1280, 1000, "light");
  const resize = await resizeContext.newPage();
  await ready(resize, "/docs/sessions/");
  await resize.getByRole("navigation", { name:"Documentation pages", exact:true }).getByRole("link", { name:"Sessions and resuming work", exact:true }).focus();
  await resize.setViewportSize({ width:390, height:844 });
  await resize.waitForFunction(() => document.activeElement === document.querySelector(".docs-guide-list>summary"));
  assert(!(await resize.locator(".docs-guide-list").evaluate(node => node.open)));
  await capture(resize, "resize-compact-focus", "Keep navigation focus when the sidebar becomes a disclosure");
  await resize.setViewportSize({ width:1280, height:1000 });
  await resize.waitForFunction(() => document.activeElement?.getAttribute("aria-current") === "page");
  await capture(resize, "resize-desktop-focus", "Return focus to the current page when the sidebar expands");
  await resizeContext.close();

  for (const appearance of ["light", "dark"]) {
    const context = await contextFor(320, 900, appearance);
    const page = await context.newPage();
    for (const id of ["index", "commands", "configuration", "storage"]) {
      await ready(page, docUrl(id)); await noOverflow(page, id + " at 320");
    }
    await capture(page, "reflow-320-" + appearance, "Read storage at 320 CSS pixels", true);
    await page.setViewportSize({ width:800, height:1000 });
    await ready(page, "/docs/configuration/");
    const textSelectors = [".docs-content p", ".docs-content p a", ".docs-content strong", ".docs-content table", ".docs-content td", ".docs-content code", ".docs-content pre", ".docs-breadcrumbs"];
    const fontSizes = () => page.evaluate(selectors => selectors.map(selector => {
      const element = document.querySelector(selector);
      return parseFloat(getComputedStyle(element).fontSize);
    }), textSelectors);
    const originalSizes = await fontSizes();
    await page.addStyleTag({ content:"html { font-size: 200% !important; }" });
    // WebKit can settle inherited sizes after the root size and two frames.
    // Wait for actual text sizes, so evidence cannot capture a half-updated frame.
    await page.waitForFunction(({ selectors, original }) => selectors.every((selector, i) =>
      Math.abs(parseFloat(getComputedStyle(document.querySelector(selector)).fontSize) / original[i] - 2) < 0.01
    ), { selectors:textSelectors, original:originalSizes });
    await stable(page);
    const enlargedSizes = await fontSizes();
    for (let i = 0; i < textSelectors.length; i++) {
      assert(Math.abs(enlargedSizes[i] / originalSizes[i] - 2) < 0.01, textSelectors[i] + " enlarges to 200%");
    }
    await noOverflow(page, "200% text");
    assert(await page.getByRole("searchbox").isVisible());
    await capture(page, "text-200-top-" + appearance, "Find navigation and the title with text enlarged to 200%");
    await page.locator(".docs-content p").first().scrollIntoViewIfNeeded();
    await capture(page, "text-200-detail-" + appearance, "Read paragraph links and table headings with text enlarged to 200%");
    await page.evaluate(() => scrollTo(0, 0));
    await capture(page, "text-200-" + appearance, "Read configuration with text enlarged to 200%", true);
    await context.close();

    const offlineContext = await contextFor(390, 844, appearance);
    const offline = await offlineContext.newPage();
    await offline.route("**/pagefind/**", route => route.abort());
    await ready(offline, "/docs/");
    await offline.getByRole("searchbox").fill("upgrade");
    await offline.getByText("Search couldn’t load. Try reloading, or browse documentation below.", { exact:true }).waitFor();
    await offline.locator(".docs-guide-list>summary").click();
    await offline.getByRole("navigation", { name:"Documentation pages", exact:true }).getByRole("link", { name:"Upgrades", exact:true }).click();
    assert(new URL(offline.url()).pathname.includes("/docs/upgrades"));
    await offline.getByRole("searchbox").fill("upgrade");
    await offline.getByText("Search couldn’t load. Try reloading, or browse documentation below.", { exact:true }).waitFor();
    await capture(offline, "search-unavailable-" + appearance, "Keep browsing when the search index cannot load");
    await offlineContext.close();
  }
  const nojsContext = await contextFor(390, 844, "light", { javaScriptEnabled:false });
  const nojs = await nojsContext.newPage();
  // Animation-frame callbacks do not run when script execution is disabled.
  const nojsResponse = await nojs.goto(origin + "/docs/");
  assert.equal(nojsResponse.status(), 200);
  await nojs.getByRole("navigation", { name:"Documentation pages", exact:true }).getByRole("link", { name:"Installation", exact:true }).click();
  // Hosts may canonicalize directory routes with a trailing slash.
  await nojs.waitForURL(url => url.pathname.replace(/\/$/, "") === "/docs/installation");
  await nojs.waitForLoadState("load");
  await nojs.locator("main h1").waitFor({ state:"visible" });
  assert.equal(await nojs.locator("main h1").textContent(), "Installation");
  await nojsContext.unrouteAll({ behavior:"wait" });
  await nojsContext.close();
  checks.push("Resize focus both directions; 320 CSS pixel reflow; 200% text; search-load failure with working fallback; navigation without JavaScript");
  assert.deepEqual(errors, []);
  status = "passed";
  console.log("Documentation browser checks passed: " + ids.length + " pages in four profiles.");
} catch (error) { failure = String(error); throw error; }
finally {
  await saveReport();
  await browser.close(); server.close();
}
