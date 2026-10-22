/**
 * Generate every raster website identity asset from its reviewed SVG source,
 * and the social image from the brand module. Social text is outlined from the
 * vendored Instrument Sans files at generation time, so the result does not
 * depend on which fonts the building machine has installed.
 */
import { createRequire } from "node:module";
import { publicAssetUrl } from "./asset-url.mjs";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import opentype from "opentype.js";
import { siteName, tagline, definition, siteUrl } from "../src/data/brand.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const publicDir = resolve(here, "../public");
const require = createRequire(import.meta.url);
const sharp = require("sharp");

const svg = readFileSync(resolve(publicDir, "favicon.svg"));
for (const [name, size] of [
  ["favicon-16.png", 16], ["favicon-32.png", 32], ["apple-touch-icon.png", 180],
  ["icon-192.png", 192], ["icon-512.png", 512],
]) {
  await sharp(svg, { density: 300 }).resize(size, size).png().toFile(resolve(publicDir, name));
}

const font = (file) => opentype.parse(readFileSync(resolve(here, "fonts", file)).buffer.slice(0));
const semibold = font("InstrumentSans-SemiBold.ttf"), regular = font("InstrumentSans-Regular.ttf");
const fixed = (value) => Number(value.toFixed(2));

/** Advance width of `text` in CSS pixels at `size`, with pair kerning. */
function measure(font, text, size) {
  const scale = size / font.unitsPerEm;
  let width = 0, previous = null;
  for (const character of text) {
    const glyph = font.charToGlyph(character);
    if (previous) { const kern = font.getKerningValue(previous, glyph); if (Number.isFinite(kern)) width += kern * scale; }
    width += glyph.advanceWidth * scale; previous = glyph;
  }
  return width;
}

/**
 * Wrap by measured width into as few lines as a greedy wrap needs, then
 * choose the break points that keep the widest line narrowest, the way the
 * page's balanced headline wraps.
 */
function wrap(font, text, size, maxWidth) {
  const words = text.split(" ");
  const width = (from, to) => measure(font, words.slice(from, to + 1).join(" "), size);
  let lines = 1;
  for (let start = 0, i = 0; i < words.length; i++) {
    if (i > start && width(start, i) > maxWidth) { lines++; start = i; }
  }
  const memo = new Map();
  const best = (from, remaining) => {
    if (remaining === 1) return { widest: width(from, words.length - 1), breaks: [] };
    const key = from + ":" + remaining;
    if (memo.has(key)) return memo.get(key);
    let choice = { widest: Infinity, breaks: [] };
    for (let end = from; end <= words.length - remaining; end++) {
      const rest = best(end + 1, remaining - 1);
      const widest = Math.max(width(from, end), rest.widest);
      if (widest < choice.widest) choice = { widest, breaks: [end, ...rest.breaks] };
    }
    memo.set(key, choice);
    return choice;
  };
  const { breaks } = best(0, lines);
  const out = [];
  let start = 0;
  for (const end of [...breaks, words.length - 1]) { out.push(words.slice(start, end + 1).join(" ")); start = end + 1; }
  return out;
}

/**
 * Outline `text` as one SVG path. Glyph commands come straight from the font
 * tables; opentype.js's own text layout trips on this font's GSUB tables.
 */
function outline(font, text, x, y, size, fill) {
  const scale = size / font.unitsPerEm;
  let cursor = x, data = "", previous = null;
  for (const character of text) {
    const glyph = font.charToGlyph(character);
    if (previous) { const kern = font.getKerningValue(previous, glyph); if (Number.isFinite(kern)) cursor += kern * scale; }
    const X = (v) => fixed(cursor + v * scale), Y = (v) => fixed(y - v * scale);
    for (const c of glyph.path.commands) {
      if (c.type === "M" || c.type === "L") data += `${c.type}${X(c.x)} ${Y(c.y)}`;
      else if (c.type === "Q") data += `Q${X(c.x1)} ${Y(c.y1)} ${X(c.x)} ${Y(c.y)}`;
      else if (c.type === "C") data += `C${X(c.x1)} ${Y(c.y1)} ${X(c.x2)} ${Y(c.y2)} ${X(c.x)} ${Y(c.y)}`;
      else if (c.type === "Z") data += "Z";
    }
    cursor += glyph.advanceWidth * scale; previous = glyph;
  }
  if (/NaN/.test(data)) throw new Error(`Glyph outline failed in: ${text}`);
  return `<path fill="${fill}" d="${data}"/>`;
}

const width = 1200, height = 630, margin = 64, measureWidth = width - 2 * margin;
const headline = wrap(semibold, tagline, 68, measureWidth);
const lede = wrap(regular, definition, 28, measureWidth);
let y = 255;
const text = [];
text.push(outline(semibold, siteName, 138, 103, 38, "#edf0e7"));
for (const line of headline) { text.push(outline(semibold, line, margin, y, 68, "#edf0e7")); y += 85; }
y += 5;
for (const line of lede) { text.push(outline(regular, line, margin, y, 28, "#b0b9ae")); y += 44; }
text.push(outline(regular, siteUrl.replace(/^https?:\/\//, ""), margin, height - 62, 24, "#b0b9ae"));
const escape = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");
const social = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}" width="${width}" height="${height}" role="img" aria-labelledby="title desc">
  <!-- Generated by scripts/generate-favicons.mjs from src/data/brand.mjs. Do not edit by hand. -->
  <title id="title">${escape(siteName)}: ${escape(tagline)}</title>
  <desc id="desc">${escape(definition)}</desc>
  <rect width="${width}" height="${height}" fill="#141916"/>
  <path d="M64 114V64l25 34 25-34v50" fill="none" stroke="#ff9b66" stroke-width="7"/>
  <path d="M80 114h18" stroke="#e7ebe3" stroke-width="5"/>
  ${text.join("\n  ")}
</svg>
`;
writeFileSync(resolve(publicDir, "og-image.svg"), social);
for (const name of ["og-image.png", "github-social.png"]) {
  await sharp(Buffer.from(social), { density: 150 }).resize(width, height).png().toFile(resolve(publicDir, name));
}

const manifestPath = resolve(publicDir, "site.webmanifest");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
manifest.description = tagline;
for (const icon of manifest.icons) {
  icon.src = publicAssetUrl(icon.src.split("?")[0].slice(1), publicDir);
}
writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + "\n");
console.log(`Generated five icons, the outlined social image (${headline.length + lede.length} text lines) and the versioned manifest.`);
