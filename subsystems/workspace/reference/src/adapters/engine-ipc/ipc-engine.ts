/**
 * The production EnginePort adapter (SDD §6.4): the engine-child IPC bridge
 * `src/ports/engine.ts` names. The server side never parses untrusted bytes
 * natively — parse and rewrite travel to the child over frames; file reading
 * for search stays here (reading is not parsing) and the per-file haystack
 * travels on the request, keeping the protocol shared-nothing.
 */

import type {
  EngineMatch,
  EnginePort,
  EngineSearchRequest,
  ParseRequest,
  ParseVerdict,
  RewriteRequest,
  RewriteResult,
} from "../../ports";
import { PORT_VERSIONS } from "../../ports";
import { languageForPath } from "./indexer";
import type { EngineSupervisor } from "./supervisor";

/** How the bridge reads workspace files for search. Injected by the server. */
export interface EngineFileAccess {
  read(path: string): Promise<string>;
  list(): Promise<readonly string[]>;
}

interface ChildSearchHit {
  readonly line: number;
  readonly start: number;
  readonly end: number;
  readonly lineText: string;
}

interface ChildStructuralHit {
  readonly line: number;
  readonly column: number;
  readonly endLine: number;
  readonly kind: string;
  readonly lineText: string;
}

export class IpcEngineAdapter implements EnginePort {
  readonly version = PORT_VERSIONS.engine;

  constructor(
    private readonly supervisor: EngineSupervisor,
    private readonly files: EngineFileAccess,
  ) {}

  async parse(request: ParseRequest): Promise<ParseVerdict> {
    const result = await this.supervisor.call("parse", {
      path: request.path,
      source: request.content,
    });
    return result as ParseVerdict;
  }

  async search(request: EngineSearchRequest): Promise<readonly EngineMatch[]> {
    if (request.mode === "structural") return this.structuralSearch(request);
    if (request.mode !== "lexical") return [];
    const paths = request.paths.length > 0 ? request.paths : await this.files.list();
    const matches: EngineMatch[] = [];
    for (const path of paths) {
      // §7.7: a cancelled request stops burning cycles between files.
      request.signal?.throwIfAborted();
      const haystack = await this.files.read(path);
      const result = (await this.supervisor.call(
        "search",
        {
          pattern: request.pattern,
          haystack,
          path,
        },
        { ...(request.signal === undefined ? {} : { signal: request.signal }) },
      )) as { readonly hits: readonly ChildSearchHit[] };
      for (const hit of result.hits) {
        matches.push({
          location: { path, line: hit.line, column: hit.start + 1 },
          excerpt: hit.lineText,
        });
      }
    }
    return matches;
  }

  private async structuralSearch(request: EngineSearchRequest): Promise<readonly EngineMatch[]> {
    const paths = request.paths.length > 0 ? request.paths : await this.files.list();
    const matches: EngineMatch[] = [];
    for (const path of paths) {
      // Only bundled source languages can match structurally; skipping the
      // rest saves the file read and the frame round-trip.
      if (languageForPath(path) === undefined) continue;
      request.signal?.throwIfAborted();
      const source = await this.files.read(path);
      const result = (await this.supervisor.call(
        "structural",
        {
          pattern: request.pattern,
          source,
          path,
        },
        { ...(request.signal === undefined ? {} : { signal: request.signal }) },
      )) as { readonly hits: readonly ChildStructuralHit[] };
      for (const hit of result.hits) {
        matches.push({
          location: { path, line: hit.line, column: hit.column },
          excerpt: hit.lineText,
          kind: hit.kind,
          endLine: hit.endLine,
        });
      }
    }
    return matches;
  }

  async rewrite(request: RewriteRequest): Promise<RewriteResult> {
    const result = await this.supervisor.call("rewrite", {
      path: request.path,
      source: request.content,
      edits: request.edits,
    });
    return result as RewriteResult;
  }
}
