/**
 * EnginePort — parse / search / rewrite (SDD §6.4).
 *
 * Production adapter is the engine-child IPC bridge; the WASM fallback is
 * selected inside the child; tests use the in-process adapter. The tool layer
 * sees only this interface.
 */

import type { SourceLocation } from "./types";
import type { PORT_VERSIONS } from "./versions";

export type SearchMode = "lexical" | "structural" | "references";

export interface ParseRequest {
  readonly path: string;
  readonly content: string;
}

/** Parse verdict for the write engine's parse-verify step (§7.5). */
export interface ParseVerdict {
  readonly ok: boolean;
  /** Present iff `ok` is false: the parser's own message for the syntax break. */
  readonly parserMessage?: string;
}

export interface EngineSearchRequest {
  readonly mode: SearchMode;
  readonly pattern: string;
  /** Workspace-relative paths to search. Empty means the whole workspace. */
  readonly paths: readonly string[];
  /**
   * §7.7 cancellation propagation: adapters check this between files and
   * forward it into the IPC layer so a cancelled request stops burning the
   * engine. Never serialized onto a wire.
   */
  readonly signal?: AbortSignal;
}

export interface EngineMatch {
  readonly location: SourceLocation;
  readonly excerpt: string;
  /** Structural mode: the tree-sitter node kind of the matched node. */
  readonly kind?: string;
  /** Structural mode: 1-based last line of a multi-line match. */
  readonly endLine?: number;
}

export interface RewriteEdit {
  readonly find: string;
  readonly replace: string;
}

export interface RewriteRequest {
  readonly path: string;
  readonly content: string;
  readonly edits: readonly RewriteEdit[];
}

export interface RewriteResult {
  readonly content: string;
  readonly changed: boolean;
}

export interface EnginePort {
  readonly version: typeof PORT_VERSIONS.engine;
  parse(request: ParseRequest): Promise<ParseVerdict>;
  search(request: EngineSearchRequest): Promise<readonly EngineMatch[]>;
  rewrite(request: RewriteRequest): Promise<RewriteResult>;
}
