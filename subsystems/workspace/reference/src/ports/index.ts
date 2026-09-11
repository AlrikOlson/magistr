/**
 * The seven-port vocabulary of SDD §6.4 — the only surface the tool layer
 * may depend on. The dependency rule (protocol → tools → ports ← adapters)
 * is enforced mechanically by `scripts/check-import-graph.ts` in CI.
 */

import type { EnginePort } from "./engine";
import type { FormatterPort } from "./formatter";
import type { IndexPort } from "./index-port";
import type { JournalPort } from "./journal";
import type { ResolverPort } from "./resolver";
import type { SandboxPort } from "./sandbox";
import type { WatcherPort } from "./watcher";

/** One of everything — what an adapter family provides and a server composes. */
export interface PortSet {
  readonly engine: EnginePort;
  readonly index: IndexPort;
  readonly journal: JournalPort;
  readonly watcher: WatcherPort;
  readonly sandbox: SandboxPort;
  readonly formatter: FormatterPort;
  readonly resolver: ResolverPort;
}

export type {
  EngineMatch,
  EnginePort,
  EngineSearchRequest,
  ParseRequest,
  ParseVerdict,
  RewriteEdit,
  RewriteRequest,
  RewriteResult,
  SearchMode,
} from "./engine";
export type { FormatRequest, FormatResult, FormatterPort } from "./formatter";
export type {
  CrateImportEdge,
  CrateScope,
  ImportGraphEntry,
  ImportGraphQuery,
  ImportGraphResult,
  IndexedFile,
  IndexPort,
  IndexStatus,
  IndexTier,
  OutlineQuery,
  OutlineQueryResult,
  ReferenceConfidence,
  ReferenceEdge,
  ReferenceProvenance,
  ReferenceQuery,
  ReferenceQueryResult,
  SymbolEntry,
  SymbolQuery,
  SymbolQueryResult,
  TestRange,
} from "./index-port";
export type {
  JournalAppendResult,
  JournalFileChange,
  JournalFileKind,
  JournalFileRecord,
  JournalLogEntry,
  JournalLogQuery,
  JournalOp,
  JournalOpInit,
  JournalPort,
  UndoCandidateQuery,
} from "./journal";
export type { WriteLock } from "./lock";
export type { ResolvedEdge, ResolveKind, ResolveRequest, ResolverPort } from "./resolver";
export type { AccessIntent, AccessRequest, AccessVerdict, SandboxPort } from "./sandbox";
export type { EngineSpan, RootEndAttributes, SpanStatus, ToolSpan, Tracer } from "./tracing";
export { noopTracer } from "./tracing";
export type { SourceLocation } from "./types";
export type { PortName } from "./versions";
export { PORT_NAMES, PORT_VERSIONS } from "./versions";
export type {
  WatchEvent,
  WatchEventKind,
  WatcherPort,
  WatchSubscription,
} from "./watcher";
