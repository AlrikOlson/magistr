/**
 * The ResponseEnvelope runtime — the TypeScript face of
 * `schemas/envelope.schema.json` (normative v1, SDD §4.2).
 *
 * Every tool, core or plugin, returns one of these. The builders here are the
 * only sanctioned way to make one: `okEnvelope` accepts results either whole
 * or through `truncateResults`, whose signature demands the full manifest
 * (dropped/reason/narrow/cursor) before it will cut anything — so a silent
 * truncation is unrepresentable, not merely tested against.
 */

export const ENVELOPE_VERSION_MIN = 1;
export const ENVELOPE_VERSION_LATEST = 1;

export type Freshness = "cold" | "warming" | "warm";

/** The five recovery classes of SDD §4.2. The code taxonomy that maps onto
 * them one-to-one belongs to chunk:m0-error-taxonomy. */
export type Recovery = "retry" | "narrow" | "escalate" | "confirm" | "abort";

export type TruncationReason = "budget" | "ceiling" | "timeout";

export interface Budget {
  readonly requested: number;
  readonly used: number;
}

/** Present iff results were cut. All four fields are required (never silent). */
export interface Truncation {
  readonly dropped: number;
  readonly reason: TruncationReason;
  readonly narrow: readonly string[];
  readonly cursor: string;
}

export interface EnvelopeError {
  readonly code: string;
  readonly recovery: Recovery;
  readonly message: string;
  readonly candidates?: readonly string[];
  /** PARSE_REJECTED: the parser's own message for the syntax break. */
  readonly parserMessage?: string;
  /** PARSE_REJECTED: the diff that was rolled back. */
  readonly rejectedDiff?: string;
  /** STALE_ANCHOR: fresh content hash of the file that moved under the diff. */
  readonly contentHash?: string;
  /** ENGINE_BUSY: suggested backoff before retrying, in milliseconds. */
  readonly backoffMs?: number;
  /** PLUGIN_CRASHED: the plugin that trapped or exhausted fuel. */
  readonly plugin?: string;
}

export interface OkEnvelope<R = unknown> {
  readonly v: number;
  readonly ok: true;
  readonly freshness: Freshness;
  readonly budget: Budget;
  readonly results: readonly R[];
  readonly truncation?: Truncation;
  readonly notes?: readonly string[];
}

export interface ErrorEnvelope {
  readonly v: number;
  readonly ok: false;
  readonly error: EnvelopeError;
  readonly notes?: readonly string[];
}

export type ResponseEnvelope<R = unknown> = OkEnvelope<R> | ErrorEnvelope;

/** Results plus the manifest describing what was cut from them, if anything.
 * Produced by {@link truncateResults}; consumed by {@link okEnvelope}. */
export interface TruncatedResults<R> {
  readonly kept: readonly R[];
  readonly truncation?: Truncation;
}

/**
 * Cut `all` down to at most `keep` results. The manifest inputs are required
 * up front: if a cut happens, the returned truncation carries the real dropped
 * count plus the caller's reason, narrow hints and cursor. If nothing is
 * dropped, no manifest is attached (present iff cut).
 */
export function truncateResults<R>(
  all: readonly R[],
  keep: number,
  cut: {
    readonly reason: TruncationReason;
    readonly narrow: readonly string[];
    readonly cursor: string;
  },
): TruncatedResults<R> {
  if (!Number.isInteger(keep) || keep < 0) {
    throw new RangeError(`keep must be a non-negative integer, got ${keep}`);
  }
  if (keep >= all.length) {
    return { kept: all };
  }
  if (cut.cursor.length === 0) {
    throw new RangeError("a cut requires a non-empty cursor");
  }
  return {
    kept: all.slice(0, keep),
    truncation: {
      dropped: all.length - keep,
      reason: cut.reason,
      narrow: cut.narrow,
      cursor: cut.cursor,
    },
  };
}

export interface OkEnvelopeInit<R> {
  readonly v: number;
  readonly freshness: Freshness;
  readonly budget: Budget;
  /** A plain array means "complete, nothing cut". Cut result sets must come
   * through {@link truncateResults} so the manifest exists by construction. */
  readonly results: readonly R[] | TruncatedResults<R>;
  readonly notes?: readonly string[];
}

function isTruncated<R>(value: readonly R[] | TruncatedResults<R>): value is TruncatedResults<R> {
  return !Array.isArray(value);
}

export function okEnvelope<R>(init: OkEnvelopeInit<R>): OkEnvelope<R> {
  const { v, freshness, budget, results, notes } = init;
  const base = isTruncated(results)
    ? {
        results: results.kept,
        ...(results.truncation === undefined ? {} : { truncation: results.truncation }),
      }
    : { results };
  return {
    v,
    ok: true,
    freshness,
    budget,
    ...base,
    ...(notes === undefined ? {} : { notes }),
  };
}

export function errorEnvelope(init: {
  readonly v: number;
  readonly error: EnvelopeError;
  readonly notes?: readonly string[];
}): ErrorEnvelope {
  return {
    v: init.v,
    ok: false,
    error: init.error,
    ...(init.notes === undefined ? {} : { notes: init.notes }),
  };
}
