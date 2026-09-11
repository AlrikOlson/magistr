/**
 * The in-process test adapter (SDD §6.4) — every port implemented over one
 * shared in-memory workspace, no child process, no filesystem. This is what
 * keeps the statelessness and freshness suites fast; it is an adapter like
 * any other, selected by composition, never imported by the tool layer.
 */

import type { FileContent } from "../../envelope";
import type {
  AccessRequest,
  AccessVerdict,
  EngineMatch,
  EnginePort,
  EngineSearchRequest,
  FormatRequest,
  FormatResult,
  FormatterPort,
  ImportGraphQuery,
  ImportGraphResult,
  IndexedFile,
  IndexPort,
  IndexStatus,
  JournalAppendResult,
  JournalLogEntry,
  JournalLogQuery,
  JournalOp,
  JournalOpInit,
  JournalPort,
  OutlineQuery,
  OutlineQueryResult,
  ParseRequest,
  ParseVerdict,
  PortSet,
  ReferenceEdge,
  ReferenceQuery,
  ReferenceQueryResult,
  ResolvedEdge,
  ResolveRequest,
  ResolverPort,
  RewriteRequest,
  RewriteResult,
  SandboxPort,
  SymbolEntry,
  SymbolQuery,
  SymbolQueryResult,
  UndoCandidateQuery,
  WatchEvent,
  WatcherPort,
  WatchSubscription,
} from "../../ports";
import { PORT_VERSIONS } from "../../ports";

/** The shared in-memory workspace all seven in-process ports operate on. */
export class InProcessWorkspace {
  readonly root: string;
  readonly files = new Map<string, string>();

  constructor(root = "/workspace") {
    this.root = root;
  }
}

const OPENERS = new Map([
  ["(", ")"],
  ["[", "]"],
  ["{", "}"],
]);
const CLOSERS = new Set(OPENERS.values());

class InProcessEngine implements EnginePort {
  readonly version = PORT_VERSIONS.engine;

  constructor(private readonly workspace: InProcessWorkspace) {}

  parse(request: ParseRequest): Promise<ParseVerdict> {
    const stack: string[] = [];
    for (const ch of request.content) {
      const closer = OPENERS.get(ch);
      if (closer !== undefined) {
        stack.push(closer);
      } else if (CLOSERS.has(ch) && stack.pop() !== ch) {
        return Promise.resolve({ ok: false, parserMessage: `unbalanced '${ch}'` });
      }
    }
    const dangling = stack.pop();
    return Promise.resolve(
      dangling === undefined
        ? { ok: true }
        : { ok: false, parserMessage: `expected '${dangling}' before end of input` },
    );
  }

  search(request: EngineSearchRequest): Promise<readonly EngineMatch[]> {
    if (request.mode !== "lexical") return Promise.resolve([]);
    const matches: EngineMatch[] = [];
    for (const [path, content] of this.workspace.files) {
      if (request.paths.length > 0 && !request.paths.includes(path)) continue;
      content.split("\n").forEach((text, i) => {
        const column = text.indexOf(request.pattern);
        if (column >= 0) {
          matches.push({ location: { path, line: i + 1, column: column + 1 }, excerpt: text });
        }
      });
    }
    return Promise.resolve(matches);
  }

  rewrite(request: RewriteRequest): Promise<RewriteResult> {
    let content = request.content;
    for (const edit of request.edits) {
      content = content.replaceAll(edit.find, edit.replace);
    }
    return Promise.resolve({ content, changed: content !== request.content });
  }
}

class InProcessIndex implements IndexPort {
  readonly version = PORT_VERSIONS.index;
  private readonly symbolsByPath = new Map<string, readonly SymbolEntry[]>();
  private generation = 0;
  private buildStampMs: number | undefined;

  constructor(private readonly workspace: InProcessWorkspace) {}

  status(): Promise<IndexStatus> {
    const total = this.workspace.files.size;
    return Promise.resolve({
      tier: 1 as const,
      generation: this.generation,
      building: false,
      filesTotal: total,
      filesIndexed: total,
      ...(this.buildStampMs === undefined
        ? {}
        : { buildStartedAtMs: this.buildStampMs, buildUpdatedAtMs: this.buildStampMs }),
    });
  }

  /** In memory there is nothing to warm: a build is immediately complete. */
  build(root: string): Promise<IndexStatus> {
    void root;
    this.generation += 1;
    this.buildStampMs = Date.now();
    return this.status();
  }

  querySymbols(query: SymbolQuery): Promise<SymbolQueryResult> {
    const name = query.name.toLowerCase();
    const hits: SymbolEntry[] = [];
    // In memory nothing can go stale behind the index's back, so there is no
    // mtime+size analogue to verify: upserted rows are authoritative.
    for (const symbols of this.symbolsByPath.values()) {
      for (const symbol of symbols) {
        if (!symbol.name.toLowerCase().includes(name)) continue;
        if (query.kind !== undefined && symbol.kind !== query.kind) continue;
        hits.push(symbol);
      }
    }
    return Promise.resolve({ symbols: hits, freshness: "warm" });
  }

  queryOutline(query: OutlineQuery): Promise<OutlineQueryResult> {
    const files = query.paths.map((path) => ({
      path,
      symbols: this.symbolsByPath.get(path) ?? [],
    }));
    return Promise.resolve({ files, freshness: "warm" as const });
  }

  queryReferences(query: ReferenceQuery): Promise<ReferenceQueryResult> {
    const edges: ReferenceEdge[] = [];
    const defLines = new Map<string, Set<number>>();
    for (const symbols of this.symbolsByPath.values()) {
      for (const symbol of symbols) {
        if (symbol.name.toLowerCase() !== query.name.toLowerCase()) continue;
        edges.push({
          location: symbol.location,
          excerpt: "",
          refKind: "definition",
          confidence: "high",
          provenance: "tier1-symbols",
        });
        const lines = defLines.get(symbol.location.path) ?? new Set<number>();
        lines.add(symbol.location.line);
        defLines.set(symbol.location.path, lines);
      }
    }
    for (const [path, content] of this.workspace.files) {
      content.split("\n").forEach((text, i) => {
        if (defLines.get(path)?.has(i + 1) === true) return;
        const column = text.indexOf(query.name);
        if (column < 0) return;
        edges.push({
          location: { path, line: i + 1, column: column + 1 },
          excerpt: text,
          refKind: "use",
          confidence: "low",
          provenance: "tier1-text",
        });
      });
    }
    return Promise.resolve({ edges, freshness: "warm" });
  }

  /** upsertFile carries symbols only — this adapter records no import edges,
   * so the honest summary is empty, not fabricated. */
  queryImportGraph(query: ImportGraphQuery): Promise<ImportGraphResult> {
    void query;
    return Promise.resolve({ fanIn: [], fanOut: [], freshness: "warm" });
  }

  upsertFile(file: IndexedFile): Promise<void> {
    this.symbolsByPath.set(file.path, file.symbols);
    this.generation += 1;
    return Promise.resolve();
  }

  refreshFile(path: string): Promise<void> {
    if (!this.workspace.files.has(path)) this.symbolsByPath.delete(path);
    this.generation += 1;
    return Promise.resolve();
  }

  removeFile(path: string): Promise<void> {
    if (this.symbolsByPath.delete(path)) this.generation += 1;
    return Promise.resolve();
  }
}

class InProcessJournal implements JournalPort {
  readonly version = PORT_VERSIONS.journal;
  private readonly ops: JournalOp[] = [];
  private readonly undone = new Set<number>();
  /** Which undo-op consumed each undone op — what the log shows. */
  private readonly undoneBy = new Map<number, number>();

  append(op: JournalOpInit): Promise<JournalAppendResult> {
    const entry: JournalOp = {
      seq: this.ops.length + 1,
      at: Date.now(),
      tool: op.tool,
      argsHash: op.argsHash,
      sessionId: op.sessionId,
      ...(op.undoOf === undefined ? {} : { undoOf: [...op.undoOf] }),
      files: op.files.map((f) => ({
        kind: f.kind,
        path: f.path,
        beforeHash: sha256Hex(f.before),
        afterHash: sha256Hex(f.after),
        before: typeof f.before === "string" ? f.before : new Uint8Array(f.before),
        after: typeof f.after === "string" ? f.after : new Uint8Array(f.after),
      })),
    };
    this.ops.push(entry);
    for (const seq of op.undoOf ?? []) {
      this.undone.add(seq);
      this.undoneBy.set(seq, entry.seq);
    }
    return Promise.resolve({ op: entry, notes: [] });
  }

  list(sinceSeq: number): Promise<readonly JournalOp[]> {
    return Promise.resolve(this.ops.filter((op) => op.seq >= sinceSeq));
  }

  recent(query: JournalLogQuery): Promise<readonly JournalLogEntry[]> {
    const since = query.sinceSeq ?? 0;
    const before = query.beforeSeq ?? Number.POSITIVE_INFINITY;
    const matching = this.ops.filter(
      (op) =>
        op.seq >= since &&
        op.seq < before &&
        (query.sessionId === undefined || op.sessionId === query.sessionId),
    );
    return Promise.resolve(
      matching
        .slice()
        .reverse()
        .slice(0, query.limit)
        .map((op) => ({
          op,
          undoable: op.undoOf === undefined && !this.undone.has(op.seq),
          undoneBy: this.undoneBy.get(op.seq) ?? null,
        })),
    );
  }

  get(seq: number): Promise<JournalOp | null> {
    return Promise.resolve(this.ops.find((op) => op.seq === seq) ?? null);
  }

  undoCandidates(query: UndoCandidateQuery): Promise<readonly JournalOp[]> {
    const eligible = this.ops.filter(
      (op) =>
        op.undoOf === undefined &&
        !this.undone.has(op.seq) &&
        (query.sessionId === undefined || op.sessionId === query.sessionId) &&
        (query.seqs === undefined || query.seqs.includes(op.seq)),
    );
    return Promise.resolve(eligible.reverse().slice(0, query.limit));
  }
}

function sha256Hex(content: FileContent): string {
  return new Bun.CryptoHasher("sha256").update(content).digest("hex");
}

class InProcessWatcher implements WatcherPort {
  readonly version = PORT_VERSIONS.watcher;
  private readonly subscribers = new Set<{
    paths: readonly string[];
    onEvent: (event: WatchEvent) => void;
  }>();

  subscribe(
    paths: readonly string[],
    onEvent: (event: WatchEvent) => void,
  ): Promise<WatchSubscription> {
    const entry = { paths, onEvent };
    this.subscribers.add(entry);
    return Promise.resolve({
      close: () => {
        this.subscribers.delete(entry);
        return Promise.resolve();
      },
    });
  }

  /** Test hook: deliver an event to every matching subscriber, synchronously. */
  emit(event: WatchEvent): void {
    for (const { paths, onEvent } of this.subscribers) {
      if (
        paths.length === 0 ||
        paths.some((p) => event.path === p || event.path.startsWith(`${p}/`))
      ) {
        onEvent(event);
      }
    }
  }
}

class InProcessSandbox implements SandboxPort {
  readonly version = PORT_VERSIONS.sandbox;

  constructor(private readonly workspace: InProcessWorkspace) {}

  canonicalize(path: string): Promise<string> {
    const root = this.workspace.root;
    const raw = path.replaceAll("\\", "/");
    const joined = raw.startsWith("/") ? raw : `${root}/${raw}`;
    const parts: string[] = [];
    for (const part of joined.split("/")) {
      if (part === "" || part === ".") continue;
      if (part === "..") {
        if (parts.length === 0) {
          return Promise.reject(new Error(`path escapes the workspace: ${path}`));
        }
        parts.pop();
      } else {
        parts.push(part);
      }
    }
    const canonical = `/${parts.join("/")}`;
    if (canonical !== root && !canonical.startsWith(`${root}/`)) {
      return Promise.reject(new Error(`path outside the workspace: ${path}`));
    }
    return Promise.resolve(canonical);
  }

  async check(request: AccessRequest): Promise<AccessVerdict> {
    try {
      await this.canonicalize(request.path);
      return { allowed: true };
    } catch (error) {
      return { allowed: false, reason: error instanceof Error ? error.message : String(error) };
    }
  }
}

class InProcessFormatter implements FormatterPort {
  readonly version = PORT_VERSIONS.formatter;

  format(request: FormatRequest): Promise<FormatResult> {
    const trimmed = request.content
      .split("\n")
      .map((line) => line.trimEnd())
      .join("\n");
    const content = trimmed.endsWith("\n") || trimmed === "" ? trimmed : `${trimmed}\n`;
    return Promise.resolve({ content, changed: content !== request.content });
  }
}

class InProcessResolver implements ResolverPort {
  readonly version = PORT_VERSIONS.resolver;

  constructor(
    private readonly index: InProcessIndex,
    private readonly engine: InProcessEngine,
  ) {}

  async resolve(request: ResolveRequest): Promise<readonly ResolvedEdge[]> {
    // Symbolic (tree-sitter fidelity) resolution: v2 labels every edge with
    // honest provenance — never `resolver-lsp`, which only the plugin's
    // language-server-verified lane may claim.
    if (request.kind === "definition") {
      const { symbols } = await this.index.querySymbols({ name: request.symbol });
      return symbols
        .filter((s) => s.name === request.symbol)
        .map((s) => ({
          location: s.location,
          provenance: "tier1-symbols" as const,
          confidence: "high" as const,
        }));
    }
    const matches = await this.engine.search({
      mode: "lexical",
      pattern: request.symbol,
      paths: [],
    });
    return matches.map((m) => ({
      location: m.location,
      provenance: "tier1-text" as const,
      confidence: "low" as const,
    }));
  }
}

export interface InProcessPorts extends PortSet {
  /** The watcher, concretely, so tests can `emit` events. */
  readonly watcher: InProcessWatcher;
  /** The backing store, so tests can seed and inspect files. */
  readonly workspace: InProcessWorkspace;
}

export function createInProcessPorts(workspace = new InProcessWorkspace()): InProcessPorts {
  const engine = new InProcessEngine(workspace);
  const index = new InProcessIndex(workspace);
  return {
    workspace,
    engine,
    index,
    journal: new InProcessJournal(),
    watcher: new InProcessWatcher(),
    sandbox: new InProcessSandbox(workspace),
    formatter: new InProcessFormatter(),
    resolver: new InProcessResolver(index, engine),
  };
}
