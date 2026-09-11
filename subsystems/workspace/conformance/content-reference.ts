/** Shared byte vectors from the pinned Workbench file-content contract. */
import assert from "node:assert/strict";
import { contentBytes, contentEqual, contentHash, decodeContent } from "../reference/src/envelope/file-content";
import fixtures from "./content.json";

for (const fixture of fixtures) {
  const bytes = Uint8Array.from(fixture.bytes);
  const content = decodeContent(bytes);
  assert.equal(typeof content === "string" ? "text" : "binary", fixture.kind);
  assert.deepEqual(Array.from(contentBytes(content)), fixture.bytes);
  assert.equal(contentHash(content), fixture.hash);
  assert.equal(contentEqual(content, bytes), true);
  assert.equal(contentEqual(content, null), false);
}
process.stdout.write("Reference byte/text classification and SHA-256 fixtures passed.\n");
