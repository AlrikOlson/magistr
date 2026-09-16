/**
 * Post-build tests: the built site says what the product does, carries
 * the terminal screenshots at their full size, and keeps its metadata.
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
const installHtml = page("docs/installation/index.html");
const conceptsHtml = page("docs/concepts/index.html");
const notFoundHtml = page("404.html");
const productPages = [["home", indexHtml], ["installation", installHtml], ["concepts", conceptsHtml], ["404", notFoundHtml]];
const mainOf = (html) => html.split("<main")[1]?.split("</main>")[0] ?? html;
const textOf = (html) => mainOf(html).replace(/<code[\s\S]*?<\/code>/g, "").replace(/<pre[\s\S]*?<\/pre>/g, "").replace(/<[^>]+>/g, " ");

console.log("Home:");
assert(indexHtml.includes("Roadmap and workflow management"), "headline present");
assert(indexHtml.includes("for coding agents"), "headline accent text present");
assert(indexHtml.includes('id="install-btn"') && indexHtml.includes('href="#install"') && indexHtml.includes('id="install"'), "install call to action, anchor and section");
assert(indexHtml.includes('id="roadmap"') && indexHtml.includes('href="/#roadmap"'), "roadmap section and its nav link match");
assert(indexHtml.includes("cargo install --locked --path crates/magistr-cli"), "installation command present");
assert(indexHtml.includes("git clone https://github.com/AlrikOlson/magistr.git"), "clone command targets the repository");
assert(indexHtml.includes("magistr --version") && indexHtml.includes("magistr init") && indexHtml.includes("magistr next-loop"), "commands after install");
assert(indexHtml.includes("0.1.0"), "the current release version is named");
assert(!indexHtml.includes("cargo install magistr"), "no registry installation command");
assert(!indexHtml.includes("feature-card") && !indexHtml.includes("walkthrough-step") && !indexHtml.includes("community-link") && !indexHtml.includes("changelog"), "no marketing card grids or changelog");
const homeImages = [...indexHtml.matchAll(/src="\/screenshots\/([a-z-]+)\.png\?v=[a-f0-9]{12}"/g)].map((m) => m[1]);
assert(new Set(homeImages).size === 8 && homeImages.length === 8, `each screenshot appears once on the home page (${homeImages.join(", ")})`);

console.log("\nNavigation and accessibility:");
assert(indexHtml.includes('aria-label="Primary"'), "primary navigation labelled");
assert(indexHtml.includes('href="/#roadmap"') && indexHtml.includes('href="/#install"') && indexHtml.includes('href="/docs/installation"'), "the three destinations");
assert(indexHtml.includes('class="skip-link" href="#main-content"') && indexHtml.includes('id="main-content"'), "skip link reaches main");
assert(indexHtml.includes('aria-label="Magistr on GitHub (opens in a new tab)"'), "GitHub link has an accessible name");
assert(indexHtml.includes('class="github-mark"') && indexHtml.includes('width="20" height="20"'), "GitHub mark has explicit dimensions");
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

console.log("\nTerminal screenshots:");
const manifest = JSON.parse(readFileSync(resolve(distDir, "screenshots/captures.json"), "utf-8"));
assert(manifest.renderer === "magistr-tui::screenshot::frame" && manifest.surface === "Terminal", "manifest names the terminal renderer");
assert(manifest.state.includes("fixtures/harbor.json"), "manifest names the fixture it was rendered from");
assert(manifest.annotations === false, "screenshots carry no annotations");
assert(manifest.images.length === 8, "overview, roadmap, turn and session screenshots in both themes");
for (const view of ["overview", "roadmap", "turn", "session"]) {
  for (const theme of ["dark", "light"]) {
    assert(manifest.images.some(({ file }) => file === `terminal-${view}-${theme}.png`), `${view} has a ${theme} screenshot`);
  }
}
for (const capture of manifest.images) {
  assert(/^terminal-(overview|roadmap|turn|session)-(dark|light)\.png$/.test(capture.file), `${capture.file} carries a public name`);
  const url = `/screenshots/${capture.file}?v=${capture.sha256.slice(0, 12)}`;
  assert(indexHtml.includes(`src="${url}"`) && indexHtml.includes(`href="${url}"`), capture.file + " image and full-size link use the current content hash");
  const png = readFileSync(resolve(distDir, "screenshots", capture.file));
  assert(png.readUInt32BE(16) === capture.width && png.readUInt32BE(20) === capture.height, capture.file + " keeps its full viewport");
  assert(capture.width === manifest.columns * 10 && capture.height === manifest.rows * 20, capture.file + " is the whole terminal at its cell size");
  assert(createHash("sha256").update(png).digest("hex") === capture.sha256, capture.file + " matches the built bytes");
}
assert(!readdirSync(resolve(distDir, "screenshots")).some((name) => name.startsWith("tui2-")), "no stale screenshot files ship");
assert((indexHtml.match(/width="1320" height="840"/g) || []).length === 8, "every screenshot declares its native dimensions");
assert((indexHtml.match(/<figcaption>/g) || []).length === 4, "four captioned figures");
assert(!indexHtml.includes("/diagrams/") && !indexHtml.includes("/icons/"), "no decorative diagrams or icons");

console.log("\nPublic language:");
const jargon = /TUI2|GUI2|tui2|gui2|think:|ship:|chunk-id|chunk\b|workflow server|native state|admitted|typed command/;
const commentary = /\bsample\b|\bHarbor\b|\bcaptured\b|\bcapture\b|\bfixture\b|on purpose|\bsynthetic\b|\bwe\b|\bour\b|\bgolden|wrangler|Cloudflare|wear/i;
for (const [name, html] of productPages) {
  const text = textOf(html);
  assert(!jargon.test(text), `${name} page carries no internal jargon`);
  assert(!commentary.test(text), `${name} page carries no production commentary`);
}
assert(!indexHtml.includes("illustrative") && !indexHtml.includes("Mermaid"), "no placeholder or diagram-source copy");

console.log("\nDocs:");
assert(installHtml.includes("Permission and privacy boundary"), "installation page states the boundary");
assert(installHtml.includes("project.json") && !installHtml.includes("roadmap_exports"), "installation page covers the project file without policy notes");
assert(installHtml.includes("--agent") && installHtml.includes("codex") && installHtml.includes("copilot") && installHtml.includes("cursor") && installHtml.includes("cline") && installHtml.includes("windsurf") && installHtml.includes("aider"), "installation page names every agent target the CLI accepts");
assert(installHtml.includes("--max-iterations"), "installation page names the loop's bound");
assert(!installHtml.includes("release-inventory") && !installHtml.includes("Release archives"), "installation page carries no release-process links");
assert(conceptsHtml.includes(">Concepts</h1>"), "concepts page has its title");
for (const word of ["Roadmap", "Sessions", "Recorded work", "Verification", "Project state"]) {
  assert(conceptsHtml.includes(word), `concepts page covers ${word}`);
}
assert(conceptsHtml.includes("pinned") && conceptsHtml.includes("checkpoint"), "concepts page separates pin from checkpoint");
for (const slug of ["installation", "concepts"]) {
  assert(installHtml.includes(`href="/docs/${slug}"`), `sidebar links ${slug}`);
}
for (const gone of ["configuration", "hooks", "skills", "deployment", "architecture", "development"]) {
  assert(!existsSync(resolve(distDir, "docs", gone)), `no ${gone} page is built`);
}
assert(installHtml.includes("docs/architecture/overview.md") && installHtml.includes("docs/development.md"), "sidebar points contributors at the repository guides");
assert(installHtml.includes("docs-sidebar") && installHtml.includes("data-pagefind-body"), "docs layout and search body");
assert(existsSync(resolve(distDir, "pagefind/pagefind.js")), "pagefind index built");

console.log("\nMetadata:");
for (const [name, html] of [["home", indexHtml], ["installation", installHtml], ["concepts", conceptsHtml]]) {
  assert(html.includes('rel="canonical" href="https://magistr.dev/'), `${name} canonical URL`);
  assert(html.includes('property="og:title"') && html.includes('property="og:image"') && html.includes('property="og:url"'), `${name} Open Graph tags`);
  assert(html.includes('name="twitter:card"') && html.includes('name="twitter:image"'), `${name} Twitter card`);
  assert(html.includes("application/ld+json"), `${name} structured data`);
}
assert(notFoundHtml.includes('name="robots" content="noindex'), "404 page is not indexed");
assert(indexHtml.includes("og-image.png") && existsSync(resolve(distDir, "og-image.png")), "OG image shipped");
assert(indexHtml.includes("site.webmanifest") && indexHtml.includes("apple-touch-icon"), "manifest and touch icon linked");
assert(existsSync(resolve(distDir, "favicon.svg")) && existsSync(resolve(distDir, "favicon-32.png")), "favicons shipped");
assert(indexHtml.includes("prefers-color-scheme") && indexHtml.includes("theme-toggle"), "theme detection and toggle");
const css = readdirSync(resolve(distDir, "_astro")).filter((f) => f.endsWith(".css")).map((f) => readFileSync(resolve(distDir, "_astro", f), "utf-8")).join("\n");
assert(css.includes("prefers-reduced-motion"), "reduced motion respected in the stylesheet");
assert(css.includes("aspect-ratio:1320/840") || css.includes("aspect-ratio: 1320 / 840"), "screenshots reserve their aspect ratio");

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
for (const gone of ["/docs/architecture", "/docs/development", "/docs/deployment", "/docs/configuration", "/docs/skills", "/docs/hooks", "/docs"]) {
  assert(new RegExp(`^${gone}/?\\s+\\S+\\s+301`, "m").test(redirects), `${gone} redirects permanently`);
}

console.log("\n404:");
assert(notFoundHtml.includes("Page not found") && notFoundHtml.includes('href="/docs/installation"') && notFoundHtml.includes('href="/"'), "404 page links home and to the docs");
assert(!notFoundHtml.includes("curl") && !notFoundHtml.includes("rounded-lg"), "404 page uses the site's own style");

console.log("\nSource hygiene:");
const components = readdirSync(resolve(srcDir, "components"));
assert(components.every((name) => ["Nav.astro", "ThemeToggle.astro"].includes(name)), `only the components in use remain (${components.join(", ")})`);
assert(readdirSync(resolve(srcDir, "content/docs")).sort().join(",") === "concepts.mdx,installation.mdx", "exactly the two docs pages exist");
assert(!existsSync(resolve(distDir, "icons")), "no unused icon assets ship");

console.log(`\n${failures === 0 ? "ALL TESTS PASSED" : `${failures} TEST(S) FAILED`}`);
process.exit(failures === 0 ? 0 : 1);
