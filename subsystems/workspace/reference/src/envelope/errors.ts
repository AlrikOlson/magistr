/**
 * The error-code taxonomy runtime (SDD §4.3) — the TypeScript face of
 * `schemas/error-taxonomy.json`, read through the generated module so the
 * schema stays the single source of truth.
 *
 * `taxonomyError` is the sanctioned way to build a failure envelope: the
 * recovery class comes from the taxonomy (a code paired with the wrong
 * recovery is unrepresentable), and the payload fields §4.3 requires for a
 * code are demanded by the type — and re-checked at runtime for untyped
 * callers — before an envelope exists.
 */

import { errorTaxonomy } from "../generated/schemas";
import { type EnvelopeError, type ErrorEnvelope, errorEnvelope } from "./envelope";

type TaxonomyEntry = (typeof errorTaxonomy)["codes"][number];

export type ErrorCode = TaxonomyEntry["code"];

type EntryFor<C extends ErrorCode> = Extract<TaxonomyEntry, { readonly code: C }>;

/** The recovery class the taxonomy fixes for a given code. */
export type RecoveryFor<C extends ErrorCode> = EntryFor<C>["recovery"];

/** Every code, in taxonomy order. */
export const ERROR_CODES: readonly ErrorCode[] = errorTaxonomy.codes.map((entry) => entry.code);

const CODE_SET: ReadonlySet<string> = new Set(ERROR_CODES);

function entryFor<C extends ErrorCode>(code: C): EntryFor<C> {
  const entry = errorTaxonomy.codes.find((candidate) => candidate.code === code);
  if (entry === undefined) {
    throw new RangeError(`unknown error code ${JSON.stringify(code)}`);
  }
  return entry as EntryFor<C>;
}

export function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === "string" && CODE_SET.has(value);
}

/** The one recovery class mapped to `code` (one-code-one-recovery, §4.3). */
export function recoveryFor<C extends ErrorCode>(code: C): RecoveryFor<C> {
  // Safe by construction: entryFor(code) is the taxonomy entry for `code`,
  // whose `recovery` is exactly RecoveryFor<C>; tsc cannot see through the
  // generic Extract, so it needs telling.
  return entryFor(code).recovery as RecoveryFor<C>;
}

function payloadFieldsFor(code: ErrorCode): readonly PayloadField[] {
  const entry: TaxonomyEntry = entryFor(code);
  return "payload" in entry ? entry.payload : [];
}

/** Wire types of the payload fields the taxonomy can require on an error. */
interface PayloadFieldTypes {
  readonly candidates: readonly string[];
  readonly parserMessage: string;
  readonly rejectedDiff: string;
  readonly contentHash: string;
  readonly confirmToken: string;
  readonly backoffMs: number;
  readonly plugin: string;
}

type PayloadField = keyof PayloadFieldTypes;

type PayloadKeys<C extends ErrorCode> =
  EntryFor<C> extends {
    readonly payload: readonly (infer K)[];
  }
    ? K & PayloadField
    : never;

/** The payload fields §4.3 requires for `code` — empty for most codes. */
export type PayloadOf<C extends ErrorCode> = {
  readonly [K in PayloadKeys<C>]: PayloadFieldTypes[K];
};

export type TaxonomyErrorInit<C extends ErrorCode> = {
  readonly v: number;
  readonly code: C;
  readonly message: string;
  readonly notes?: readonly string[];
} & PayloadOf<C>;

/**
 * Build a failure envelope for a taxonomy code. Recovery is filled from the
 * taxonomy, and exactly the payload fields the taxonomy declares for the code
 * are copied onto the error — a missing one throws rather than shipping an
 * error the agent cannot act on.
 */
export function taxonomyError<C extends ErrorCode>(init: TaxonomyErrorInit<C>): ErrorEnvelope {
  const { v, code, message, notes } = init;
  const payload: Record<string, unknown> = {};
  for (const field of payloadFieldsFor(code)) {
    const value = (init as Partial<Record<PayloadField, unknown>>)[field];
    if (value === undefined) {
      throw new RangeError(`${code} requires payload field "${field}" (SDD §4.3)`);
    }
    payload[field] = value;
  }
  const error = { code, recovery: recoveryFor(code), message, ...payload } as EnvelopeError;
  return errorEnvelope({ v, error, ...(notes === undefined ? {} : { notes }) });
}
