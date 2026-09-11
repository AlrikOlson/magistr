import { expect, test } from "bun:test";
import { readdir } from "node:fs/promises";
import { envelopeSchema, toolSchemas } from "../src/generated/schemas";

const TOOL_SUFFIX = ".input.schema.json";

test("every tool is emitted from a file in schemas/tools", async () => {
  const entries = await readdir(new URL("../schemas/tools", import.meta.url));
  const expected = entries
    .filter((name) => name.endsWith(TOOL_SUFFIX))
    .map((name) => name.slice(0, -TOOL_SUFFIX.length))
    .sort();

  expect(Object.keys(toolSchemas).sort()).toEqual(expected);
});

test("every tool's outputSchema is the one response envelope", () => {
  for (const [name, schema] of Object.entries(toolSchemas)) {
    expect(schema.outputSchema, `${name} must return the ResponseEnvelope`).toBe(envelopeSchema);
  }
});

test("the envelope requires its version field", () => {
  expect(envelopeSchema.required).toContain("v");
});
