/**
 * Frame protocol unit tests (SDD §6.3): encode/decode roundtrips, split and
 * coalesced delivery, the soft-vs-fatal failure split, and the version gate.
 */

import { describe, expect, test } from "bun:test";
import {
  encodeFrame,
  type Frame,
  FrameDecodeError,
  FrameDecoder,
  IPC_VERSION,
  LENGTH_PREFIX_BYTES,
  MAX_FRAME_BYTES,
  readFrame,
} from "../src/adapters/engine-ipc";

const FRAMES = [
  { v: IPC_VERSION, kind: "req", id: 1, method: "parse", params: { path: "a.ts", source: "x" } },
  { v: IPC_VERSION, kind: "res", id: 1, result: { ok: true } },
  { v: IPC_VERSION, kind: "err", id: 2, error: { code: "ENGINE_UNAVAILABLE", message: "down" } },
  { v: IPC_VERSION, kind: "cancel", id: 3 },
  { v: IPC_VERSION, kind: "ready", core: { flavor: "native", version: "0.0.0" } },
] as const satisfies readonly Frame[];

describe("frame codec", () => {
  test("every frame kind roundtrips through encode → decode", () => {
    const decoder = new FrameDecoder();
    for (const frame of FRAMES) {
      const decoded = decoder.push(encodeFrame(frame));
      expect(decoded).toEqual([{ ok: true, frame }]);
    }
    expect(decoder.pendingBytes).toBe(0);
  });

  test("byte-by-byte delivery yields the same frames", () => {
    const decoder = new FrameDecoder();
    const bytes = encodeFrame(FRAMES[0]);
    const out = [];
    for (const byte of bytes) out.push(...decoder.push(new Uint8Array([byte])));
    expect(out).toEqual([{ ok: true, frame: FRAMES[0] }]);
  });

  test("several frames coalesced into one chunk all decode", () => {
    const chunks = FRAMES.map((f) => encodeFrame(f));
    const merged = new Uint8Array(chunks.reduce((n, c) => n + c.length, 0));
    let offset = 0;
    for (const chunk of chunks) {
      merged.set(chunk, offset);
      offset += chunk.length;
    }
    const decoded = new FrameDecoder().push(merged);
    expect(decoded.map((d) => (d.ok ? d.frame : null))).toEqual([...FRAMES]);
  });

  test("an oversized length prefix is stream-fatal", () => {
    const bytes = new Uint8Array(LENGTH_PREFIX_BYTES);
    new DataView(bytes.buffer).setUint32(0, MAX_FRAME_BYTES + 1, true);
    expect(() => new FrameDecoder().push(bytes)).toThrow(FrameDecodeError);
  });

  test("a non-JSON payload is stream-fatal", () => {
    const payload = new TextEncoder().encode("not json");
    const bytes = new Uint8Array(LENGTH_PREFIX_BYTES + payload.length);
    new DataView(bytes.buffer).setUint32(0, payload.length, true);
    bytes.set(payload, LENGTH_PREFIX_BYTES);
    expect(() => new FrameDecoder().push(bytes)).toThrow(FrameDecodeError);
  });

  test("a version mismatch is soft and keeps the peer's id answerable", () => {
    const decoded = readFrame({ v: IPC_VERSION + 1, kind: "req", id: 7, method: "ping" });
    expect(decoded.ok).toBe(false);
    if (!decoded.ok) {
      expect(decoded.id).toBe(7);
      expect(decoded.error.code).toBe("UNSUPPORTED_VERSION");
    }
  });

  test("unknown kinds and broken shapes are soft errors, not throws", () => {
    for (const value of [
      { v: IPC_VERSION, kind: "nope", id: 1 },
      { v: IPC_VERSION, kind: "req", id: "one", method: "ping" },
      { v: IPC_VERSION, kind: "err", id: 1, error: { code: 5 } },
      { v: IPC_VERSION, kind: "ready", core: {} },
      [],
      "string",
      null,
    ]) {
      const decoded = readFrame(value);
      expect(decoded.ok).toBe(false);
      if (!decoded.ok) expect(decoded.error.code).toBe("MALFORMED_FRAME");
    }
  });

  test("partial trailing bytes are reported, not silently dropped", () => {
    const decoder = new FrameDecoder();
    const bytes = encodeFrame(FRAMES[3]);
    decoder.push(bytes.subarray(0, bytes.length - 2));
    expect(decoder.pendingBytes).toBeGreaterThan(0);
    const decoded = decoder.push(bytes.subarray(bytes.length - 2));
    expect(decoded).toEqual([{ ok: true, frame: FRAMES[3] }]);
    expect(decoder.pendingBytes).toBe(0);
  });
});
