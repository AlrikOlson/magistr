/**
 * The engine IPC frame protocol (SDD §6.3) — versioned, length-prefixed
 * frames over pipes: a u32le byte length followed by a UTF-8 JSON payload.
 * Request-scoped and shared-nothing; every request carries an id so
 * cancellation can propagate (§7.7).
 *
 * Two failure classes, deliberately distinct:
 *  - per-frame problems (unknown shape, version mismatch) are returned as
 *    soft results so the peer can be answered with an `err` frame;
 *  - stream corruption (oversized length, non-JSON bytes) throws
 *    {@link FrameDecodeError}, because nothing after it can be trusted.
 */

/** Protocol major version. Within a major, evolution is additive only. */
export const IPC_VERSION = 1;

/** u32le byte-length prefix. */
export const LENGTH_PREFIX_BYTES = 4;

/** Hard ceiling on one frame's payload; beyond it the stream is corrupt. */
export const MAX_FRAME_BYTES = 32 * 1024 * 1024;

export interface RequestFrame {
  readonly v: number;
  readonly kind: "req";
  readonly id: number;
  readonly method: string;
  readonly params: unknown;
}

export interface ResponseFrame {
  readonly v: number;
  readonly kind: "res";
  readonly id: number;
  readonly result: unknown;
}

export interface FrameError {
  readonly code: string;
  readonly message: string;
}

export interface ErrorFrame {
  readonly v: number;
  readonly kind: "err";
  readonly id: number;
  readonly error: FrameError;
}

export interface CancelFrame {
  readonly v: number;
  readonly kind: "cancel";
  readonly id: number;
}

/** Sent once by the engine child after its startup self-test passed. */
export interface ReadyFrame {
  readonly v: number;
  readonly kind: "ready";
  readonly core: { readonly flavor: string; readonly version: string };
}

export type Frame = RequestFrame | ResponseFrame | ErrorFrame | CancelFrame | ReadyFrame;

/** A structurally valid frame, or what to answer the peer with instead. */
export type DecodedFrame =
  | { readonly ok: true; readonly frame: Frame }
  | { readonly ok: false; readonly id: number | null; readonly error: FrameError };

/** The stream itself is corrupt; the connection must be torn down. */
export class FrameDecodeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "FrameDecodeError";
  }
}

export function encodeFrame(frame: Frame): Uint8Array {
  const payload = new TextEncoder().encode(JSON.stringify(frame));
  const bytes = new Uint8Array(LENGTH_PREFIX_BYTES + payload.length);
  new DataView(bytes.buffer).setUint32(0, payload.length, true);
  bytes.set(payload, LENGTH_PREFIX_BYTES);
  return bytes;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function idOf(value: Record<string, unknown>): number | null {
  return typeof value["id"] === "number" ? (value["id"] as number) : null;
}

/** Classify one parsed payload as a frame or a soft, answerable error. */
export function readFrame(value: unknown): DecodedFrame {
  if (!isRecord(value)) {
    return { ok: false, id: null, error: { code: "MALFORMED_FRAME", message: "not an object" } };
  }
  const id = idOf(value);
  if (value["v"] !== IPC_VERSION) {
    return {
      ok: false,
      id,
      error: {
        code: "UNSUPPORTED_VERSION",
        message: `frame version ${String(value["v"])} != ${IPC_VERSION}`,
      },
    };
  }
  const kind = value["kind"];
  const malformed = (message: string): DecodedFrame => ({
    ok: false,
    id,
    error: { code: "MALFORMED_FRAME", message },
  });
  switch (kind) {
    case "req":
      return id !== null && typeof value["method"] === "string"
        ? { ok: true, frame: value as unknown as RequestFrame }
        : malformed("req frame requires numeric id and string method");
    case "res":
      return id !== null
        ? { ok: true, frame: value as unknown as ResponseFrame }
        : malformed("res frame requires numeric id");
    case "err": {
      const error = value["error"];
      return id !== null &&
        isRecord(error) &&
        typeof error["code"] === "string" &&
        typeof error["message"] === "string"
        ? { ok: true, frame: value as unknown as ErrorFrame }
        : malformed("err frame requires numeric id and {code, message} error");
    }
    case "cancel":
      return id !== null
        ? { ok: true, frame: value as unknown as CancelFrame }
        : malformed("cancel frame requires numeric id");
    case "ready": {
      const core = value["core"];
      return isRecord(core) &&
        typeof core["flavor"] === "string" &&
        typeof core["version"] === "string"
        ? { ok: true, frame: value as unknown as ReadyFrame }
        : malformed("ready frame requires {flavor, version} core");
    }
    default:
      return malformed(`unknown frame kind ${JSON.stringify(kind)}`);
  }
}

/**
 * Incremental frame decoder: feed it byte chunks as they arrive, get back
 * every complete frame. Holds at most one partial frame of buffered bytes.
 */
export class FrameDecoder {
  private buffer = new Uint8Array(0);

  push(chunk: Uint8Array): DecodedFrame[] {
    if (chunk.length > 0) {
      const merged = new Uint8Array(this.buffer.length + chunk.length);
      merged.set(this.buffer);
      merged.set(chunk, this.buffer.length);
      this.buffer = merged;
    }
    const frames: DecodedFrame[] = [];
    while (this.buffer.length >= LENGTH_PREFIX_BYTES) {
      const view = new DataView(this.buffer.buffer, this.buffer.byteOffset);
      const length = view.getUint32(0, true);
      if (length > MAX_FRAME_BYTES) {
        throw new FrameDecodeError(`frame length ${length} exceeds ${MAX_FRAME_BYTES}`);
      }
      if (this.buffer.length < LENGTH_PREFIX_BYTES + length) break;
      const payload = this.buffer.subarray(LENGTH_PREFIX_BYTES, LENGTH_PREFIX_BYTES + length);
      this.buffer = this.buffer.slice(LENGTH_PREFIX_BYTES + length);
      let parsed: unknown;
      try {
        parsed = JSON.parse(new TextDecoder().decode(payload));
      } catch (error) {
        throw new FrameDecodeError(
          `frame payload is not JSON: ${error instanceof Error ? error.message : String(error)}`,
        );
      }
      frames.push(readFrame(parsed));
    }
    return frames;
  }

  /** Bytes buffered toward an incomplete frame — non-zero on dirty EOF. */
  get pendingBytes(): number {
    return this.buffer.length;
  }
}
