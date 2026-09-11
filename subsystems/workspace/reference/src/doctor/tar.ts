/**
 * Minimal POSIX ustar writer for the doctor bundle (SDD §11.2, research R3).
 *
 * Deliberately narrow: flat member names under 100 bytes, regular files
 * only, no pax extensions, no links, no long-name logic. That subset is the
 * whole of what a diagnostic bundle needs, keeps the writer auditable, and
 * costs zero dependencies (stack commitment). Any `tar -xzf` opens the
 * result; `test/doctor.test.ts` parses the headers with an independent
 * reader to pin conformance.
 */

const BLOCK = 512;

export interface TarMember {
  /** Full member path, e.g. "workbench-doctor/manifest.json". ≤100 bytes. */
  readonly name: string;
  readonly data: Uint8Array;
}

/** Zero-padded octal with a trailing NUL, per the ustar numeric fields. */
function octal(value: number, width: number): Uint8Array {
  const text = value.toString(8).padStart(width - 1, "0");
  const out = new Uint8Array(width);
  new TextEncoder().encodeInto(text, out);
  out[width - 1] = 0;
  return out;
}

function ascii(text: string, width: number): Uint8Array {
  const out = new Uint8Array(width);
  const encoded = new TextEncoder().encode(text);
  if (encoded.length > width) {
    throw new Error(`tar field overflow: ${JSON.stringify(text)} exceeds ${width} bytes`);
  }
  out.set(encoded);
  return out;
}

function header(name: string, size: number, mtimeSeconds: number): Uint8Array {
  if (new TextEncoder().encode(name).length > 100) {
    throw new Error(`tar member name exceeds 100 bytes: ${name}`);
  }
  const h = new Uint8Array(BLOCK);
  h.set(ascii(name, 100), 0); // name
  h.set(ascii("0000644", 8), 100); // mode (NUL-terminated by ascii pad)
  h.set(octal(0, 8), 108); // uid
  h.set(octal(0, 8), 116); // gid
  h.set(octal(size, 12), 124); // size
  h.set(octal(Math.max(0, Math.floor(mtimeSeconds)), 12), 136); // mtime
  // chksum is computed over the header with the field set to 8 spaces.
  h.fill(0x20, 148, 156);
  h[156] = 0x30; // typeflag '0' — regular file
  h.set(ascii("ustar", 6), 257); // magic, NUL-terminated
  h.set(ascii("00", 2), 263); // version — exactly "00", no NUL
  let sum = 0;
  for (const byte of h) sum += byte;
  // "%06o\0 ": six octal digits, NUL, space.
  const chk = sum.toString(8).padStart(6, "0");
  h.set(new TextEncoder().encode(chk), 148);
  h[154] = 0;
  h[155] = 0x20;
  return h;
}

/** Serialize members into a ustar archive (uncompressed). */
export function tarArchive(
  members: readonly TarMember[],
  mtimeSeconds: number,
): Uint8Array<ArrayBuffer> {
  const chunks: Uint8Array[] = [];
  for (const member of members) {
    chunks.push(header(member.name, member.data.length, mtimeSeconds));
    chunks.push(member.data);
    const pad = (BLOCK - (member.data.length % BLOCK)) % BLOCK;
    if (pad > 0) chunks.push(new Uint8Array(pad));
  }
  // End-of-archive: two zero blocks.
  chunks.push(new Uint8Array(BLOCK * 2));
  const total = chunks.reduce((n, c) => n + c.length, 0);
  const out = new Uint8Array(new ArrayBuffer(total));
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}

/** The bundle artifact: gzip-compressed ustar. */
export function tarGz(
  members: readonly TarMember[],
  mtimeSeconds: number,
): Uint8Array<ArrayBuffer> {
  return Bun.gzipSync(tarArchive(members, mtimeSeconds));
}
