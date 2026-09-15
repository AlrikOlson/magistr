/**
 * Post-build tests: the built site says what the product does, carries
 * the real terminal captures at their full size, and keeps its metadata.
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
const architectureHtml = page("docs/architecture/index.html");
const developmentHtml = page("docs/development/index.html");
const deploymentHtml = page("docs/deployment/index.html");
const notFoundHtml = page("404.html");
const publicPages = [indexHtml, installHtml, conceptsHtml, architectureHtml, developmentHtml, deploymentHtml, notFoundHtml];

console.log("Home:");
assert(indexHtml.includes("Roadmap and workflow management"), "headline present");
assert(indexHtml.includes("for coding agents"), "headline accent text present");
assert(indexHtml.includes("Install from source"), "install call to action present");
assert(indexHtml.includes('href="#install"') && indexHtml.includes('id="install"'), "install anchor and section");
assert(indexHtml.includes('id="terminal"'), "terminal story section present");
assert(indexHtml.includes("cargo install --locked --path crates/magistr-cli"), "source installation command present");
assert(indexHtml.includes("git clone https://github.com/AlrikOlson/magistr.git"), "clone command targets the repository");
assert(indexHtml.includes("magistr --version"), "installation verification command present");
assert(indexHtml.includes("magistr init"), "initialization command present");
assert(indexHtml.includes("magistr next-loop"), "loop command present");
assert(indexHtml.includes("0.1.0"), "the current release version is named");
assert(!indexHtml.includes("cargo install magistr"), "no registry installation command");
assert(!indexHtml.includes("feature-card") && !indexHtml.includes("walkthrough-step") && !indexHtml.includes("community-link"), "no marketing card grids");
assert(!indexHtml.includes("changelog"), "no changelog placeholder");

console.log("\nNavigation and accessibility:");
assert(indexHtml.includes('aria-label="Primary"'), "primary navigation labelled");
assert(indexHtml.includes('href="/#terminal"') && indexHtml.includes('href="/#install"') && indexHtml.includes('href="/docs/installation"'), "the three destinations");
assert(indexHtml.includes('class="skip-link" href="#main-content"') && indexHtml.includes('id="main-content"'), "skip link reaches main");
assert(indexHtml.includes('aria-label="Magistr on GitHub (opens in a new tab)"'), "GitHub link has an accessible name");
assert(indexHtml.includes('class="github-mark"') && indexHtml.includes('width="20" height="20"'), "GitHub mark has explicit dimensions");
assert(indexHtml.includes('aria-controls="mobile-menu"') && indexHtml.includes('id="mobile-menu"'), "mobile menu is wired");
assert(installHtml.includes('aria-current="page"'), "the docs link marks the active page");
const h1s = (indexHtml.match(/<h1[\s>]/g) || []).length;
assert(h1s === 1, `exactly one h1 on the home page (${h1s})`);
for (const html of publicPages) {
  assert(!/<h3[\s>]/.test(html.split("<main")[1]?.split("</main>")[0] ?? "") || /<h2[\s>]/.test(html), "no heading level is skipped");
}
assert(indexHtml.includes('lang="en"'), "html lang attribute");
assert(indexHtml.includes("viewport"), "viewport meta tag");

console.log("\nTerminal captures:");
const manifest = JSON.parse(readFileSync(resolve(distDir, "screenshots/captures.json"), "utf-8"));
assert(manifest.renderer === "magistr-tui::screenshot::frame" && manifest.surface === "Terminal", "capture provenance names the terminal renderer");
assert(manifest.state.includes("fixtures/harbor.json"), "the manifest names the fixture it was rendered from");
assert(manifest.annotations === false, "captures carry no annotations");
assert(manifest.images.length === 4, "roadmap and trace captures in both themes");
for (const capture of manifest.images) {
  assert(/^terminal-(roadmap|trace)-(dark|light)\.png$/.test(capture.file), `${capture.file} carries a public name`);
  assert(indexHtml.includes("/screenshots/" + capture.file), capture.file + " referenced on the home page");
  const png = readFileSync(resolve(distDir, "screenshots", capture.file));
  assert(png.readUInt32BE(16) === capture.width && png.readUInt32BE(20) === capture.height, capture.file + " keeps its full viewport");
  assert(capture.width === manifest.columns * 10 && capture.height === manifest.rows * 20, capture.file + " is the whole terminal at its cell size");
  assert(createHash("sha256").update(png).digest("hex") === capture.sha256, capture.file + " matches the captured bytes");
}
assert(!readdirSync(resolve(distDir, "screenshots")).some((name) => name.startsWith("tui2-")), "no stale capture files ship");
assert((indexHtml.match(/width="1320" height="840"/g) || []).length >= 6, "every capture declares its native dimensions");
assert(indexHtml.includes("<figure") && indexHtml.includes("<figcaption"), "captures are captioned figures");
assert(!indexHtml.includes("/diagrams/") && !indexHtml.includes("/icons/"), "no decorative diagrams or icons on the home page");

console.log("\nPublic language:");
const jargon = /TUI2|GUI2|tui2|gui2|think:|ship:|chunk-id|workflow server/;
const meta = /\bsample\b|\bHarbor\b|\bcaptured\b|\bcapture\b|\bfixture\b|on purpose|\bsynthetic\b|\bwe\b|\bour\b/i;
for (const [name, html] of [["home", indexHtml], ["installation", installHtml], ["concepts", conceptsHtml], ["architecture", architectureHtml], ["development", developmentHtml], ["deployment", deploymentHtml]]) {
  const body = html.split("<main")[1]?.split("</main>")[0] ?? html;
  assert(!jargon.test(body), `${name} page carries no internal jargon`);
  const prose = body.replace(/<code[\s\S]*?<\/code>/g, "").replace(/<pre[\s\S]*?<\/pre>/g, "");
  if (name !== "development" && name !== "deployment") {
    assert(!meta.test(prose.replace(/<[^>]+>/g, " ")), `${name} page carries no production commentary`);
  }
}
assert(!indexHtml.includes("illustrative") && !indexHtml.includes("Mermaid"), "no placeholder or diagram-source copy");

console.log("\nDocs:");
assert(installHtml.includes("Installation") && installHtml.includes("Permission and privacy boundary"), "installation page states the boundary");
assert(installHtml.includes("project.json") && installHtml.includes("roadmap_exports"), "installation page covers the project file");
assert(installHtml.includes("--agent codex") && installHtml.includes("/next"), "installation page connects an agent");
assert(installHtml.includes("release-inventory.md"), "installation page links the release inventory");
assert(conceptsHtml.includes(">Concepts</h1>"), "concepts page has its title");
for (const word of ["Roadmap", "Sessions", "Recorded work", "Verification", "Project state"]) {
  assert(conceptsHtml.includes(word), `concepts page covers ${word}`);
}
assert(conceptsHtml.includes("pinned") && conceptsHtml.includes("checkpoint"), "concepts page separates pin from checkpoint");
assert(architectureHtml.includes("magistr-cockpit") && architectureHtml.includes("magistr-tui"), "architecture page names the interface crates");
assert(developmentHtml.includes("cargo nextest run --workspace") && developmentHtml.includes("website-captures.sh"), "development page has the commands");
assert(deploymentHtml.includes("wrangler pages deploy"), "deployment page has the deploy command");
for (const slug of ["installation", "concepts", "architecture", "development", "deployment"]) {
  assert(installHtml.includes(`href="/docs/${slug}"`), `sidebar links ${slug}`);
}
for (const gone of ["configuration", "hooks", "skills"]) {
  assert(!existsSync(resolve(distDir, "docs", gone)), `no ${gone} page is built`);
}
assert(installHtml.includes("docs-sidebar") && installHtml.includes("data-pagefind-body"), "docs layout and search body");
assert(existsSync(resolve(distDir, "pagefind/pagefind.js")), "pagefind index built");

console.log("\nMetadata:");
for (const [name, html] of [["home", indexHtml], ["installation", installHtml]]) {
  assert(html.includes('rel="canonical" href="https://magistr.dev/'), `${name} canonical URL`);
  assert(html.includes('property="og:title"') && html.includes('property="og:image"') && html.includes('property="og:url"'), `${name} Open Graph tags`);
  assert(html.includes('name="twitter:card"') && html.includes('name="twitter:image"'), `${name} Twitter card`);
  assert(html.includes("application/ld+json"), `${name} structured data`);
}
assert(indexHtml.includes("og-image.png") && existsSync(resolve(distDir, "og-image.png")), "OG image shipped");
assert(indexHtml.includes("site.webmanifest") && indexHtml.includes("apple-touch-icon"), "manifest and touch icon linked");
assert(existsSync(resolve(distDir, "favicon.svg")) && existsSync(resolve(distDir, "favicon-32.png")), "favicons shipped");
assert(indexHtml.includes("prefers-color-scheme"), "system theme detection");
assert(indexHtml.includes("theme-toggle"), "theme toggle present");
const css = readdirSync(resolve(distDir, "_astro")).filter((f) => f.endsWith(".css")).map((f) => readFileSync(resolve(distDir, "_astro", f), "utf-8")).join("\n");
assert(css.includes("prefers-reduced-motion"), "reduced motion respected in the stylesheet");
assert(css.includes("aspect-ratio:1320/840") || css.includes("aspect-ratio: 1320 / 840"), "captures reserve their aspect ratio");

console.log("\nSitemap, robots and headers:");
assert(readFileSync(resolve(distDir, "sitemap-index.xml"), "utf-8").includes("sitemap"), "sitemap index generated");
const robotsTxt = readFileSync(resolve(distDir, "robots.txt"), "utf-8");
assert(robotsTxt.includes("User-agent: *") && robotsTxt.includes("Allow: /") && robotsTxt.includes("sitemap-index.xml"), "robots.txt allows all and names the sitemap");
const headersFile = readFileSync(resolve(distDir, "_headers"), "utf-8");
assert(headersFile.includes("immutable") && headersFile.includes("must-revalidate") && headersFile.includes("/_astro/*"), "caching rules present");
assert(headersFile.includes("X-Content-Type-Options") && headersFile.includes("X-Frame-Options"), "security headers present");

console.log("\n404:");
assert(notFoundHtml.includes("Page not found") && notFoundHtml.includes('href="/docs/installation"'), "404 page links back into the site");

console.log("\nSource hygiene:");
const components = readdirSync(resolve(srcDir, "components"));
assert(components.every((name) => ["Nav.astro", "ThemeToggle.astro"].includes(name)), `only the components in use remain (${components.join(", ")})`);
assert(!existsSync(resolve(distDir, "icons")), "no unused icon assets ship");

console.log(`\n${failures === 0 ? "ALL TESTS PASSED" : `${failures} TEST(S) FAILED`}`);
process.exit(failures === 0 ? 0 : 1);
