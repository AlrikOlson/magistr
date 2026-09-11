/**
 * ResolverPort — symbol resolution: definitions and references (SDD §6.4).
 *
 * v1 kept resolution symbolic (by name) at bare locations. v2 (M5,
 * chunk:m5-lsp-resolver-plugin) makes every resolved edge carry confidence
 * provenance so the envelope never overclaims: a symbolic implementor labels
 * `tier1-*`, and only the resolver plugin's language-server-verified answers
 * may carry `resolver-lsp`. Position-based resolution remains a later
 * additive bump, arriving when the public WIT surface itself grows.
 */

import type { ReferenceConfidence, ReferenceProvenance } from "./index-port";
import type { SourceLocation } from "./types";
import type { PORT_VERSIONS } from "./versions";

export type ResolveKind = "definition" | "references";

export interface ResolveRequest {
  readonly kind: ResolveKind;
  readonly symbol: string;
}

/** One resolved edge: a location plus how it was derived (never bare). */
export interface ResolvedEdge {
  readonly location: SourceLocation;
  readonly provenance: ReferenceProvenance;
  readonly confidence: ReferenceConfidence;
}

export interface ResolverPort {
  readonly version: typeof PORT_VERSIONS.resolver;
  resolve(request: ResolveRequest): Promise<readonly ResolvedEdge[]>;
}
