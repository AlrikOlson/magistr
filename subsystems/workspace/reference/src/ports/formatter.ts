/**
 * FormatterPort — the write engine's format step (SDD §6.4, §7.5).
 *
 * Formatting is idempotent from the caller's point of view: `changed` says
 * whether the content moved, so re-verify can be skipped when it did not.
 */

import type { PORT_VERSIONS } from "./versions";

export interface FormatRequest {
  readonly path: string;
  readonly content: string;
}

export interface FormatResult {
  readonly content: string;
  readonly changed: boolean;
}

export interface FormatterPort {
  readonly version: typeof PORT_VERSIONS.formatter;
  format(request: FormatRequest): Promise<FormatResult>;
}
