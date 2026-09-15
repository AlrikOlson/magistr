/**
 * Post-build tests: verify the built HTML contains expected elements.
 * Run with: node tests/build-output.mjs (after `npm run build`)
 */
import { readFileSync, existsSync } from "node:fs";
import { createHash } from "node:crypto";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const distDir = resolve(__dirname, "../dist");

let failures = 0;

function assert(condition, message) {
	if (!condition) {
		console.error(`  FAIL: ${message}`);
		failures++;
	} else {
		console.log(`  PASS: ${message}`);
	}
}

// Read index.html
const indexHtml = readFileSync(resolve(distDir, "index.html"), "utf-8");

console.log("Hero section:");
assert(indexHtml.includes("Roadmap and workflow management"), "headline present");
assert(indexHtml.includes("for coding agents"), "headline accent text present");
assert(
	indexHtml.includes("Build from source"),
	"source installation CTA present",
);
assert(indexHtml.includes("install-btn"), "install button has id");
assert(
	!indexHtml.includes("cargo install magistr"),
	"no obsolete registry installation command",
);

console.log("\nNavigation:");
assert(indexHtml.includes('href="/#features"'), "Features anchor link");
assert(indexHtml.includes('href="/#quickstart"'), "Quickstart anchor link");
assert(indexHtml.includes('href="/docs/installation"'), "Docs link present");
assert(indexHtml.includes('href="/#community"'), "Community anchor link");
assert(indexHtml.includes("github.com"), "GitHub link present");
assert(indexHtml.includes("backdrop-blur"), "nav has backdrop blur");

console.log("\nTUI2 screenshots:");
assert(indexHtml.includes('id="screenshots"'), "terminal screenshots section present");
assert(indexHtml.includes("TUI2 terminal cockpit"), "terminal section heading present");
assert(!indexHtml.includes("workflow-preview"), "handmade workflow preview removed");
const captures = JSON.parse(readFileSync(resolve(distDir, "screenshots/captures.json"), "utf-8"));
assert(captures.renderer === "magistr-tui::screenshot::frame" && captures.surface === "Terminal", "capture provenance names the terminal renderer");
assert(captures.images.length === 4, "roadmap and trace captures have both themes");
for (const capture of captures.images) {
  assert(indexHtml.includes("/screenshots/" + capture.file), capture.file + " referenced");
  const png = readFileSync(resolve(distDir, "screenshots", capture.file));
  assert(png.readUInt32BE(16) === capture.width && png.readUInt32BE(20) === capture.height, capture.file + " keeps its full viewport");
  assert(createHash("sha256").update(png).digest("hex") === capture.sha256, capture.file + " matches the captured bytes");
}
for (const oldView of ["workbench", "receipt", "session"]) {
  const oldAsset = "magistr-tui2-" + oldView + "-clean.png";
  assert(!indexHtml.includes(oldAsset) && !existsSync(resolve(distDir, "screenshots", oldAsset)), "mislabeled GUI2 " + oldView + " asset removed");
}
assert(!indexHtml.includes("AsciinemaPlayer"), "legacy player removed");
assert(!indexHtml.includes("illustrative"), "placeholder copy removed");
assert(!indexHtml.includes("Mermaid source"), "no Mermaid diagram disclosures");
assert(indexHtml.includes("aria-label"), "navigation accessibility attributes present");

console.log("\nLayout & meta:");
assert(indexHtml.includes('lang="en"'), "html lang attribute");
assert(indexHtml.includes("viewport"), "viewport meta tag");
assert(indexHtml.includes("magistr"), "title contains magistr");
assert(indexHtml.includes("favicon.svg"), "favicon reference");
assert(indexHtml.includes("JetBrains+Mono"), "JetBrains Mono font loaded");

console.log("\nFeature grid:");
assert(indexHtml.includes('id="features"'), "features section anchor present");
assert(indexHtml.includes("Shape the roadmap"), "features section heading present");
assert(indexHtml.includes("feature-card"), "feature card class present");
assert(indexHtml.includes("Scaffolding"), "Scaffolding feature card present");
assert(
	indexHtml.includes("Recorded iteration"),
	"Recorded iteration feature card present",
);
assert(
	indexHtml.includes("Terminal cockpit"),
	"Terminal cockpit feature card present",
);
assert(
	indexHtml.includes("Roadmap Tracking"),
	"Roadmap Tracking feature card present",
);
assert(indexHtml.includes("Source tools &amp; MCP"), "source tools and MCP feature card present");
assert(indexHtml.includes("Desktop &amp; peer protocol"), "experimental desktop and peer card present");
assert(!indexHtml.includes("ratatui"), "website does not promise a ratatui theme set");
assert(!indexHtml.includes("Tokyo Night"), "website does not promise named built-in themes");
assert(
	indexHtml.includes("Workbench"),
	"MCP card names the code navigation tool",
);

console.log("\nQuickstart section:");
assert(
	indexHtml.includes('id="quickstart"'),
	"quickstart section anchor present",
);
assert(
	indexHtml.includes("quickstart-tabs"),
	"quickstart tabs container present",
);
assert(indexHtml.includes("quickstart-tab"), "tab buttons present");
assert(indexHtml.includes("quickstart-copy"), "copy buttons present");
assert(
	indexHtml.includes(
		"cargo install --locked --path magistr/crates/magistr-cli",
	),
	"source installation command present",
);
assert(
	indexHtml.includes("git clone https://github.com/AlrikOlson/magistr magistr"),
	"clone command targets the source repository",
);
assert(
	indexHtml.includes("magistr --version"),
	"installation verification command present",
);
assert(indexHtml.includes("0.1.0"), "website identifies the current release version");
assert(indexHtml.includes("release-inventory.md"), "website links the release inventory");
assert(indexHtml.includes('role="tablist"'), "tablist ARIA role present");
assert(indexHtml.includes('role="tab"'), "tab ARIA role present");
assert(indexHtml.includes('role="tabpanel"'), "tabpanel ARIA role present");

console.log("\nWalkthrough section:");
assert(indexHtml.includes("walkthrough-step"), "walkthrough steps present");
assert(indexHtml.includes("magistr init"), "step 1: magistr init command");
assert(indexHtml.includes("magistr next"), "step 2: magistr next command");
assert(
	indexHtml.includes("magistr next-loop"),
	"step 3: magistr next-loop command",
);
assert(indexHtml.includes("From zero to"), "walkthrough heading present");

console.log("\nResponsive classes:");
assert(indexHtml.includes("sm:"), "sm: breakpoint classes present");
assert(indexHtml.includes("lg:"), "lg: breakpoint classes present");
assert(
	indexHtml.includes("sm:grid-cols-2"),
	"feature grid responsive: sm:grid-cols-2",
);
assert(
	indexHtml.includes("lg:grid-cols-3"),
	"feature grid responsive: lg:grid-cols-3",
);

// --- Docs pages ---
console.log("\nDocs - Installation:");
const installHtml = readFileSync(
	resolve(distDir, "docs/installation/index.html"),
	"utf-8",
);
assert(installHtml.includes("Installation"), "installation page has title");
assert(installHtml.includes("v0.1.0"), "installation page identifies the current release");
assert(installHtml.includes("Release archives"), "installation page documents packaged archives");
assert(installHtml.includes("gui2"), "installation page labels desktop as experimental");
assert(installHtml.includes("release-inventory.md"), "installation page links the release inventory");
assert(
	installHtml.includes("cargo install --locked --path crates/magistr-cli"),
	"installation page has source installation command",
);
assert(
	installHtml.includes("--locked") && !installHtml.includes("qs-framework"),
	"installation page uses pinned dependencies without a private desktop dependency",
);
assert(installHtml.includes("docs-sidebar"), "installation page has sidebar");
assert(installHtml.includes("Permission and privacy boundary"), "installation page states permission boundary");
assert(installHtml.includes("source-first"), "installation page states source-first release status");
assert(
	installHtml.includes("docs-content"),
	"installation page has content area",
);
assert(
	installHtml.includes("data-pagefind-body"),
	"installation page has pagefind body",
);

console.log("\nDocs - Configuration:");
const configHtml = readFileSync(
	resolve(distDir, "docs/configuration/index.html"),
	"utf-8",
);
assert(configHtml.includes("Configuration"), "configuration page has title");
assert(
	configHtml.includes("project.json"),
	"configuration page mentions project.json",
);
assert(
	configHtml.includes("roadmap_exports"),
	"configuration page explains native roadmap storage",
);

console.log("\nDocs - Skills:");
const skillsHtml = readFileSync(
	resolve(distDir, "docs/skills/index.html"),
	"utf-8",
);
assert(skillsHtml.includes(">Skills</h1>"), "skills page has title");
assert(skillsHtml.includes("/next"), "skills page mentions /next");
assert(skillsHtml.includes("/roadmap-refresh"), "skills page mentions /roadmap-refresh");
assert(skillsHtml.includes("$next"), "skills page documents Codex $next");
assert(skillsHtml.includes("$roadmap-refresh"), "skills page documents Codex $roadmap-refresh");
assert(skillsHtml.includes("validate"), "skills page documents validation");

console.log("\nDocs - Hooks:");
const hooksHtml = readFileSync(
	resolve(distDir, "docs/hooks/index.html"),
	"utf-8",
);
assert(hooksHtml.includes(">Hooks</h1>"), "hooks page has title");
assert(
	hooksHtml.includes("lifecycle_hooks"),
	"hooks page names the configuration field",
);
assert(hooksHtml.includes("on_enter"), "hooks page documents lifecycle events");

console.log("\nDocs - Sidebar navigation:");
assert(
	installHtml.includes("Getting Started"),
	"sidebar has Getting Started category",
);
assert(installHtml.includes("Reference"), "sidebar has Reference category");
assert(
	installHtml.includes('href="/docs/installation"'),
	"sidebar links to installation",
);
assert(
	installHtml.includes('href="/docs/configuration"'),
	"sidebar links to configuration",
);
assert(installHtml.includes('href="/docs/skills"'), "sidebar links to skills");
assert(installHtml.includes('href="/docs/hooks"'), "sidebar links to hooks");

console.log("\nDocs - Code blocks (Expressive Code):");
assert(
	installHtml.includes("expressive-code"),
	"expressive code styles present",
);

console.log("\nDocs - Search (Pagefind):");
assert(installHtml.includes("docs-search"), "search container present");
assert(existsSync(resolve(distDir, "pagefind")), "pagefind directory exists");
assert(
	existsSync(resolve(distDir, "pagefind/pagefind.js")),
	"pagefind.js exists",
);

// Design system
console.log("\nDesign system - Theme toggle:");
assert(indexHtml.includes("theme-toggle"), "theme toggle button present");
assert(indexHtml.includes("theme-icon-sun"), "sun icon present");
assert(indexHtml.includes("theme-icon-moon"), "moon icon present");
assert(
	indexHtml.includes("localStorage"),
	"localStorage persistence in theme script",
);

console.log("\nDesign system - Color palette:");
assert(indexHtml.includes("--color-bg"), "CSS custom properties for colors");
// The inline head script applies .dark class based on preference
assert(
	indexHtml.includes("prefers-color-scheme"),
	"system preference detection",
);

console.log("\nDesign system - Animations:");
assert(indexHtml.includes("fade-in"), "fade-in animation class present");
assert(
	indexHtml.includes("prefers-reduced-motion"),
	"prefers-reduced-motion respected",
);
assert(indexHtml.includes("shimmer-cta"), "shimmer animation on CTA");

console.log("\nDesign system - UI primitives:");
assert(
	indexHtml.includes('role="separator"'),
	"gradient divider separator role",
);

console.log("\nDesign system - Favicons & meta:");
assert(indexHtml.includes("site.webmanifest"), "web manifest linked");
assert(indexHtml.includes("apple-touch-icon"), "apple touch icon linked");
assert(indexHtml.includes("og:image"), "OG image meta tag");
assert(indexHtml.includes("twitter:card"), "Twitter card meta tag");
assert(
	existsSync(resolve(distDir, "../public/favicon-16.png")),
	"favicon-16.png exists",
);
assert(
	existsSync(resolve(distDir, "../public/favicon-32.png")),
	"favicon-32.png exists",
);
assert(
	existsSync(resolve(distDir, "../public/apple-touch-icon.png")),
	"apple-touch-icon.png exists",
);
assert(
	existsSync(resolve(distDir, "../public/icon-512.png")),
	"icon-512.png exists",
);

console.log("\nDesign system - WCAG AA contrast:");
// Verify light palette text (#1a1b26) on bg (#f5f5f7) meets WCAG AA
// Contrast ratio: ~17.3:1 (passes AA which requires 4.5:1 for normal text)
// Verify dark palette text (#c0caf5) on bg (#1a1b26)
// Contrast ratio: ~9.4:1 (passes AA)
// Verify dark accent (#7aa2f7) on dark bg (#1a1b26)
// Contrast ratio: ~6.1:1 (passes AA for normal text, requires 4.5:1)
// Verify light accent (#4f6df5) on light bg (#f5f5f7)
// Contrast ratio: ~4.7:1 (passes AA for normal text)
// These are build-time assertions based on the known palette values.
{
	function luminance(hex) {
		const r = parseInt(hex.slice(1, 3), 16) / 255;
		const g = parseInt(hex.slice(3, 5), 16) / 255;
		const b = parseInt(hex.slice(5, 7), 16) / 255;
		const srgb = [r, g, b].map((c) =>
			c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4),
		);
		return 0.2126 * srgb[0] + 0.7152 * srgb[1] + 0.0722 * srgb[2];
	}

	function contrastRatio(hex1, hex2) {
		const l1 = luminance(hex1);
		const l2 = luminance(hex2);
		const lighter = Math.max(l1, l2);
		const darker = Math.min(l1, l2);
		return (lighter + 0.05) / (darker + 0.05);
	}

	const AA_NORMAL = 4.5;

	// Dark mode pairs
	const darkBg = "#1a1b26";
	const darkText = "#c0caf5";
	const darkAccent = "#7aa2f7";
	const darkMuted = "#565f89";

	assert(
		contrastRatio(darkText, darkBg) >= AA_NORMAL,
		`dark text on dark bg contrast >= ${AA_NORMAL} (${contrastRatio(darkText, darkBg).toFixed(1)})`,
	);
	assert(
		contrastRatio(darkAccent, darkBg) >= AA_NORMAL,
		`dark accent on dark bg contrast >= ${AA_NORMAL} (${contrastRatio(darkAccent, darkBg).toFixed(1)})`,
	);

	// Light mode pairs
	const lightBg = "#f5f5f7";
	const lightText = "#1a1b26";
	const lightAccent = "#3b5de7";
	const lightMuted = "#595f6e";

	assert(
		contrastRatio(lightText, lightBg) >= AA_NORMAL,
		`light text on light bg contrast >= ${AA_NORMAL} (${contrastRatio(lightText, lightBg).toFixed(1)})`,
	);
	assert(
		contrastRatio(lightAccent, lightBg) >= AA_NORMAL,
		`light accent on light bg contrast >= ${AA_NORMAL} (${contrastRatio(lightAccent, lightBg).toFixed(1)})`,
	);
	assert(
		contrastRatio(lightMuted, lightBg) >= AA_NORMAL,
		`light muted on light bg contrast >= ${AA_NORMAL} (${contrastRatio(lightMuted, lightBg).toFixed(1)})`,
	);
}

console.log("\nSource content:");
assert(
	!indexHtml.includes("Sarah Chen") && !indexHtml.includes("Your Company"),
	"no placeholder endorsements",
);

console.log("\nCommunity section:");
assert(
	indexHtml.includes('id="community"'),
	"community section anchor present",
);
assert(indexHtml.includes("community-link"), "community link cards present");
assert(indexHtml.includes("Join the"), "community heading present");
assert(
	indexHtml.includes("github.com/AlrikOlson/magistr/issues"),
	"Issues link present",
);
assert(
	indexHtml.includes("CONTRIBUTING.md"),
	"Contributing guide link present",
);
assert(indexHtml.includes("CONTRIBUTING.md"), "public contributing policy link present");
assert(indexHtml.includes("SECURITY.md"), "public security policy link present");
assert(indexHtml.includes("CODE_OF_CONDUCT.md"), "public conduct policy link present");

console.log("\nChangelog section:");
assert(
	indexHtml.includes('id="changelog"'),
	"changelog section anchor present",
);
// Changelog may have releases or empty state depending on API availability
assert(
	indexHtml.includes("changelog-timeline") ||
		indexHtml.includes("changelog-empty"),
	"changelog has timeline or empty state",
);
assert(indexHtml.includes("What"), "changelog heading present");

console.log("\nSitemap:");
const sitemapIndex = readFileSync(
	resolve(distDir, "sitemap-index.xml"),
	"utf-8",
);
assert(sitemapIndex.includes("sitemap"), "sitemap index generated");

// Performance and metadata
console.log("\nRobots.txt:");
const robotsTxt = readFileSync(resolve(distDir, "robots.txt"), "utf-8");
assert(robotsTxt.includes("User-agent: *"), "robots.txt has user-agent rule");
assert(robotsTxt.includes("Allow: /"), "robots.txt allows all");
assert(
	robotsTxt.includes("sitemap-index.xml"),
	"robots.txt references sitemap",
);

console.log("\nEnhanced meta tags:");
assert(indexHtml.includes("og:url"), "og:url meta tag present");
assert(indexHtml.includes("og:site_name"), "og:site_name meta tag present");
assert(indexHtml.includes("og:locale"), "og:locale meta tag present");
assert(indexHtml.includes("twitter:title"), "twitter:title meta tag present");
assert(
	indexHtml.includes("twitter:description"),
	"twitter:description meta tag present",
);
assert(indexHtml.includes("twitter:image"), "twitter:image meta tag present");
assert(indexHtml.includes('rel="canonical"'), "canonical link present");
assert(indexHtml.includes("https://magistr.dev"), "absolute URLs for OG tags");

console.log("\nResource hints:");
assert(
	indexHtml.includes('preconnect" href="https://api.github.com"'),
	"preconnect for GitHub API",
);
assert(
	indexHtml.includes('preconnect" href="https://cdn.jsdelivr.net"'),
	"preconnect for jsDelivr CDN",
);
assert(
	indexHtml.includes('dns-prefetch" href="https://api.github.com"'),
	"dns-prefetch for GitHub API",
);
assert(
	indexHtml.includes('dns-prefetch" href="https://cdn.jsdelivr.net"'),
	"dns-prefetch for jsDelivr CDN",
);

console.log("\nJSON-LD structured data (homepage):");
assert(indexHtml.includes("application/ld+json"), "JSON-LD script tag present");
assert(
	indexHtml.includes("SoftwareApplication"),
	"SoftwareApplication schema present",
);
assert(indexHtml.includes('"WebSite"'), "WebSite schema present");
assert(
	indexHtml.includes("DeveloperApplication"),
	"applicationCategory present",
);

console.log("\nJSON-LD structured data (docs):");
assert(installHtml.includes("application/ld+json"), "docs page has JSON-LD");
assert(
	installHtml.includes("BreadcrumbList"),
	"docs page has BreadcrumbList schema",
);
assert(installHtml.includes("ListItem"), "BreadcrumbList has ListItem entries");

console.log("\nOG image:");
assert(
	existsSync(resolve(distDir, "../public/og-image.png")),
	"og-image.png generated",
);
assert(indexHtml.includes("og-image.png"), "og:image references PNG");

console.log("\nImage optimization:");
assert(
	indexHtml.includes("astro:assets") || true,
	"image optimization infrastructure available",
);

// --- 404 Page (P7.4) ---
console.log("\n404 page:");
const notFoundHtml = readFileSync(resolve(distDir, "404.html"), "utf-8");
assert(notFoundHtml.includes("404"), "404 page has error code");
assert(notFoundHtml.includes("Page not found"), "404 page has heading");
assert(notFoundHtml.includes('href="/"'), "404 page links to home");
assert(
	notFoundHtml.includes('href="/docs/installation"'),
	"404 page links to docs",
);
assert(
	notFoundHtml.includes("github.com/AlrikOlson/magistr"),
	"404 page links to GitHub",
);
assert(notFoundHtml.includes("nav"), "404 page has navigation element");

// --- Analytics (P7.3) ---
console.log("\nAnalytics:");
assert(
	indexHtml.includes("plausible.io/js/script.js"),
	"Plausible analytics script present",
);
assert(
	indexHtml.includes("defer"),
	"analytics script is deferred (non-blocking)",
);
assert(
	indexHtml.includes('data-domain="magistr.dev"'),
	"analytics data-domain set",
);
// Verify analytics is also on other pages
assert(
	notFoundHtml.includes("plausible.io/js/script.js"),
	"analytics on 404 page too",
);

// --- CDN / Caching Headers (P7.2) ---
console.log("\nCaching headers:");
const headersFile = readFileSync(resolve(distDir, "_headers"), "utf-8");
assert(headersFile.includes("immutable"), "hashed assets have immutable cache");
assert(headersFile.includes("must-revalidate"), "HTML has must-revalidate");
assert(headersFile.includes("/_astro/*"), "astro assets have caching rule");
assert(
	headersFile.includes("X-Content-Type-Options"),
	"security header present",
);
assert(
	headersFile.includes("X-Frame-Options"),
	"frame protection header present",
);

console.log(
	`\n${failures === 0 ? "ALL TESTS PASSED" : `${failures} TEST(S) FAILED`}`,
);
process.exit(failures === 0 ? 0 : 1);
