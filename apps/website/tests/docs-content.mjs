import assert from "node:assert/strict";
import { readFileSync, readdirSync, existsSync, statSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repo = resolve(root, "../..");
const dist = resolve(root, "dist");
export const docIds = readdirSync(resolve(root, "src/content/docs")).filter(file => file.endsWith(".mdx")).map(file => file.slice(0, -4)).sort();
export const docUrl = id => id === "index" ? "/docs/" : `/docs/${id}/`;
const pages = new Map(docIds.map(id => [docUrl(id), readFileSync(resolve(dist, "." + docUrl(id), "index.html"), "utf8")]));
pages.set("/", readFileSync(resolve(dist, "index.html"), "utf8"));
pages.set("/404.html", readFileSync(resolve(dist, "404.html"), "utf8"));
const home = pages.get("/docs/");
const directoryIds = [...home.matchAll(/<a[^>]+data-doc-id="([^"]+)"/g)].map(match => match[1]).sort();
assert.deepEqual(directoryIds, docIds.filter(id => id !== "index"), "The collection drives the complete documentation directory");
const redirects = readFileSync(resolve(dist, "_redirects"), "utf8").split("\n").filter(line => line && !line.startsWith("#")).map(line => line.trim().split(/\s+/));
const decode = text => text.replaceAll("&amp;", "&").replaceAll("&#x26;", "&");
function destination(url) {
  const path = url.pathname.endsWith("/") || url.pathname.endsWith(".html") ? url.pathname : url.pathname + "/";
  return pages.get(path);
}
function checkLink(href, base) {
  const url = new URL(decode(href), "https://magistr.dev" + base);
  if (url.origin !== "https://magistr.dev") {
    const prefix = "https://github.com/AlrikOlson/magistr/blob/main/";
    if (url.href.startsWith(prefix)) {
      const path = decodeURIComponent(url.pathname.split("/blob/main/")[1]);
      assert(existsSync(resolve(repo, path)), `Missing repository target: ${href}`);
    }
    return;
  }
  const html = destination(url);
  if (!html) {
    assert(existsSync(resolve(dist, "." + url.pathname)), `Missing route: ${href} from ${base}`);
    return;
  }
  if (url.hash) {
    const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map(match => match[1]);
    assert(ids.includes(decodeURIComponent(url.hash.slice(1))), `Missing anchor: ${href} from ${base}`);
  }
}
for (const [url, html] of pages) {
  for (const [, href] of html.matchAll(/<a\b[^>]*\bhref="([^"]*)"/g)) checkLink(href, url);
}
for (const [from, to, status] of redirects) {
  assert.equal(status, "301");
  assert(!pages.has(from.replace(/\/$/, "") + "/"), `Redirect shadows a current page: ${from}`);
  checkLink(to, from);
}
for (const id of docIds) {
  const html = pages.get(docUrl(id));
  const source = readFileSync(resolve(root, "src/content/docs", id + ".mdx"), "utf8");
  const nav = html.match(/<nav aria-label="Documentation pages">([\s\S]*?)<\/nav>/)?.[1];
  assert(nav, `Missing grouped navigation: ${id}`);
  assert.equal((nav.match(/aria-current="page"/g) ?? []).length, 1, `Exactly one current documentation page: ${id}`);
  for (const other of docIds) assert(nav.includes(`href="${docUrl(other).replace(/\/$/, "")}"`), `Missing ${other} in ${id} navigation`);
  assert(html.includes('aria-label="Breadcrumb"') && html.includes('aria-label="Related pages"'), `Page relationships: ${id}`);
  assert(html.includes('data-pagefind-filter="collection:docs"'), `Documentation search filter: ${id}`);
  assert(html.includes('data-pagefind-meta="title"'), `Clean search title: ${id}`);
  assert.equal((html.match(/<h1[\s>]/g) ?? []).length, 1, `One title: ${id}`);
  assert(!/Previous guide|Keep going/.test(html), `No unrelated global sequence: ${id}`);
  assert(!/For the game|the opponent|as we saw|I wanted for my own/.test(source), `Standalone prose: ${id}`);
  const related = source.match(/^related: \[(.*)\]$/m)?.[1].split(",").map(value => value.trim());
  assert(related?.length, `Explicit related metadata: ${id}`);
  const relatedHtml = html.match(/<nav class="docs-related"[\s\S]*?<\/nav>/)?.[0];
  for (const target of related) assert(relatedHtml?.includes(`href="${docUrl(target).replace(/\/$/, "")}"`), `Related target ${target} on ${id}`);
  const canonical = html.match(/rel="canonical" href="([^"]+)"/)?.[1];
  assert.equal(new URL(canonical).pathname, docUrl(id), `Canonical documentation route: ${id}`);
}
assert(existsSync(resolve(dist, "pagefind/pagefind.js")), "Search index is built");
console.log(`Documentation content passed: ${docIds.length} collection pages, metadata, related links, repository targets, routes, anchors and redirects.`);
