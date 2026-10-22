/**
 * Post-build tests: the built site says what the product does, carries
 * the current app and agent screenshots at their full size, and keeps its metadata.
 * Run with: node tests/build-output.mjs (after `npm run build`)
 */
import { readFileSync, existsSync, readdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const distDir = resolve(__dirname, "../dist");
const srcDir = resolve(__dirname, "../src");

let failures = 0;

function assert(condition, message) {
  if (!condition) {
    console.error(`  FAIL: ${message}`);
    failures++;
  } else {
    console.log(`  PASS: ${message}`);
  }
}

const page = (path) => readFileSync(resolve(distDir, path), "utf-8");
const indexHtml = page("index.html");
const indexText = indexHtml.replace(/<[^>]+>/g, "");
const installHtml = page("docs/installation/index.html");
const conceptsHtml = page("docs/concepts/index.html");
const workflowHtml = page("docs/workflow/index.html");
const notFoundHtml = page("404.html");
const docIds = readdirSync(resolve(srcDir, "content/docs")).filter(file => file.endsWith(".mdx")).map(file => file.slice(0, -4));
const productPages = [["home", indexHtml], ["404", notFoundHtml], ...docIds.map(id => [id === "index" ? "docs" : id, page(id === "index" ? "docs/index.html" : `docs/${id}/index.html`)])];
const mainOf = (html) => html.split("<main")[1]?.split("</main>")[0] ?? html;
const textOf = (html) => mainOf(html).replace(/<code[\s\S]*?<\/code>/g, "").replace(/<pre[\s\S]*?<\/pre>/g, "").replace(/<[^>]+>/g, " ");

console.log("Home:");
assert(indexHtml.includes("Codex") && indexHtml.includes("Claude Code"), "supported agents are named");
assert(indexHtml.includes('id="install-btn"') && indexHtml.includes('href="#install"') && indexHtml.includes('id="install"'), "install call to action, anchor and section");
assert(indexHtml.includes('id="workflow"') && indexHtml.includes('href="/#workflow"'), "roadmap section and its nav link match");
assert(indexText.includes("cargo install --locked --path crates/magistr-cli"), "installation command present");
assert(indexText.includes("git clone https://github.com/AlrikOlson/magistr.git"), "clone command targets the repository");
assert(indexText.includes("magistr --version") && indexText.includes("magistr init") && indexText.includes("magistr next"), "commands after install");
const releaseVersion = JSON.parse(readFileSync(new URL("../../../npm/magistr/package.json", import.meta.url), "utf8")).version;
assert(indexHtml.includes(`v${releaseVersion}`), "the current development version is named");
assert(indexHtml.includes(`"softwareVersion":"${releaseVersion}"`), "structured metadata uses the release version");
assert(!indexHtml.includes("cargo install magistr"), "no registry installation command");
assert(indexHtml.includes("Registry packages are not published yet") && indexHtml.includes("public source repository"), "preview availability is explicit");
assert(!indexHtml.includes("access required") && !indexHtml.includes("Access required") && !indexHtml.includes("repository access"), "the public repository is not described as restricted");
assert(indexHtml.includes("Build bigger projects with your coding agent.") && indexHtml.includes("This is the Codex session that built the example on this page."), "one tagline and an honest recording provenance");
assert(indexHtml.includes('"operatingSystem":"macOS (Apple Silicon)"'), "first release metadata targets Apple Silicon macOS");
assert(indexHtml.includes('"downloadUrl":"https://magistr.dev/docs/installation"'), "download metadata points to current installation instructions");

assert(indexHtml.includes("Bevy 0.20.0") && indexHtml.includes("Recording data"), "one real Bevy build is available");
assert(indexHtml.includes('data-build') && indexHtml.includes('aria-label="Recorded action"'), "recorded actions have an accessible timeline");
assert(indexHtml.includes("Read the full session"), "the published record is available without JavaScript");
assert(!indexHtml.includes("data-project-tour") && !indexHtml.includes("support-inbox"), "the obsolete unrelated feature tour is gone");
assert(indexHtml.includes("Save feedback"), "saved feedback remains discoverable");

console.log("\nNavigation and accessibility:");
assert(indexHtml.includes('aria-label="Primary"'), "primary navigation labelled");
assert(indexHtml.includes('href="/#workflow"') && indexHtml.includes('href="/#install"') && indexHtml.includes('href="/docs/installation"'), "navigation reaches the current sections and guides");
assert(indexHtml.includes('class="skip-link" href="#main-content"') && indexHtml.includes('id="main-content"'), "skip link reaches main");
assert(indexHtml.includes("Repository") && indexHtml.includes('href="https://github.com/AlrikOlson/magistr"'), "the repository is linked");
assert(indexHtml.includes('aria-controls="mobile-menu"') && indexHtml.includes('id="mobile-menu"'), "mobile menu is wired");
assert(installHtml.includes('aria-current="page"'), "the docs link and the sidebar mark the active page");
for (const [name, html] of productPages) {
  const main = mainOf(html);
  assert((main.match(/<h1[\s>]/g) || []).length === 1, `${name}: exactly one h1`);
  assert(!/<h3[\s>]/.test(main) || /<h2[\s>]/.test(main), `${name}: no heading level skipped`);
  assert(html.includes('lang="en"') && html.includes("viewport"), `${name}: lang and viewport`);
}
const externalLinks = [...indexHtml.matchAll(/<a [^>]*target="_blank"[^>]*>/g)].map((m) => m[0]);
assert(externalLinks.length > 0 && externalLinks.every((a) => a.includes('rel="noopener noreferrer"')), "every external link is rel=noopener noreferrer");
assert(mainOf(indexHtml).split('target="_blank"').length - 1 === (mainOf(indexHtml).match(/opens in a new tab/g) || []).length, "every external link in the page body says it opens a new tab");

console.log("\nInternal links:");
const localDocuments = new Map(productPages.map(([name, html]) => [
  name === "home" ? "/" : name === "404" ? "/404.html" : name === "docs" ? "/docs/" : "/docs/" + name + "/", html,
]));
for (const [path, html] of localDocuments) {
  const broken = [];
  for (const [, href] of html.matchAll(/<a\b[^>]*\bhref="([^"]*)"/g)) {
    const target = new URL(href.replaceAll("&amp;", "&"), "https://magistr.dev" + path);
    if (target.origin !== "https://magistr.dev") continue;
    const destination = localDocuments.has(target.pathname) ? target.pathname
      : localDocuments.has(target.pathname + "/") ? target.pathname + "/" : target.pathname;
    if (localDocuments.has(destination)) {
      if (target.hash && ![...localDocuments.get(destination).matchAll(/\bid="([^"]+)"/g)]
        .some(([, id]) => id === decodeURIComponent(target.hash.slice(1)))) broken.push(href);
    } else if (!existsSync(resolve(distDir, "." + target.pathname))) broken.push(href);
  }
  assert(broken.length === 0, path + " has no broken local links or anchors" + (broken.length ? ": " + broken.join(", ") : ""));
}

console.log("\nPublic language:");
const jargon = /TUI2|GUI2|tui2|gui2|think-and-ship|Workbench|think:|ship:|chunk-id|workflow server|native state|admitted|typed command|[Ff]light [Rr]ecorder/;
const commentary = /\bHarbor\b|on purpose|design system|typographic hierarchy/i;
for (const [name, html] of productPages) {
  const text = textOf(html);
  assert(!jargon.test(text), `${name} page carries no internal jargon`);
  assert(!commentary.test(text), `${name} page carries no production commentary`);
}

await import("./docs-content.mjs");

console.log("\nMetadata:");
for (const [name, html] of [["home", indexHtml], ["installation", installHtml], ["concepts", conceptsHtml], ["workflow", workflowHtml]]) {
  assert(html.includes('rel="canonical" href="https://magistr.dev/'), `${name} canonical URL`);
  assert(html.includes('property="og:title"') && html.includes('property="og:image"') && html.includes('property="og:url"'), `${name} Open Graph tags`);
  assert(html.includes('name="twitter:card"') && html.includes('name="twitter:image"'), `${name} Twitter card`);
  assert(html.includes("application/ld+json"), `${name} structured data`);
}
assert(notFoundHtml.includes('name="robots" content="noindex'), "404 page is not indexed");
assert(indexHtml.includes("og-image.png") && existsSync(resolve(distDir, "og-image.png")), "OG image shipped");
assert(indexHtml.includes("site.webmanifest") && indexHtml.includes("apple-touch-icon"), "manifest and touch icon linked");
assert(existsSync(resolve(distDir, "favicon.svg")) && existsSync(resolve(distDir, "favicon-32.png")), "favicons shipped");
// Mutable branding paths must not reuse a previous deployment's CDN cache entry.
const branding = ["favicon.svg", "favicon-16.png", "favicon-32.png", "apple-touch-icon.png", "site.webmanifest", "og-image.png"];
for (const file of branding) {
  const hash = createHash("sha256").update(readFileSync(resolve(distDir, file))).digest("hex").slice(0, 12);
  for (const [name, html] of productPages) {
    assert(html.includes(`/${file}?v=${hash}`), `${name} versions ${file} from its shipped bytes`);
  }
}
const webManifest = JSON.parse(page("site.webmanifest"));
for (const icon of webManifest.icons) {
  const url = new URL(icon.src, "https://magistr.dev");
  const hash = createHash("sha256").update(readFileSync(resolve(distDir, "." + url.pathname))).digest("hex").slice(0, 12);
  assert(url.searchParams.get("v") === hash, `manifest versions ${url.pathname}`);
}
assert(indexHtml.includes("prefers-color-scheme") && indexHtml.includes("theme-toggle"), "theme detection and toggle");
const css = readdirSync(resolve(distDir, "_astro")).filter((f) => f.endsWith(".css")).map((f) => readFileSync(resolve(distDir, "_astro", f), "utf-8")).join("\n");
assert(css.includes("prefers-reduced-motion"), "reduced motion respected in the stylesheet");


console.log("\nSitemap, robots and headers:");
const sitemap = readFileSync(resolve(distDir, "sitemap-0.xml"), "utf-8");
assert(sitemap.includes("https://magistr.dev/docs/concepts/") && !sitemap.includes("/docs/deployment"), "sitemap lists the live pages only");
const robotsTxt = readFileSync(resolve(distDir, "robots.txt"), "utf-8");
assert(robotsTxt.includes("User-agent: *") && robotsTxt.includes("Allow: /") && robotsTxt.includes("sitemap-index.xml"), "robots.txt allows all and names the sitemap");
const headersFile = readFileSync(resolve(distDir, "_headers"), "utf-8");
assert(headersFile.includes("immutable") && headersFile.includes("must-revalidate") && headersFile.includes("/_astro/*"), "caching rules present");
assert(headersFile.includes("X-Content-Type-Options") && headersFile.includes("X-Frame-Options"), "security headers present");
assert(headersFile.includes("/docs/*") && headersFile.split("/docs/*")[1].includes("must-revalidate"), "page routes revalidate at the edge");
const redirects = readFileSync(resolve(distDir, "_redirects"), "utf-8");
for (const gone of ["/docs/architecture", "/docs/development", "/docs/deployment", "/docs/hooks"]) {
  assert(new RegExp(`^${gone}/?\\s+\\S+\\s+301`, "m").test(redirects), `${gone} redirects permanently`);
}

console.log("\n404:");
assert(notFoundHtml.includes("Page not found") && notFoundHtml.includes('href="/docs"') && notFoundHtml.includes('href="/"'), "404 page links home and to the docs");
assert(!notFoundHtml.includes("curl") && !notFoundHtml.includes("rounded-lg"), "404 page uses the site's own style");

console.log("\nSource hygiene:");
const components = readdirSync(resolve(srcDir, "components"));
assert(components.includes("BuildWalkthrough.astro") && !components.includes("ProjectTour.astro"), "one current walkthrough owns the example");
for (const file of readdirSync(resolve(srcDir, "content/docs"))) {
  assert(existsSync(resolve(distDir, "docs", file === "index.mdx" ? "" : file.replace(/\.mdx$/, ""), "index.html")), `guide ${file} is built`);
}
assert(!existsSync(resolve(distDir, "icons")), "no unused icon assets ship");

console.log(`\n${failures === 0 ? "ALL TESTS PASSED" : `${failures} TEST(S) FAILED`}`);
process.exit(failures === 0 ? 0 : 1);