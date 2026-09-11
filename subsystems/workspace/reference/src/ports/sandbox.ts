/**
 * SandboxPort — the security jail's seam (SDD §6.4, §7.1).
 *
 * Path canonicalization and access verdicts. Nothing reaches the filesystem
 * except through this port; a denial is a verdict with a reason, never an
 * exception, so tool code cannot forget to handle it.
 */

import type { PORT_VERSIONS } from "./versions";

export type AccessIntent = "read" | "write";

export interface AccessRequest {
  readonly path: string;
  readonly intent: AccessIntent;
}

export interface AccessVerdict {
  readonly allowed: boolean;
  /** Present iff `allowed` is false. */
  readonly reason?: string;
}

export interface SandboxPort {
  readonly version: typeof PORT_VERSIONS.sandbox;
  /** Resolve to a canonical absolute path inside the workspace, or reject. */
  canonicalize(path: string): Promise<string>;
  check(request: AccessRequest): Promise<AccessVerdict>;
}
