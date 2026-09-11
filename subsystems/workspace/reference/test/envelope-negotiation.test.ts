/**
 * Property tests for per-request _meta version negotiation (SDD §4.2 + §5.1):
 * the resolved version is a pure function of the request's declaration, so a
 * round-robin of interleaved requests can never observe session state.
 */
import { describe, expect, test } from "bun:test";
import fc from "fast-check";
import {
  ENVELOPE_VERSION_LATEST,
  ENVELOPE_VERSION_MIN,
  META_MAX_ENVELOPE_VERSION,
  negotiateEnvelopeVersion,
  UNSUPPORTED_VERSION,
} from "../src/envelope";
import { envelopeSchema } from "../src/generated/schemas";
import { envelopeValidatorFor } from "./support/ajv";

const { validate } = envelopeValidatorFor(envelopeSchema);

const arbDeclaredInt = fc.integer({ min: 1, max: 1_000 });
const arbMalformed = fc.oneof(
  fc.string(),
  fc.boolean(),
  fc.constant(null),
  fc.integer({ min: -1_000, max: 0 }),
  fc.double({ noInteger: true, noNaN: true, noDefaultInfinity: true }),
);
const arbMeta = fc.oneof(
  fc.constant(undefined),
  fc.constant({}),
  arbDeclaredInt.map((n) => ({ [META_MAX_ENVELOPE_VERSION]: n })),
  arbMalformed.map((x) => ({ [META_MAX_ENVELOPE_VERSION]: x })),
);

describe("resolution", () => {
  test("no declaration resolves to the latest version", () => {
    expect(negotiateEnvelopeVersion()).toEqual({ ok: true, v: ENVELOPE_VERSION_LATEST });
    expect(negotiateEnvelopeVersion({})).toEqual({ ok: true, v: ENVELOPE_VERSION_LATEST });
  });

  test("a positive integer declaration resolves to the highest mutually supported version", () => {
    fc.assert(
      fc.property(arbDeclaredInt, (declared) => {
        const result = negotiateEnvelopeVersion({ [META_MAX_ENVELOPE_VERSION]: declared });
        expect(result.ok).toBe(true);
        if (result.ok) {
          expect(result.v).toBe(Math.min(declared, ENVELOPE_VERSION_LATEST));
          expect(result.v).toBeLessThanOrEqual(declared);
          expect(result.v).toBeGreaterThanOrEqual(ENVELOPE_VERSION_MIN);
        }
      }),
    );
  });

  test("a malformed declaration is refused with UNSUPPORTED_VERSION/abort, and the refusal validates", () => {
    fc.assert(
      fc.property(arbMalformed, (declared) => {
        const result = negotiateEnvelopeVersion({ [META_MAX_ENVELOPE_VERSION]: declared });
        expect(result.ok).toBe(false);
        if (!result.ok) {
          expect(result.envelope.error.code).toBe(UNSUPPORTED_VERSION);
          expect(result.envelope.error.recovery).toBe("abort");
          expect(validate(result.envelope)).toBe(true);
        }
      }),
    );
  });
});

describe("statelessness", () => {
  test("the resolution of a request depends only on that request, under any interleaving", () => {
    fc.assert(
      fc.property(fc.array(arbMeta, { minLength: 1, maxLength: 25 }), (metas) => {
        // Reference: each request negotiated in isolation.
        const isolated = metas.map((meta) => negotiateEnvelopeVersion(meta));
        // Round-robin the same requests in reverse and interleaved order —
        // per §5.1 no request may observe anything left behind by another.
        const reversed = [...metas]
          .reverse()
          .map((meta) => negotiateEnvelopeVersion(meta))
          .reverse();
        expect(reversed).toEqual(isolated);
        const repeated = metas.map((meta) => {
          negotiateEnvelopeVersion({ [META_MAX_ENVELOPE_VERSION]: 1 });
          return negotiateEnvelopeVersion(meta);
        });
        expect(repeated).toEqual(isolated);
      }),
    );
  });
});
