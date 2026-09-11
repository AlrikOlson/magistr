import type { FormatRequest, FormatResult, FormatterPort } from "../../ports";
import { PORT_VERSIONS } from "../../ports";

export {
  editionFor,
  hasToolchainPin,
  RUST_EDITION_DEFAULT,
  RUST_EDITIONS,
  RUSTFMT_TIMEOUT_MS,
  type RustEdition,
  RustfmtFormatter,
  type RustfmtFormatterOptions,
  type RustfmtResolver,
  resolveRustfmt,
} from "./formatter";

export interface FormatterRule {
  /** Lower-case extensions without the dot. */
  readonly extensions: readonly string[];
  readonly formatter: FormatterPort;
}

/**
 * One FormatterPort that routes by extension (specs/012 R1): the first rule
 * whose extension matches formats the file; everything else goes to the
 * fallback (the Biome adapter, which already passes through what it does
 * not cover). The write engine keeps seeing exactly one formatter.
 */
export function routingFormatter(options: {
  readonly rules: readonly FormatterRule[];
  readonly fallback: FormatterPort;
}): FormatterPort {
  const byExt = new Map<string, FormatterPort>();
  for (const rule of options.rules) {
    for (const ext of rule.extensions) byExt.set(ext.toLowerCase(), rule.formatter);
  }
  return {
    version: PORT_VERSIONS.formatter,
    format(request: FormatRequest): Promise<FormatResult> {
      const ext = request.path.slice(request.path.lastIndexOf(".") + 1).toLowerCase();
      return (byExt.get(ext) ?? options.fallback).format(request);
    },
  };
}
