/**
 * Secret redaction at a single choke point (SDD §7.1): the envelope layer of
 * the server process, on the response path. Every output — core tools,
 * plugin tools, `script` return values, `run` output — passes through
 * envelope construction, so `sealEnvelope` is the one door between an
 * envelope and the wire and no surface can route around redaction by
 * construction.
 *
 * Redactions are *marked*, never silently dropped: a matched credential is
 * replaced with `[REDACTED:<kind>]` so the model knows something was there
 * and can say so, instead of hallucinating into an invisible hole.
 *
 * Two detectors run in order:
 *  1. Named credential patterns (AWS, GitHub, Slack, Anthropic, OpenAI,
 *     Google, PEM private keys, JWTs, `key = "…"`-style assignments).
 *  2. A high-entropy heuristic for opaque tokens the patterns do not know.
 *     It deliberately skips pure hex (git SHAs, content hashes are data, not
 *     secrets) and anything containing path separators or dots, trading a
 *     little recall for never mangling file paths, URLs, or hashes the
 *     agent needs verbatim.
 *
 * Determinism: both detectors are pure functions of the text, so redaction
 * preserves the byte-identical-envelope statelessness guarantee (§5).
 */

import type { EnvelopeError, ErrorEnvelope, OkEnvelope, ResponseEnvelope } from "./envelope";

export interface RedactionPattern {
  readonly kind: string;
  readonly pattern: RegExp;
}

/**
 * Named credential shapes, most-specific first (`sk-ant-…` must outrank the
 * generic `sk-…` OpenAI shape or it would be mislabeled).
 */
export const REDACTION_PATTERNS: readonly RedactionPattern[] = [
  { kind: "aws-key", pattern: /\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/g },
  {
    kind: "github-token",
    pattern: /\b(?:gh[pousr]_[A-Za-z0-9]{36,255}|github_pat_[A-Za-z0-9_]{22,255})\b/g,
  },
  { kind: "slack-token", pattern: /\bxox[baprs]-[A-Za-z0-9-]{10,250}\b/g },
  { kind: "anthropic-key", pattern: /\bsk-ant-[A-Za-z0-9_-]{20,}\b/g },
  { kind: "openai-key", pattern: /\bsk-[A-Za-z0-9_-]{20,}\b/g },
  { kind: "google-api-key", pattern: /\bAIza[0-9A-Za-z_-]{35}\b/g },
  {
    kind: "private-key",
    pattern: /-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z ]*PRIVATE KEY-----|$)/g,
  },
  { kind: "jwt", pattern: /\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b/g },
];

/**
 * `password = "…"` / `api_key: …` assignments. Only the value is redacted;
 * the key name survives so the agent can still see *what* was configured.
 */
const ASSIGNMENT_PATTERN =
  /\b(api[_-]?key|access[_-]?key|secret(?:[_-]?key)?|token|password|passwd)(\s*[:=]\s*["']?)([^\s"']{8,})(["']?)/gi;

/** Candidate opaque tokens for the entropy heuristic: no `/ \ .`, no hex. */
const ENTROPY_CANDIDATE = /[A-Za-z0-9+=_-]{40,}/g;
const ENTROPY_THRESHOLD_BITS = 4.5;
const HEX_ONLY = /^[0-9a-fA-F]+$/;

/** Shannon entropy of `text` in bits per character. */
export function shannonEntropy(text: string): number {
  const counts = new Map<string, number>();
  for (const char of text) {
    counts.set(char, (counts.get(char) ?? 0) + 1);
  }
  let bits = 0;
  for (const count of counts.values()) {
    const p = count / text.length;
    bits -= p * Math.log2(p);
  }
  return bits;
}

function marker(kind: string): string {
  return `[REDACTED:${kind}]`;
}

/** A marker must never itself look like a candidate to a later detector. */
const MARKER_SHAPE = /\[REDACTED:[a-z-]+\]/;

export interface RedactedText {
  readonly text: string;
  /** Every redaction kind applied, in application order (with repeats). */
  readonly kinds: readonly string[];
}

/** Redact one string. Pure; returns the input untouched when nothing matches. */
export function redactText(text: string): RedactedText {
  const kinds: string[] = [];
  let out = text;
  for (const { kind, pattern } of REDACTION_PATTERNS) {
    out = out.replace(pattern, () => {
      kinds.push(kind);
      return marker(kind);
    });
  }
  out = out.replace(
    ASSIGNMENT_PATTERN,
    (whole, name: string, sep: string, _value: string, quote: string) => {
      if (MARKER_SHAPE.test(whole)) return whole;
      kinds.push("credential-assignment");
      return `${name}${sep}${marker("credential-assignment")}${quote}`;
    },
  );
  out = out.replace(ENTROPY_CANDIDATE, (candidate) => {
    if (HEX_ONLY.test(candidate)) return candidate;
    if (shannonEntropy(candidate) < ENTROPY_THRESHOLD_BITS) return candidate;
    kinds.push("high-entropy");
    return marker("high-entropy");
  });
  return kinds.length === 0 ? { text, kinds } : { text: out, kinds };
}

function redactValue(value: unknown): unknown {
  if (typeof value === "string") return redactText(value).text;
  if (Array.isArray(value)) return value.map(redactValue);
  if (value !== null && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [key, entry] of Object.entries(value)) {
      out[key] = redactValue(entry);
    }
    return out;
  }
  return value;
}

function redactOk<R>(envelope: OkEnvelope<R>): OkEnvelope<R> {
  return {
    ...envelope,
    results: envelope.results.map((result) => redactValue(result)) as readonly R[],
    ...(envelope.notes === undefined
      ? {}
      : { notes: envelope.notes.map((note) => redactText(note).text) }),
  };
}

function redactError(envelope: ErrorEnvelope): ErrorEnvelope {
  return {
    ...envelope,
    error: redactValue(envelope.error) as EnvelopeError,
    ...(envelope.notes === undefined
      ? {}
      : { notes: envelope.notes.map((note) => redactText(note).text) }),
  };
}

/** Redact every string in the envelope — results, notes, and error payload. */
export function redactEnvelope<R>(envelope: ResponseEnvelope<R>): ResponseEnvelope<R> {
  return envelope.ok ? redactOk(envelope) : redactError(envelope);
}

export interface SealedEnvelope<R> {
  /** The redacted envelope, for `structuredContent`. */
  readonly envelope: ResponseEnvelope<R>;
  /** Its serialization, for the wire — the same bytes, by construction. */
  readonly json: string;
}

/**
 * The single door to the wire: redact, then serialize. The protocol layer
 * sends only what this returns, so text and structured content cannot
 * diverge and neither can skip redaction.
 */
export function sealEnvelope<R>(envelope: ResponseEnvelope<R>): SealedEnvelope<R> {
  const redacted = redactEnvelope(envelope);
  return { envelope: redacted, json: JSON.stringify(redacted) };
}
