/**
 * The one-code-one-recovery schema test (SDD §4.3, Constitution II,
 * chunk:m0-error-taxonomy). Validates the raw schemas/error-taxonomy.json —
 * the source of truth — so a code mapped to zero or two recovery classes
 * fails here, plus the §4.3 completeness list, the SDD-pinned recoveries,
 * the required actionable payloads, and the taxonomyError builder.
 */
import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import {
  ERROR_CODES,
  type ErrorCode,
  isErrorCode,
  negotiateEnvelopeVersion,
  recoveryFor,
  taxonomyError,
} from "../src/envelope";
import { envelopeSchema, errorTaxonomy } from "../src/generated/schemas";
import { envelopeValidatorFor } from "./support/ajv";

const RECOVERY_CLASSES = ["retry", "narrow", "escalate", "confirm", "abort"];

/** The complete §4.3 list, PATH_OUTSIDE_WORKSPACE through CONFIG_POLICY_DENIED. */
const SDD_CODES = [
  "PATH_OUTSIDE_WORKSPACE",
  "PROTECTED_PATH",
  "AMBIGUOUS_SYMBOL",
  "SYMBOL_NOT_FOUND",
  "FILE_NOT_FOUND",
  "PARSE_REJECTED",
  "STALE_ANCHOR",
  "PATTERN_INVALID",
  "BUDGET_TOO_SMALL",
  "COMMAND_NOT_DECLARED",
  "RUN_REDIRECTED",
  "SANDBOX_FUEL_EXCEEDED",
  "SANDBOX_CAP_DENIED",
  "SANDBOX_MEMORY_EXCEEDED",
  "INDEX_UNAVAILABLE",
  "CONFIRM_REQUIRED",
  "CONFIRM_MISMATCH",
  "UNSUPPORTED_VERSION",
  "ENGINE_UNAVAILABLE",
  "ENGINE_BUSY",
  "INPUT_QUARANTINED",
  "PLUGIN_CAP_DENIED",
  "PLUGIN_CRASHED",
  "CONFIG_POLICY_DENIED",
];

/** Recoveries §4.3 (and Constitution I) pin explicitly. */
const SDD_PINNED_RECOVERY: Record<string, string> = {
  AMBIGUOUS_SYMBOL: "narrow",
  FILE_NOT_FOUND: "narrow",
  COMMAND_NOT_DECLARED: "abort",
  RUN_REDIRECTED: "abort",
  INDEX_UNAVAILABLE: "retry",
  UNSUPPORTED_VERSION: "abort",
  ENGINE_UNAVAILABLE: "retry",
  ENGINE_BUSY: "retry",
  INPUT_QUARANTINED: "escalate",
  CONFIG_POLICY_DENIED: "abort",
  SANDBOX_MEMORY_EXCEEDED: "narrow",
};

/** Actionable payloads §4.3 specifies per code. */
const SDD_PAYLOADS: Record<string, readonly string[]> = {
  AMBIGUOUS_SYMBOL: ["candidates"],
  PARSE_REJECTED: ["parserMessage", "rejectedDiff"],
  STALE_ANCHOR: ["contentHash"],
  ENGINE_BUSY: ["backoffMs"],
  PLUGIN_CRASHED: ["plugin"],
};

interface RawEntry {
  readonly code?: unknown;
  readonly recovery?: unknown;
  readonly payload?: unknown;
}

/**
 * Return every one-code-one-recovery violation in a taxonomy `codes` array.
 * A duplicated code is two mappings; a missing, non-string, or unknown
 * recovery is zero mappings. Returns violations rather than throwing so the
 * checker itself can be tested in both directions.
 */
function recoveryViolations(entries: readonly RawEntry[]): string[] {
  const violations: string[] = [];
  const seen = new Map<string, number>();
  for (const [index, entry] of entries.entries()) {
    const code = entry.code;
    if (typeof code !== "string" || code.length === 0) {
      violations.push(`entry ${index}: missing or non-string code`);
      continue;
    }
    seen.set(code, (seen.get(code) ?? 0) + 1);
    if (typeof entry.recovery !== "string" || !RECOVERY_CLASSES.includes(entry.recovery)) {
      violations.push(
        `${code}: maps to zero valid recovery classes (got ${JSON.stringify(entry.recovery)})`,
      );
    }
  }
  for (const [code, count] of seen) {
    if (count > 1) {
      violations.push(`${code}: maps to ${count} recovery classes (${count} entries)`);
    }
  }
  return violations;
}

async function rawTaxonomy(): Promise<{ readonly codes: readonly RawEntry[] }> {
  const url = new URL("../schemas/error-taxonomy.json", import.meta.url);
  return JSON.parse(await readFile(url, "utf8")) as { readonly codes: readonly RawEntry[] };
}

describe("one-code-one-recovery (the §4.3 schema test)", () => {
  test("every code in the source-of-truth taxonomy maps to exactly one recovery class", async () => {
    const { codes } = await rawTaxonomy();
    expect(recoveryViolations(codes)).toEqual([]);
  });

  test("the checker fails a code mapped to two recovery classes", () => {
    const doubled: RawEntry[] = [
      { code: "ENGINE_BUSY", recovery: "retry" },
      { code: "ENGINE_BUSY", recovery: "narrow" },
    ];
    expect(recoveryViolations(doubled)).not.toEqual([]);
  });

  test("the checker fails a code mapped to zero recovery classes", () => {
    expect(recoveryViolations([{ code: "ENGINE_BUSY" }])).not.toEqual([]);
    expect(recoveryViolations([{ code: "ENGINE_BUSY", recovery: "someday" }])).not.toEqual([]);
  });
});

describe("§4.3 completeness", () => {
  test("the taxonomy carries exactly the SDD's codes — none missing, none invented", async () => {
    const { codes } = await rawTaxonomy();
    expect([...codes.map((entry) => entry.code)].sort()).toEqual([...SDD_CODES].sort());
  });

  test("SDD-pinned recoveries hold", () => {
    for (const [code, recovery] of Object.entries(SDD_PINNED_RECOVERY)) {
      expect(recoveryFor(code as ErrorCode), code).toBe(recovery as never);
    }
  });

  test("every §4.3 actionable payload is declared, and its fields exist on the envelope's error schema", async () => {
    const { codes } = await rawTaxonomy();
    const errorProperties = Object.keys(envelopeSchema.properties.error.properties);
    for (const [code, fields] of Object.entries(SDD_PAYLOADS)) {
      const entry = codes.find((candidate) => candidate.code === code);
      expect(entry?.payload, code).toEqual(fields as never);
    }
    for (const entry of codes) {
      for (const field of (entry.payload as readonly string[] | undefined) ?? []) {
        expect(errorProperties, `${String(entry.code)} payload field ${field}`).toContain(field);
      }
    }
  });

  test("the generated module mirrors the source of truth", async () => {
    const { codes } = await rawTaxonomy();
    expect(errorTaxonomy.codes).toEqual(codes as never);
  });
});

describe("taxonomyError builder", () => {
  const { validate, errorsText } = envelopeValidatorFor(envelopeSchema);
  const samplePayload: Record<string, unknown> = {
    candidates: ["UserService.authenticate", "AdminService.authenticate"],
    parserMessage: "unexpected token `}`",
    rejectedDiff: "--- a/x.ts\n+++ b/x.ts",
    contentHash: "sha256:2f7e",
    confirmToken: "wb1.d1.c1.0f0f",
    backoffMs: 250,
    plugin: "runner-jest",
  };
  const build = taxonomyError as (
    init: Record<string, unknown>,
  ) => ReturnType<typeof taxonomyError>;

  test("every code builds a schema-valid envelope whose recovery matches the taxonomy", () => {
    for (const code of ERROR_CODES) {
      const entry = errorTaxonomy.codes.find((candidate) => candidate.code === code);
      const payload = Object.fromEntries(
        (entry && "payload" in entry ? entry.payload : []).map((field) => [
          field,
          samplePayload[field],
        ]),
      );
      const envelope = build({ v: 1, code, message: `${code} happened`, ...payload });
      if (!validate(envelope)) {
        throw new Error(`${code}: ${errorsText()}`);
      }
      expect(envelope.error.recovery).toBe(recoveryFor(code));
      expect(envelope.error.code).toBe(code);
    }
  });

  test("a required payload field cannot be omitted", () => {
    expect(() => build({ v: 1, code: "ENGINE_BUSY", message: "queue full" })).toThrow(RangeError);
    expect(() =>
      build({ v: 1, code: "PARSE_REJECTED", message: "broken", parserMessage: "m" }),
    ).toThrow(RangeError);
  });

  test("isErrorCode accepts taxonomy codes and rejects strangers", () => {
    expect(isErrorCode("STALE_ANCHOR")).toBe(true);
    expect(isErrorCode("TOTALLY_MADE_UP")).toBe(false);
    expect(isErrorCode(7)).toBe(false);
  });

  test("negotiation refusals now come from the taxonomy", () => {
    const refused = negotiateEnvelopeVersion({ "workbench/maxEnvelopeVersion": 0.5 });
    if (refused.ok) {
      throw new Error("expected a refusal");
    }
    expect(refused.envelope.error.code).toBe("UNSUPPORTED_VERSION");
    expect(refused.envelope.error.recovery).toBe(recoveryFor("UNSUPPORTED_VERSION"));
  });
});
