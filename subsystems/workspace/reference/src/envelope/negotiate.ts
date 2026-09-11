/**
 * Per-request envelope-version negotiation (SDD §4.2 + §5.1).
 *
 * Clients may declare `workbench/maxEnvelopeVersion` in `_meta` on any
 * request. The server answers in the highest mutually supported version and
 * mirrors it in the envelope's `v`. A client that declares nothing gets the
 * latest. Negotiation is a pure function of the request's `_meta` — no module
 * state, no session state — which is what makes it stateless per §5.1.
 */

import { ENVELOPE_VERSION_LATEST, ENVELOPE_VERSION_MIN, type ErrorEnvelope } from "./envelope";
import { type ErrorCode, taxonomyError } from "./errors";

export const META_MAX_ENVELOPE_VERSION = "workbench/maxEnvelopeVersion";

/** The taxonomy code negotiation emits; SDD §4.3 fixes its recovery to abort. */
export const UNSUPPORTED_VERSION: ErrorCode = "UNSUPPORTED_VERSION";

export type Negotiation =
  | { readonly ok: true; readonly v: number }
  | { readonly ok: false; readonly envelope: ErrorEnvelope };

function refusal(v: number, message: string): Negotiation {
  return {
    ok: false,
    envelope: taxonomyError({ v, code: "UNSUPPORTED_VERSION", message }),
  };
}

/**
 * Resolve the envelope version for one request from its `_meta`.
 *
 * - No `_meta`, or no declaration: the latest version.
 * - An integer declaration >= the server minimum: min(declared, latest).
 * - An integer below the server minimum: `UNSUPPORTED_VERSION` (abort).
 * - A malformed declaration: `UNSUPPORTED_VERSION` (abort). A client that
 *   tried to pin and garbled it must not be silently upgraded to latest —
 *   that would defeat the pin.
 */
export function negotiateEnvelopeVersion(meta?: Readonly<Record<string, unknown>>): Negotiation {
  const declared = meta?.[META_MAX_ENVELOPE_VERSION];
  if (declared === undefined) {
    return { ok: true, v: ENVELOPE_VERSION_LATEST };
  }
  if (typeof declared !== "number" || !Number.isInteger(declared) || declared < 1) {
    return refusal(
      ENVELOPE_VERSION_LATEST,
      `${META_MAX_ENVELOPE_VERSION} must be a positive integer, got ${JSON.stringify(declared)}`,
    );
  }
  if (declared < ENVELOPE_VERSION_MIN) {
    return refusal(
      ENVELOPE_VERSION_MIN,
      `client maximum envelope version ${declared} is below the server minimum ${ENVELOPE_VERSION_MIN}`,
    );
  }
  return { ok: true, v: Math.min(declared, ENVELOPE_VERSION_LATEST) };
}
