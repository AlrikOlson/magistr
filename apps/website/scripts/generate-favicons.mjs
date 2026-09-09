/**
 * Generate PNG favicons from favicon.svg.
 * Run automatically via `npm run prebuild`.
 */
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const publicDir = resolve(__dirname, "../public");
const svg = readFileSync(resolve(publicDir, "favicon.svg"));

// sharp may be globally installed or local — try both
let sharp;
try {
  const require = createRequire(import.meta.url);
  sharp = require("sharp");
} catch {
  console.warn("sharp not found — skipping favicon PNG generation");
  process.exit(0);
}

const sizes = [
  { name: "favicon-16.png", size: 16 },
  { name: "favicon-32.png", size: 32 },
  { name: "apple-touch-icon.png", size: 180 },
  { name: "icon-512.png", size: 512 },
];

for (const { name, size } of sizes) {
  await sharp(svg, { density: 300 })
    .resize(size, size)
    .png()
    .toFile(resolve(publicDir, name));
}

console.log(`Generated ${sizes.length} favicon PNGs`);

// Generate OG image PNG from og-image.svg
const ogSvg = readFileSync(resolve(publicDir, "og-image.svg"));
await sharp(ogSvg, { density: 150 })
  .resize(1200, 630)
  .png()
  .toFile(resolve(publicDir, "og-image.png"));

console.log("Generated og-image.png");
