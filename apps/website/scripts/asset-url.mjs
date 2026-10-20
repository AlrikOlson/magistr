import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

/** Give public assets a new cache key when their actual bytes change. */
export function publicAssetUrl(file, publicDirectory = resolve(process.cwd(), "public")) {
  const hash = createHash("sha256").update(readFileSync(resolve(publicDirectory, file))).digest("hex").slice(0, 12);
  return `/${file}?v=${hash}`;
}
