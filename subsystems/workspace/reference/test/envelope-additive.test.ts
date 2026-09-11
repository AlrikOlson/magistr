/**
 * Additive-evolution gate (SDD §4.2): within a major envelope version, only
 * new optional fields may be added. The live schema is compared against the
 * frozen v1 baseline; any removal, redefinition, or new required field fails.
 */
import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

function isObject(value: Json): value is { [key: string]: Json } {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Assert `current` is an additive evolution of `baseline`:
 * - under a `properties` node, `current` may add NEW keys (new fields);
 * - everywhere else, key sets, array contents and scalars must be identical —
 *   which pins every `required` array, enum, type and branch to the baseline.
 * Returns the list of violations instead of throwing, so tests can assert both
 * directions.
 */
function additiveViolations(
  baseline: Json,
  current: Json,
  path = "$",
  inProperties = false,
): string[] {
  if (isObject(baseline) && isObject(current)) {
    const violations: string[] = [];
    for (const key of Object.keys(baseline)) {
      if (!(key in current)) {
        violations.push(`${path}.${key}: removed`);
        continue;
      }
      const child = current[key] as Json;
      violations.push(
        ...additiveViolations(baseline[key] as Json, child, `${path}.${key}`, key === "properties"),
      );
    }
    if (!inProperties) {
      for (const key of Object.keys(current)) {
        if (!(key in baseline)) {
          violations.push(`${path}.${key}: added outside a properties node`);
        }
      }
    }
    return violations;
  }
  if (Array.isArray(baseline) && Array.isArray(current)) {
    return JSON.stringify(baseline) === JSON.stringify(current) ? [] : [`${path}: array changed`];
  }
  return baseline === current ? [] : [`${path}: value changed from ${JSON.stringify(baseline)}`];
}

async function loadJson(url: URL): Promise<Json> {
  return JSON.parse(await readFile(url, "utf8")) as Json;
}

const baselineUrl = new URL("../schemas/baselines/envelope.v1.schema.json", import.meta.url);
const liveUrl = new URL("../schemas/envelope.schema.json", import.meta.url);

describe("the live envelope schema vs the frozen v1 baseline", () => {
  test("is an additive evolution", async () => {
    const violations = additiveViolations(await loadJson(baselineUrl), await loadJson(liveUrl));
    expect(violations).toEqual([]);
  });
});

describe("the additive checker itself", () => {
  const schema: Json = {
    type: "object",
    required: ["v", "ok"],
    properties: {
      v: { type: "integer", minimum: 1 },
      ok: { type: "boolean" },
      notes: { type: "array", items: { type: "string" } },
    },
  };
  const clone = (): Json => structuredClone(schema);

  test("accepts an identical schema", () => {
    expect(additiveViolations(schema, clone())).toEqual([]);
  });

  test("accepts a new optional property", () => {
    const next = clone() as { properties: { [key: string]: Json } };
    next.properties.extra = { type: "string" };
    expect(additiveViolations(schema, next)).toEqual([]);
  });

  test("rejects a removed property", () => {
    const next = clone() as { properties: { [key: string]: Json } };
    delete next.properties.notes;
    expect(additiveViolations(schema, next)).not.toEqual([]);
  });

  test("rejects a new required field", () => {
    const next = clone() as { required: Json[]; properties: { [key: string]: Json } };
    next.properties.extra = { type: "string" };
    next.required.push("extra");
    expect(additiveViolations(schema, next)).not.toEqual([]);
  });

  test("rejects a changed type or narrowed constraint", () => {
    const retyped = clone() as { properties: { v: { type: Json } } };
    retyped.properties.v.type = "string";
    expect(additiveViolations(schema, retyped)).not.toEqual([]);

    const narrowed = clone() as { properties: { v: { [key: string]: Json } } };
    narrowed.properties.v.maximum = 10;
    expect(additiveViolations(schema, narrowed)).not.toEqual([]);
  });
});
