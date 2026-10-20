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
const releaseVersion = JSON.parse(readFileSync(new URL("../../../npm/magistr/package.json", import.meta.url), "utf8")).version;
assert(indexHtml.includes(`v${releaseVersion}`), "the current development version is named");
assert(indexHtml.includes(`"softwareVersion":"${releaseVersion}"`), "structured metadata uses the release version");
assert(!indexHtml.includes("cargo install magistr"), "no registry installation command");
assert(indexHtml.includes("Registry packages are not published yet") && indexHtml.includes("repository access"), "preview availability is explicit");
assert(indexHtml.includes('"operatingSystem":"macOS (Apple Silicon)"'), "first release metadata targets Apple Silicon macOS");
assert(indexHtml.includes('"downloadUrl":"https://magistr.dev/docs/installation"'), "download metadata points to current installation instructions");

assert(!indexHtml.includes("feature-card") && !indexHtml.includes("walkthrough-step") && !indexHtml.includes("community-link") && !indexHtml.includes("changelog"), "no marketing card grids or changelog");
const homeImages = [...indexHtml.matchAll(/src="\/screenshots\/([a-z-]+)\.png\?v=[a-f0-9]{12}"/g)].map((m) => m[1]);
assert(new Set(homeImages).size === 4 && homeImages.length === 4, `each screenshot appears once on the home page (${homeImages.join(", ")})`);
const roadmapSection = indexHtml.match(/<section id="roadmap"[\s\S]*?<\/section>/)?.[0] ?? "";
assert(roadmapSection.includes("magistr next") && roadmapSection.includes("acceptance criteria"), "roadmap section describes the current command workflow");
assert(!roadmapSection.includes("Roadmap screen"),"future desktop planning is not advertised");
assert(!roadmapSection.includes("without leaving the conversation") && !roadmapSection.includes("actions beside the conversation"), "full Roadmap is not described as conversation details");

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

console.log("\nProduct screenshots:");
const manifest=JSON.parse(readFileSync(resolve(distDir,"screenshots/captures.json"),"utf-8"));
assert(manifest.annotations===false && manifest.images.length===4,"four unannotated product screenshots");
assert(manifest.images.map(({file})=>file).sort().join(",")==="claude-code-session.png,codex-session.png,desktop-dark.png,desktop-light.png","desktop appearances and both agent sessions are present");
for(const capture of manifest.images){
 assert(Boolean(capture.renderer && capture.captured_at && capture.source_commit && capture.evidence),"each image names its renderer, source and retained evidence");
 const png=readFileSync(resolve(distDir,"screenshots",capture.file));
 assert(createHash("sha256").update(png).digest("hex")===capture.sha256,"capture matches reviewed bytes");
 assert(png.readUInt32BE(16)===capture.width && png.readUInt32BE(20)===capture.height,"capture keeps its full native viewport");
 const url=`/screenshots/${capture.file}?v=${capture.sha256.slice(0,12)}`;
 assert(indexHtml.includes(`src="${url}"`) && indexHtml.includes(`href="${url}"`),"image and full-size link use content hashes");
 const tag=[...indexHtml.matchAll(/<img [^>]*>/g)].map(([tag])=>tag).find(tag=>tag.includes(`src="${url}"`));
 assert(tag?.includes(`width="${capture.width}"`) && tag?.includes(`height="${capture.height}"`),"image dimensions reserve its actual aspect ratio");
}
assert(!readdirSync(resolve(distDir,"screenshots")).some(name=>name.startsWith("terminal-")||name.startsWith("tui2-")),"retired captures are absent");
const figures=[...indexHtml.matchAll(/<figure\b[\s\S]*?<\/figure>/g)];
assert(figures.length===3 && figures.every(([figure])=>figure.includes("<figcaption>")),"desktop and both agent sessions have captioned figures");

console.log("\nPublic language:");
const jargon = /TUI2|GUI2|tui2|gui2|think:|ship:|chunk-id|chunk\b|workflow server|native state|admitted|typed command|[Ff]light [Rr]ecorder|\b[Dd]ecisions?\b/;
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
const installText = installHtml.replace(/<[^>]*>/g, "").replace(/&#x26;/g, "&");
for (const command of [
  "magistr init . --agent codex,claude --dry-run",
  "magistr init . --agent codex --yes",
  "magistr init . --agent claude --yes",
  "magistr init . --agent portable --yes",
  "magistr doctor .",
  "magistr repair . --dry-run",
  "magistr remove . --dry-run",
  "cargo uninstall magistr-cli",
]) {
  assert(installText.includes(command), `current setup and recovery command: ${command}`);
}
assert(installText.includes("Only selected integrations are generated.") && !installText.includes("setup is always written"), "setup writes only selected integrations");
assert(installText.includes("Setup does not start implementation.") && installText.includes("starter prompt"), "onboarding finishes with planning");
assert(installText.includes("MCP server does not repair project files automatically"), "repair is explicit");
assert(installText.includes("registry packages are not published yet"), "installation guide does not advertise unpublished downloads");

assert(!installHtml.includes("release-inventory") && !installHtml.includes("Release archives"), "installation page carries no release-process links");
assert(conceptsHtml.includes(">Concepts</h1>"), "concepts page has its title");
for (const word of ["Roadmap", "Sessions", "Recorded work", "Verification", "Project state"]) {
  assert(conceptsHtml.includes(word), `concepts page covers ${word}`);
}
assert(conceptsHtml.includes("pinned") && conceptsHtml.includes("checkpoint"), "concepts page separates pin from checkpoint");
assert(conceptsHtml.includes("current desktop focuses on conversations"),"concepts states the current desktop scope");
assert(conceptsHtml.includes("recover your draft"),"concepts explains resident drafts");
assert(conceptsHtml.includes("Stop") && conceptsHtml.includes("Permission requests") && conceptsHtml.includes("Close session"),"current conversation actions are explained");
assert(roadmapSection.includes("recorded plan"),"roadmap recovery is explained");
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
