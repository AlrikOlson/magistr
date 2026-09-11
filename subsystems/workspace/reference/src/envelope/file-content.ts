/** File snapshots stay lossless across the write engine and durable journal.
 * Strings are UTF-8 text; byte arrays are opaque and never parsed/formatted. */
export type FileContent = string | Uint8Array;

export function contentBytes(content: FileContent): Uint8Array {
  return typeof content === "string" ? Buffer.from(content, "utf8") : content;
}

/** NUL or non-UTF-8 content is binary, independent of filename. Round-trip
 * comparison preserves BOMs and catches invalid sequences without U+FFFD loss. */
export function decodeContent(bytes: Uint8Array): FileContent {
  const buffer = Buffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (buffer.includes(0)) return bytes;
  const text = buffer.toString("utf8");
  return buffer.equals(Buffer.from(text, "utf8")) ? text : bytes;
}

export function contentEqual(a: FileContent | null, b: FileContent | null): boolean {
  if (a === b) return true;
  if (a === null || b === null) return false;
  if (typeof a === "string" && typeof b === "string") return false;
  const bytes = contentBytes(a);
  return Buffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength).equals(contentBytes(b));
}

export function contentHash(content: FileContent): string {
  return new Bun.CryptoHasher("sha256").update(content).digest("hex");
}
