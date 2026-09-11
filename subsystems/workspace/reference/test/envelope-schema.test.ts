/**
 * Property tests for the normative v1 envelope: everything the builders can
 * produce validates against the generated schema, the success/failure branches
 * are exclusive, and truncation is never silent (chunk:m0-envelope-contract).
 */
import { describe, expect, test } from "bun:test";
import fc from "fast-check";
import {
  errorEnvelope,
  type Freshness,
  okEnvelope,
  type Recovery,
  type TruncationReason,
  truncateResults,
} from "../src/envelope";
import { envelopeSchema } from "../src/generated/schemas";
import { envelopeValidatorFor } from "./support/ajv";

const { validate, errorsText } = envelopeValidatorFor(envelopeSchema);

function assertValid(envelope: unknown): void {
  if (!validate(envelope)) {
    throw new Error(`envelope failed schema validation: ${errorsText()}`);
  }
}

const arbFreshness = fc.constantFrom<Freshness>("cold", "warming", "warm");
const arbBudget = fc.record({ requested: fc.nat(), used: fc.nat() });
const arbResults = fc.array(fc.jsonValue(), { maxLength: 8 });
const arbCut = fc.record({
  reason: fc.constantFrom<TruncationReason>("budget", "ceiling", "timeout"),
  narrow: fc.array(fc.string(), { maxLength: 4 }),
  cursor: fc.string({ minLength: 1 }),
});
const arbError = fc.record(
  {
    code: fc.stringMatching(/^[A-Z][A-Z0-9_]{0,30}$/),
    recovery: fc.constantFrom<Recovery>("retry", "narrow", "escalate", "confirm", "abort"),
    message: fc.string(),
    candidates: fc.array(fc.string(), { maxLength: 4 }),
  },
  { requiredKeys: ["code", "recovery", "message"] },
);

describe("success envelopes", () => {
  test("every built success envelope validates, and truncation is present iff results were cut", () => {
    fc.assert(
      fc.property(
        arbResults,
        fc.nat(12),
        arbCut,
        arbFreshness,
        arbBudget,
        (all, keep, cut, freshness, budget) => {
          const truncated = truncateResults(all, keep, cut);
          const envelope = okEnvelope({ v: 1, freshness, budget, results: truncated });
          assertValid(envelope);

          const wasCut = keep < all.length;
          expect(envelope.truncation !== undefined).toBe(wasCut);
          expect(envelope.results.length).toBe(Math.min(keep, all.length));
          if (envelope.truncation !== undefined) {
            // Never silent: the manifest always carries all four fields.
            expect(envelope.truncation.dropped).toBe(all.length - keep);
            expect(envelope.truncation.reason).toBe(cut.reason);
            expect(envelope.truncation.narrow).toEqual(cut.narrow);
            expect(envelope.truncation.cursor).toBe(cut.cursor);
          }
        },
      ),
    );
  });

  test("complete result sets validate and never carry a manifest", () => {
    fc.assert(
      fc.property(arbResults, arbFreshness, arbBudget, (results, freshness, budget) => {
        const envelope = okEnvelope({ v: 1, freshness, budget, results, notes: ["n"] });
        assertValid(envelope);
        expect(envelope.truncation).toBeUndefined();
      }),
    );
  });

  test("truncateResults refuses a cut without a cursor", () => {
    expect(() =>
      truncateResults([1, 2, 3], 1, { reason: "budget", narrow: [], cursor: "" }),
    ).toThrow(RangeError);
  });
});

describe("failure envelopes", () => {
  test("every built error envelope validates", () => {
    fc.assert(
      fc.property(arbError, (error) => {
        assertValid(errorEnvelope({ v: 1, error }));
      }),
    );
  });
});

describe("branch exclusivity (schema-level)", () => {
  const okBase = {
    v: 1,
    ok: true,
    freshness: "warm",
    budget: { requested: 4000, used: 10 },
    results: [],
  };
  const err = { code: "UNSUPPORTED_VERSION", recovery: "abort", message: "x" };

  test("a success envelope carrying an error is invalid", () => {
    expect(validate({ ...okBase, error: err })).toBe(false);
  });

  test("a failure envelope carrying results or truncation is invalid", () => {
    expect(validate({ v: 1, ok: false, error: err, results: [] })).toBe(false);
    expect(
      validate({
        v: 1,
        ok: false,
        error: err,
        truncation: { dropped: 1, reason: "budget", narrow: [], cursor: "c" },
      }),
    ).toBe(false);
  });

  test("a failure envelope without an error is invalid", () => {
    expect(validate({ v: 1, ok: false })).toBe(false);
  });

  test("a partial truncation manifest is invalid — all four fields are required", () => {
    for (const missing of ["dropped", "reason", "narrow", "cursor"]) {
      const manifest: Record<string, unknown> = {
        dropped: 3,
        reason: "budget",
        narrow: ["kind:function"],
        cursor: "abc",
      };
      delete manifest[missing];
      expect(validate({ ...okBase, truncation: manifest }), `missing ${missing}`).toBe(false);
    }
  });

  test("a success envelope missing freshness, budget or results is invalid", () => {
    for (const missing of ["freshness", "budget", "results"]) {
      const candidate: Record<string, unknown> = { ...okBase };
      delete candidate[missing];
      expect(validate(candidate), `missing ${missing}`).toBe(false);
    }
  });
});
