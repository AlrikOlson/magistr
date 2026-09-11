/**
 * The production IndexPort adapter (SDD §6.4): a thin bridge to the engine
 * child's indexer over frames. The server never touches index.db — every
 * read and write goes through the child, preserving the single-writer
 * invariant inside the child process.
 */

import type {
  ImportGraphQuery,
  ImportGraphResult,
  IndexedFile,
  IndexPort,
  IndexStatus,
  OutlineQuery,
  OutlineQueryResult,
  ReferenceQuery,
  ReferenceQueryResult,
  SymbolEntry,
  SymbolQuery,
  SymbolQueryResult,
  TestRange,
} from "../../ports";
import { PORT_VERSIONS } from "../../ports";
import type { EngineSupervisor } from "./supervisor";

interface ChildStatus {
  readonly tier: 0 | 1;
  readonly generation: number;
  readonly building: boolean;
  readonly filesTotal: number;
  readonly filesIndexed: number;
  readonly error?: string;
  readonly buildStartedAtMs?: number;
  readonly buildUpdatedAtMs?: number;
}

interface ChildSymbolRow {
  readonly name: string;
  readonly kind: string;
  readonly path: string;
  readonly start_line: number;
  readonly start_col: number;
  readonly end_line: number;
  readonly detail?: string | null;
  readonly doc?: string | null;
}

/** One child symbol row as the port's `SymbolEntry`. `detail`/`doc` are
 * omitted rather than set to null when the index has none — absent is absent
 * (specs/009 FR-007). */
function symbolEntryOf(row: ChildSymbolRow): SymbolEntry {
  return {
    name: row.name,
    kind: row.kind,
    location: { path: row.path, line: row.start_line, column: row.start_col },
    endLine: row.end_line,
    ...(row.detail === undefined || row.detail === null ? {} : { detail: row.detail }),
    ...(row.doc === undefined || row.doc === null ? {} : { doc: row.doc }),
  };
}

interface ChildVerifiedSymbols {
  readonly symbols: readonly ChildSymbolRow[];
  readonly freshness: SymbolQueryResult["freshness"];
}

interface ChildReferenceRow {
  readonly path: string;
  readonly line: number;
  readonly col: number;
  readonly excerpt: string;
  readonly ref_kind: "definition" | "use";
  readonly confidence: "high" | "medium" | "low";
  readonly provenance: "tier1-symbols" | "tier1-imports" | "tier1-text";
}

interface ChildOutlineFile {
  readonly path: string;
  readonly symbols: readonly ChildSymbolRow[];
}

interface ChildVerifiedOutline {
  readonly files: readonly ChildOutlineFile[];
  readonly freshness: OutlineQueryResult["freshness"];
}

interface ChildVerifiedReferences {
  readonly edges: readonly ChildReferenceRow[];
  readonly freshness: ReferenceQueryResult["freshness"];
}

interface ChildImportGraphRow {
  readonly module: string;
  readonly count: number;
}

interface ChildVerifiedImportGraph {
  readonly fanIn: readonly ChildImportGraphRow[];
  readonly fanOut: readonly ChildImportGraphRow[];
  readonly crates?: readonly {
    readonly from: string;
    readonly to: string;
    readonly files: number;
  }[];
  readonly freshness: ImportGraphResult["freshness"];
}

function toStatus(raw: ChildStatus): IndexStatus {
  return {
    tier: raw.tier,
    generation: raw.generation,
    building: raw.building,
    filesTotal: raw.filesTotal,
    filesIndexed: raw.filesIndexed,
    ...(raw.buildStartedAtMs === undefined ? {} : { buildStartedAtMs: raw.buildStartedAtMs }),
    ...(raw.buildUpdatedAtMs === undefined ? {} : { buildUpdatedAtMs: raw.buildUpdatedAtMs }),
    ...(raw.error === undefined ? {} : { lastError: raw.error }),
  };
}

export interface IpcIndexAdapterOptions {
  /** SQLite journal mode for index.db; the server's network-FS detection decides. */
  readonly journalMode?: "WAL" | "TRUNCATE";
}

export class IpcIndexAdapter implements IndexPort {
  readonly version = PORT_VERSIONS.index;

  constructor(
    private readonly supervisor: EngineSupervisor,
    private readonly root: string,
    private readonly options: IpcIndexAdapterOptions = {},
  ) {}

  async status(): Promise<IndexStatus> {
    const raw = (await this.supervisor.call("index/status", {})) as ChildStatus;
    return toStatus(raw);
  }

  async build(root: string): Promise<IndexStatus> {
    const raw = (await this.supervisor.call("index/build", {
      root,
      ...(this.options.journalMode === undefined ? {} : { journalMode: this.options.journalMode }),
    })) as ChildStatus;
    return toStatus(raw);
  }

  async querySymbols(query: SymbolQuery): Promise<SymbolQueryResult> {
    const result = (await this.supervisor.call("index/symbols", {
      name: query.name,
      ...(query.kind === undefined ? {} : { kind: query.kind }),
    })) as ChildVerifiedSymbols;
    return {
      symbols: result.symbols.map((row) => symbolEntryOf(row)),
      freshness: result.freshness,
    };
  }

  async queryOutline(query: OutlineQuery): Promise<OutlineQueryResult> {
    const result = (await this.supervisor.call("index/outline", {
      paths: [...query.paths],
    })) as ChildVerifiedOutline;
    return {
      files: result.files.map((file) => ({
        path: file.path,
        symbols: file.symbols.map((row) => symbolEntryOf(row)),
      })),
      freshness: result.freshness,
    };
  }

  async queryReferences(query: ReferenceQuery): Promise<ReferenceQueryResult> {
    const result = (await this.supervisor.call("index/references", {
      name: query.name,
    })) as ChildVerifiedReferences;
    return {
      edges: result.edges.map((row) => ({
        location: { path: row.path, line: row.line, column: row.col },
        excerpt: row.excerpt,
        refKind: row.ref_kind,
        confidence: row.confidence,
        provenance: row.provenance,
      })),
      freshness: result.freshness,
    };
  }

  async queryImportGraph(query: ImportGraphQuery): Promise<ImportGraphResult> {
    const result = (await this.supervisor.call("index/importGraph", {
      ...(query.limit === undefined ? {} : { limit: query.limit }),
      ...(query.crates === undefined
        ? {}
        : { crates: query.crates.map((c) => ({ name: c.name, dir: c.dir })) }),
    })) as ChildVerifiedImportGraph;
    return {
      fanIn: result.fanIn.map((row) => ({ module: row.module, count: row.count })),
      fanOut: result.fanOut.map((row) => ({ module: row.module, count: row.count })),
      ...(result.crates === undefined
        ? {}
        : { crates: result.crates.map((e) => ({ from: e.from, to: e.to, files: e.files })) }),
      freshness: result.freshness,
    };
  }

  async queryTestRanges(
    paths: readonly string[],
  ): Promise<Readonly<Record<string, readonly TestRange[]>>> {
    const result = (await this.supervisor.call("index/testRanges", {
      paths: [...paths],
    })) as Record<string, readonly { start_line: number; end_line: number }[]>;
    const out: Record<string, readonly TestRange[]> = {};
    for (const [path, rows] of Object.entries(result)) {
      out[path] = rows.map((row) => ({ startLine: row.start_line, endLine: row.end_line }));
    }
    return out;
  }

  async upsertFile(file: IndexedFile): Promise<void> {
    await this.supervisor.call("index/upsert", {
      path: file.path,
      symbols: file.symbols.map((s) => ({
        name: s.name,
        kind: s.kind,
        path: file.path,
        start_line: s.location.line,
        start_col: s.location.column,
        end_line: s.endLine ?? s.location.line,
      })),
    });
  }

  async refreshFile(path: string): Promise<void> {
    // index/upsert without symbols re-reads the file from disk and re-extracts.
    await this.supervisor.call("index/upsert", { path });
  }

  async removeFile(path: string): Promise<void> {
    await this.supervisor.call("index/remove", { path });
  }

  /** The workspace root this adapter builds by default. */
  get workspaceRoot(): string {
    return this.root;
  }
}
