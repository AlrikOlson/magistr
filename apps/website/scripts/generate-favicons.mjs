/** Generate every raster website identity asset from its reviewed SVG source. */
import { createRequire } from "node:module";
import { publicAssetUrl } from "./asset-url.mjs";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const publicDir = resolve(dirname(fileURLToPath(import.meta.url)), "../public");
const require = createRequire(import.meta.url);
const sharp = require("sharp");
const svg = readFileSync(resolve(publicDir, "favicon.svg"));
for (const [name, size] of [
  ["favicon-16.png", 16], ["favicon-32.png", 32], ["apple-touch-icon.png", 180],
  ["icon-192.png", 192], ["icon-512.png", 512],
]) {
  await sharp(svg, { density: 300 }).resize(size, size).png().toFile(resolve(publicDir, name));
}
const ogSvg = readFileSync(resolve(publicDir, "og-image.svg"));
for (const name of ["og-image.png", "github-social.png"]) {
  await sharp(ogSvg, { density: 150 }).resize(1200, 630).png().toFile(resolve(publicDir, name));
}
const manifestPath = resolve(publicDir, "site.webmanifest");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
for (const icon of manifest.icons) {
  icon.src = publicAssetUrl(icon.src.split("?")[0].slice(1), publicDir);
}
writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + "\n");
console.log("Generated five icons, both social previews and versioned manifest icons.");
