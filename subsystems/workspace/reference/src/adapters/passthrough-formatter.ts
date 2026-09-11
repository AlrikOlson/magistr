/**
 * The formatter that does nothing (SDD §7.5, `format.enabled = false`).
 *
 * The write engine always runs a format step, so "formatting off" is a
 * FormatterPort that returns the content untouched rather than a branch in
 * the engine. `changed: false` lets re-verify be skipped exactly as it is
 * for a formatter that had nothing to do.
 *
 * Parse-verification is unaffected: a write that introduces a syntax error
 * is still rejected in the same call. Only restyling is off.
 */

import type { FormatRequest, FormatResult, FormatterPort } from "../ports";
import { PORT_VERSIONS } from "../ports";

export function passthroughFormatter(): FormatterPort {
  return {
    version: PORT_VERSIONS.formatter,
    format(request: FormatRequest): Promise<FormatResult> {
      return Promise.resolve({ content: request.content, changed: false });
    },
  };
}
