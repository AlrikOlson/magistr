/**
 * The flight-recorder vocabulary (SDD §11.2, feature 005-tracing-ring-buffer).
 *
 * Privacy is structural, not reviewed (research R4): the tracer API accepts
 * only the typed attributes below — there is no free-form attach, so tool
 * arguments, query text, file contents and env values are unrepresentable in
 * a span record. Status codes are the published taxonomy's own type; a code
 * paired with the wrong recovery cannot exist because the writer derives the
 * recovery class from the code (one-code-one-recovery, SDD §4.3).
 *
 * Lives in ports so adapters may consume it (the engine supervisor takes an
 * optional Tracer) without reaching into protocol.
 */

import type { ErrorCode } from "../envelope";

/** Span outcome: ok, or a taxonomy code (recovery derived by the writer). */
export type SpanStatus = { readonly ok: true } | { readonly ok: false; readonly code: ErrorCode };

/** Root-span attributes known only once the envelope exists. */
export interface RootEndAttributes {
  readonly envelopeV?: number;
  readonly notesCount?: number;
  readonly truncated?: boolean;
}

/** One engine IPC call, recorded as `engine:<method>` under its request. */
export interface EngineSpan {
  end(status: SpanStatus): void;
}

/** One served tool request, recorded as `tool:<name>`. */
export interface ToolSpan {
  /** A linked child for one engine call made while serving this request. */
  engineChild(attributes: { readonly method: string }): EngineSpan;
  end(status: SpanStatus, result?: RootEndAttributes): void;
}

/**
 * The tracer seam. A span that is started but never ended emits nothing —
 * that is the sanctioned way to drop work with no taxonomy outcome (e.g. a
 * caller-cancelled request).
 */
export interface Tracer {
  startToolSpan(attributes: { readonly tool: string }): ToolSpan;
  /** The request span the current async execution is serving, if any. */
  activeSpan(): ToolSpan | null;
  /** Establish `span` as the active parent for everything `fn` awaits. */
  runWith<T>(span: ToolSpan, fn: () => T): T;
}

const NOOP_ENGINE_SPAN: EngineSpan = { end: () => {} };

const NOOP_TOOL_SPAN: ToolSpan = {
  engineChild: () => NOOP_ENGINE_SPAN,
  end: () => {},
};

/** Wired when `tracing.enabled` is false, and the default everywhere a
 * constructor receives no tracer: nothing behaves differently untraced. */
export const noopTracer: Tracer = {
  startToolSpan: () => NOOP_TOOL_SPAN,
  activeSpan: () => null,
  runWith: (_span, fn) => fn(),
};
