/**
 * The JSONL flight recorder (feature 005-tracing-ring-buffer, research R1/R3).
 *
 * OTel-shaped span records — vocabulary, not SDK (zero dependencies) — as
 * one JSON line each in `.workbench/trace.jsonl`. The "ring buffer" is the
 * shipped shadow-log mechanism reused verbatim: a fixed documented cap, the
 * newest half of lines kept on breach, every IO error swallowed, appends
 * fire-and-forget. Tracing can never fail, block, or slow a request
 * (FR-006): the request path never awaits trace IO — writes chain onto an
 * internal queue whose failures vanish.
 *
 * Cache-class data (SDD §10.2): unreadable ⇒ discardable; losing the file
 * loses history, never correctness. No migration ever runs on it.
 */

import { AsyncLocalStorage } from "node:async_hooks";
import { randomBytes } from "node:crypto";
import { appendFile, readFile, stat, writeFile } from "node:fs/promises";
import { recoveryFor } from "../envelope";
import type { EngineSpan, RootEndAttributes, SpanStatus, ToolSpan, Tracer } from "../ports/tracing";

/** Record format version (Constitution V); additive-only within the major. */
export const TRACE_RECORD_VERSION = 1;

/** Trace-log size cap (research R3); newest half of lines survives a breach. */
export const TRACE_LOG_CAP_BYTES = 8 * 1024 * 1024;

/** The wire shape of one line (contracts/trace-record.md). */
interface SpanRecord {
  readonly v: number;
  readonly traceId: string;
  readonly spanId: string;
  readonly parentSpanId?: string;
  readonly name: string;
  readonly startUnixMs: number;
  readonly durationMs: number;
  readonly status:
    | { readonly ok: true }
    | { readonly ok: false; readonly code: string; readonly recovery: string };
  readonly attributes: Readonly<Record<string, string | number | boolean>>;
}

/** OTel id widths: 16-byte trace id, 8-byte span id, lowercase hex. */
const traceId = (): string => randomBytes(16).toString("hex");
const spanId = (): string => randomBytes(8).toString("hex");

function wireStatus(status: SpanStatus): SpanRecord["status"] {
  return status.ok
    ? { ok: true }
    : { ok: false, code: status.code, recovery: recoveryFor(status.code) };
}

class JsonlEngineSpan implements EngineSpan {
  private ended = false;
  private readonly startUnixMs = Date.now();
  private readonly startMono = performance.now();

  constructor(
    private readonly tracer: JsonlTracer,
    private readonly ids: { readonly traceId: string; readonly parentSpanId: string },
    private readonly method: string,
  ) {}

  end(status: SpanStatus): void {
    if (this.ended) return;
    this.ended = true;
    this.tracer.emit({
      v: TRACE_RECORD_VERSION,
      traceId: this.ids.traceId,
      spanId: spanId(),
      parentSpanId: this.ids.parentSpanId,
      name: `engine:${this.method}`,
      startUnixMs: this.startUnixMs,
      durationMs: performance.now() - this.startMono,
      status: wireStatus(status),
      attributes: { method: this.method },
    });
  }
}

class JsonlToolSpan implements ToolSpan {
  readonly traceId = traceId();
  readonly spanId = spanId();
  private ended = false;
  private readonly startUnixMs = Date.now();
  private readonly startMono = performance.now();

  constructor(
    private readonly tracer: JsonlTracer,
    private readonly tool: string,
  ) {}

  engineChild(attributes: { readonly method: string }): EngineSpan {
    return new JsonlEngineSpan(
      this.tracer,
      { traceId: this.traceId, parentSpanId: this.spanId },
      attributes.method,
    );
  }

  end(status: SpanStatus, result?: RootEndAttributes): void {
    if (this.ended) return;
    this.ended = true;
    this.tracer.emit({
      v: TRACE_RECORD_VERSION,
      traceId: this.traceId,
      spanId: this.spanId,
      name: `tool:${this.tool}`,
      startUnixMs: this.startUnixMs,
      durationMs: performance.now() - this.startMono,
      status: wireStatus(status),
      attributes: {
        tool: this.tool,
        ...(result?.envelopeV === undefined ? {} : { envelopeV: result.envelopeV }),
        ...(result?.notesCount === undefined ? {} : { notesCount: result.notesCount }),
        ...(result?.truncated === undefined ? {} : { truncated: result.truncated }),
      },
    });
  }
}

export class JsonlTracer implements Tracer {
  private readonly active = new AsyncLocalStorage<ToolSpan>();
  /** Serializes writes so cap eviction never races an append. */
  private tail: Promise<void> = Promise.resolve();

  constructor(private readonly path: string) {}

  startToolSpan(attributes: { readonly tool: string }): ToolSpan {
    return new JsonlToolSpan(this, attributes.tool);
  }

  activeSpan(): ToolSpan | null {
    return this.active.getStore() ?? null;
  }

  runWith<T>(span: ToolSpan, fn: () => T): T {
    return this.active.run(span, fn);
  }

  /** Fire-and-forget: callers never await; every failure is swallowed. */
  emit(record: SpanRecord): void {
    const line = JSON.stringify(record);
    this.tail = this.tail.then(() => this.write(line)).catch(() => {});
  }

  /** Wait for queued writes to land — a seam for tests and shutdown only. */
  settle(): Promise<void> {
    return this.tail.catch(() => {});
  }

  private async write(line: string): Promise<void> {
    try {
      const size = await stat(this.path).then(
        (s) => s.size,
        () => 0,
      );
      if (size + line.length + 1 > TRACE_LOG_CAP_BYTES) {
        const existing = await readFile(this.path, "utf8").catch(() => "");
        const lines = existing.split("\n").filter((l) => l.length > 0);
        const kept = lines.slice(Math.ceil(lines.length / 2));
        await writeFile(this.path, `${[...kept, line].join("\n")}\n`, "utf8");
      } else {
        await appendFile(this.path, `${line}\n`, "utf8");
      }
    } catch {
      // Tracing must never fail anything (FR-006).
    }
  }
}
