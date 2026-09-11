/**
 * The `read` tool (chunk m1-read, SDD §9.3).
 *
 * runRead against fake ports, acceptance-mapped: the budget parameter is
 * honoured and a cut always carries the full truncation manifest with a
 * resumable line cursor (first criterion); line numbers are verified fresh
 * against mtime+size before return — the stat sandwich, the retry, the
 * honest cold demotion, and the symbol-anchor check (second); secrets are
 * redacted with visible [REDACTED:*] markers at the seal choke point
 * (third). Plus whole-file/lines/symbol addressing, surround handling,
 * ambiguity/missing refusals, and BUDGET_TOO_SMALL. Every envelope is
 * validated against the normative schema.
 */

import { describe, expect, test } from "bun:test";
import envelopeSchema from "../schemas/envelope.schema.json";
import type { ResponseEnvelope } from "../src/envelope";
import { sealEnvelope } from "../src/envelope";
import type {
  ImportGraphResult,
  IndexPort,
  IndexStatus,
  OutlineQueryResult,
  ReferenceQueryResult,
  SymbolEntry,
  SymbolQueryResult,
} from "../src/ports";
import { PORT_VERSIONS } from "../src/ports";
import {
  type ReadDeps,
  type ReadInput,
  type ReadResult,
  type ReadSegment,
  type ReadUnit,
  runRead,
} from "../src/tools";
import { envelopeValidatorFor } from "./support/ajv";

const { validate, errorsText } = envelopeValidatorFor(envelopeSchema);

function assertValidEnvelope(envelope: ResponseEnvelope<unknown>): void {
  expect(validate(envelope) ? "" : errorsText()).toBe("");
}

interface FakeStat {
  readonly mtimeMs: number;
  readonly size: number;
}

interface FakeSymbol {
  readonly name: string;
  readonly kind: string;
  readonly line: number;
  readonly endLine?: number;
  /** Trait a Rust impl member came from (specs/009). */
  readonly detail?: string;
  /** First line of the item's doc comment (specs/009). */
  readonly doc?: string;
}

interface FakeOptions {
  /** "throw" = no index exists at all (the pure Tier-0 workspace). */
  status?: IndexStatus | "throw";
  outlines?: Record<string, readonly FakeSymbol[]>;
  outlineFreshness?: OutlineQueryResult["freshness"];
  /** Stat answers in call order; the last repeats. Default: stable. */
  statSeq?: readonly FakeStat[];
  /** ENOENT on every access — the missing file. */
  missing?: boolean;
}

function fakeDeps(
  content: string,
  options: FakeOptions = {},
): ReadDeps & { counts: () => { reads: number; stats: number } } {
  let reads = 0;
  let stats = 0;
  const enoent = (): Promise<never> =>
    Promise.reject(Object.assign(new Error("ENOENT: no such file"), { code: "ENOENT" }));
  const index: IndexPort = {
    version: PORT_VERSIONS.index,
    status: (): Promise<IndexStatus> =>
      options.status === "throw"
        ? Promise.reject(new Error("no index exists yet"))
        : Promise.resolve(options.status ?? { tier: 1, generation: 1, building: false }),
    build: () => Promise.reject(new Error("not under test")),
    querySymbols: (): Promise<SymbolQueryResult> =>
      Promise.resolve({ symbols: [], freshness: "warm" }),
    queryReferences: (): Promise<ReferenceQueryResult> =>
      Promise.resolve({ edges: [], freshness: "warm" }),
    queryImportGraph: (): Promise<ImportGraphResult> =>
      Promise.resolve({ fanIn: [], fanOut: [], freshness: "warm" }),
    queryOutline: ({ paths }): Promise<OutlineQueryResult> =>
      Promise.resolve({
        files: paths.map((p) => ({
          path: p,
          symbols: (options.outlines?.[p] ?? []).map(
            (s): SymbolEntry => ({
              name: s.name,
              kind: s.kind,
              location: { path: p, line: s.line, column: 1 },
              ...(s.endLine === undefined ? {} : { endLine: s.endLine }),
              ...(s.detail === undefined ? {} : { detail: s.detail }),
              ...(s.doc === undefined ? {} : { doc: s.doc }),
            }),
          ),
        })),
        freshness: options.outlineFreshness ?? "warm",
      }),
    upsertFile: () => Promise.resolve(),
    refreshFile: () => Promise.resolve(),
    removeFile: () => Promise.resolve(),
  };
  return {
    index,
    workspace: {
      read: () => {
        reads += 1;
        return options.missing ? enoent() : Promise.resolve(content);
      },
      stat: () => {
        if (options.missing) return enoent();
        const seq = options.statSeq;
        const answer =
          seq === undefined
            ? { mtimeMs: 1000, size: content.length }
            : (seq[Math.min(stats, seq.length - 1)] as FakeStat);
        stats += 1;
        return Promise.resolve(answer);
      },
    },
    counts: () => ({ reads, stats }),
  };
}

async function ok(deps: ReadDeps, input: ReadInput): Promise<ResponseEnvelope<ReadResult>> {
  const envelope = await runRead(deps, input, 1);
  assertValidEnvelope(envelope);
  expect(envelope.ok).toBe(true);
  return envelope;
}

function unitOf(envelope: ResponseEnvelope<ReadResult>): ReadUnit {
  if (!envelope.ok) throw new Error("expected an ok envelope");
  const first = envelope.results[0];
  if (first?.kind !== "unit") throw new Error("first row must be the unit header");
  return first;
}

function segmentsOf(envelope: ResponseEnvelope<ReadResult>): readonly ReadSegment[] {
  if (!envelope.ok) throw new Error("expected an ok envelope");
  return envelope.results.filter((r): r is ReadSegment => r.kind === "lines");
}

async function refused(
  deps: ReadDeps,
  input: ReadInput,
  code: string,
): Promise<ResponseEnvelope<ReadResult>> {
  const envelope = await runRead(deps, input, 1);
  assertValidEnvelope(envelope);
  expect(envelope.ok).toBe(false);
  if (envelope.ok) throw new Error("unreachable");
  expect(envelope.error.code).toBe(code);
  return envelope;
}

const numberedLines = (n: number): string =>
  Array.from({ length: n }, (_, i) => `line ${i + 1} content`).join("\n");

const sha256 = (text: string): string => new Bun.CryptoHasher("sha256").update(text).digest("hex");

const SOURCE = [
  'import { a } from "./a";',
  'import { b } from "./b";',
  "",
  "export class UserService {",
  "  authenticate(user: string): boolean {",
  "    return user.length > 0;",
  "  }",
  "  revoke(user: string): void {",
  "    void user;",
  "  }",
  "}",
  "",
  "export function authenticate(raw: string): boolean {",
  "  return raw !== '';",
  "}",
].join("\n");

const OUTLINE: readonly FakeSymbol[] = [
  { name: "UserService", kind: "class", line: 4, endLine: 11 },
  { name: "authenticate", kind: "method", line: 5, endLine: 7 },
  { name: "revoke", kind: "method", line: 8, endLine: 10 },
  { name: "authenticate", kind: "function", line: 13, endLine: 15 },
];

describe("read: whole file and lines addressing", () => {
  test("whole file: unit header with anchors and hash, then contiguous segments", async () => {
    const deps = fakeDeps(SOURCE);
    const envelope = await ok(deps, { file: "src/auth.ts" });
    const unit = unitOf(envelope);
    expect(unit.file).toBe("src/auth.ts");
    expect(unit.range).toEqual({ start: 1, end: 15 });
    expect(unit.totalLines).toBe(15);
    expect(unit.contentHash).toBe(sha256(SOURCE));
    const segments = segmentsOf(envelope);
    expect(segments[0]?.start).toBe(1);
    expect(segments[segments.length - 1]?.end).toBe(15);
    expect(segments.map((s) => s.text).join("\n")).toBe(SOURCE);
  });

  test("lines: {start, end} returns exactly that range, 1-based inclusive", async () => {
    const deps = fakeDeps(numberedLines(100));
    const envelope = await ok(deps, { file: "big.txt", lines: { start: 40, end: 42 } });
    expect(unitOf(envelope).range).toEqual({ start: 40, end: 42 });
    const [segment] = segmentsOf(envelope);
    expect(segment?.text).toBe("line 40 content\nline 41 content\nline 42 content");
  });

  test("lines past EOF are clamped with a note, never invented", async () => {
    const deps = fakeDeps(numberedLines(10));
    const envelope = await ok(deps, { file: "small.txt", lines: { start: 8, end: 99 } });
    expect(unitOf(envelope).range).toEqual({ start: 8, end: 10 });
    expect(envelope.ok && envelope.notes?.some((n) => n.includes("clamped"))).toBe(true);
  });

  test("normalizes windows-style input paths to the index's posix dialect", async () => {
    const deps = fakeDeps(SOURCE);
    const envelope = await ok(deps, { file: ".\\src\\auth.ts" });
    expect(unitOf(envelope).file).toBe("src/auth.ts");
  });

  test("empty file: range {0,0}, no segments, a note", async () => {
    const deps = fakeDeps("");
    const envelope = await ok(deps, { file: "empty.txt" });
    expect(unitOf(envelope).range).toEqual({ start: 0, end: 0 });
    expect(segmentsOf(envelope)).toHaveLength(0);
    expect(envelope.ok && envelope.notes?.some((n) => n.includes("empty"))).toBe(true);
  });

  test("symbol and lines together is a caller contract violation", () => {
    const deps = fakeDeps(SOURCE);
    expect(
      runRead(deps, { file: "a.ts", symbol: "x", lines: { start: 1, end: 2 } }, 1),
    ).rejects.toThrow(RangeError);
  });

  test("missing file refuses with the narrow-recovery code and says why", async () => {
    const deps = fakeDeps("", { missing: true });
    const envelope = await refused(deps, { file: "gone.ts" }, "FILE_NOT_FOUND");
    if (envelope.ok) throw new Error("unreachable");
    expect(envelope.error.message).toContain("no such file");
    expect(envelope.error.recovery).toBe("narrow");
  });
});

describe("read: budget and the truncation manifest (acceptance 1)", () => {
  test("a cut carries dropped count, reason, hints and a resumable line cursor", async () => {
    const content = numberedLines(400);
    const deps = fakeDeps(content);
    const envelope = await ok(deps, { file: "big.txt", budget: 300 });
    if (!envelope.ok) throw new Error("unreachable");
    expect(envelope.truncation).toBeDefined();
    expect(envelope.truncation?.reason).toBe("budget");
    expect(envelope.truncation?.dropped).toBeGreaterThan(0);
    expect(envelope.budget).toEqual({ requested: 300, used: envelope.budget.used });
    expect(envelope.budget.used).toBeLessThanOrEqual(300);
    const segments = segmentsOf(envelope);
    const lastKept = segments[segments.length - 1] as ReadSegment;
    expect(envelope.truncation?.cursor).toBe(`line:${lastKept.end + 1}`);
  });

  test("the cursor resumes: lines from the cursor return the dropped tail", async () => {
    const content = numberedLines(400);
    const deps = fakeDeps(content);
    const first = await ok(deps, { file: "big.txt", budget: 300 });
    if (!first.ok) throw new Error("unreachable");
    const cursorLine = Number((first.truncation?.cursor ?? "").split(":")[1]);
    const rest = await ok(deps, { file: "big.txt", lines: { start: cursorLine, end: 400 } });
    const restSegments = segmentsOf(rest);
    expect(restSegments[0]?.start).toBe(cursorLine);
    expect(restSegments[restSegments.length - 1]?.end).toBe(400);
  });

  test("an uncut read has no truncation manifest (present iff cut)", async () => {
    const deps = fakeDeps(numberedLines(5));
    const envelope = await ok(deps, { file: "small.txt" });
    if (!envelope.ok) throw new Error("unreachable");
    expect(envelope.truncation).toBeUndefined();
  });

  test("a budget that cannot fit the unit header refuses with BUDGET_TOO_SMALL", async () => {
    const deps = fakeDeps(numberedLines(5));
    await refused(deps, { file: "small.txt", budget: 1 }, "BUDGET_TOO_SMALL");
  });
});

describe("read: mtime+size freshness verification (acceptance 2)", () => {
  test("a stable stat sandwich answers warm", async () => {
    const deps = fakeDeps(SOURCE);
    const envelope = await ok(deps, { file: "src/auth.ts" });
    if (!envelope.ok) throw new Error("unreachable");
    expect(envelope.freshness).toBe("warm");
    expect(deps.counts()).toEqual({ reads: 1, stats: 2 });
  });

  test("a torn first read retries once and answers warm when stable", async () => {
    const stable = { mtimeMs: 2000, size: SOURCE.length };
    const deps = fakeDeps(SOURCE, {
      statSeq: [{ mtimeMs: 1000, size: 10 }, stable, stable, stable],
    });
    const envelope = await ok(deps, { file: "src/auth.ts" });
    if (!envelope.ok) throw new Error("unreachable");
    expect(envelope.freshness).toBe("warm");
    expect(deps.counts().reads).toBe(2);
  });

  test("a file that keeps changing is served honestly cold, with a note", async () => {
    // Four distinct answers: both sandwich attempts see a torn read.
    const deps = fakeDeps(SOURCE, {
      statSeq: [
        { mtimeMs: 1, size: 1 },
        { mtimeMs: 2, size: 2 },
        { mtimeMs: 3, size: 3 },
        { mtimeMs: 4, size: 4 },
      ],
    });
    const envelope = await ok(deps, { file: "src/auth.ts" });
    if (!envelope.ok) throw new Error("unreachable");
    expect(envelope.freshness).toBe("cold");
    expect(envelope.notes?.some((n) => n.includes("mtime+size mismatch"))).toBe(true);
  });

  test("symbol mode: the envelope never claims fresher than the outline answer", async () => {
    const deps = fakeDeps(SOURCE, {
      outlines: { "src/auth.ts": OUTLINE },
      outlineFreshness: "cold",
    });
    const envelope = await ok(deps, {
      file: "src/auth.ts",
      symbol: "UserService.authenticate",
    });
    if (!envelope.ok) throw new Error("unreachable");
    expect(envelope.freshness).toBe("cold");
  });

  test("a symbol anchor that does not match current content demotes to cold", async () => {
    const deps = fakeDeps(SOURCE, {
      outlines: { "src/auth.ts": [{ name: "vanished", kind: "function", line: 2, endLine: 3 }] },
    });
    const envelope = await ok(deps, { file: "src/auth.ts", symbol: "vanished" });
    if (!envelope.ok) throw new Error("unreachable");
    expect(envelope.freshness).toBe("cold");
    expect(envelope.notes?.some((n) => n.includes("did not match the current content"))).toBe(true);
  });
});

describe("read: symbol addressing", () => {
  test("a dotted address resolves the member inside its parent by containment", async () => {
    const deps = fakeDeps(SOURCE, { outlines: { "src/auth.ts": OUTLINE } });
    const envelope = await ok(deps, {
      file: "src/auth.ts",
      symbol: "UserService.authenticate",
      surround: "none",
    });
    const unit = unitOf(envelope);
    expect(unit.symbol).toEqual({ name: "authenticate", kind: "method", line: 5, endLine: 7 });
    expect(unit.range).toEqual({ start: 5, end: 7 });
    expect(segmentsOf(envelope)[0]?.text).toContain("authenticate(user: string)");
  });

  test("surround: enclosing widens the range to the parent symbol", async () => {
    const deps = fakeDeps(SOURCE, { outlines: { "src/auth.ts": OUTLINE } });
    const envelope = await ok(deps, {
      file: "src/auth.ts",
      symbol: "UserService.revoke",
      surround: "enclosing",
    });
    const unit = unitOf(envelope);
    expect(unit.symbol?.name).toBe("revoke");
    expect(unit.enclosing).toEqual({ name: "UserService", kind: "class", line: 4, endLine: 11 });
    expect(unit.range).toEqual({ start: 4, end: 11 });
  });

  test("a bare ambiguous name refuses with line-labeled candidates", async () => {
    const deps = fakeDeps(SOURCE, { outlines: { "src/auth.ts": OUTLINE } });
    const envelope = await refused(
      deps,
      { file: "src/auth.ts", symbol: "authenticate" },
      "AMBIGUOUS_SYMBOL",
    );
    if (envelope.ok) throw new Error("unreachable");
    expect(envelope.error.candidates).toEqual([
      "authenticate (method) line 5",
      "authenticate (function) line 13",
    ]);
  });

  test("an unknown symbol refuses with SYMBOL_NOT_FOUND and names what exists", async () => {
    const deps = fakeDeps(SOURCE, { outlines: { "src/auth.ts": OUTLINE } });
    const envelope = await refused(
      deps,
      { file: "src/auth.ts", symbol: "nonesuch" },
      "SYMBOL_NOT_FOUND",
    );
    if (envelope.ok) throw new Error("unreachable");
    expect(envelope.notes?.[0]).toContain("UserService");
  });

  test("symbol addressing without a Tier-1 index refuses with INDEX_UNAVAILABLE", async () => {
    const deps = fakeDeps(SOURCE, { status: "throw" });
    const envelope = await refused(
      deps,
      { file: "src/auth.ts", symbol: "authenticate" },
      "INDEX_UNAVAILABLE",
    );
    if (envelope.ok) throw new Error("unreachable");
    expect(envelope.error.recovery).toBe("retry");
  });

  test("surround: imports attaches the leading import block as its own row", async () => {
    const deps = fakeDeps(SOURCE, { outlines: { "src/auth.ts": OUTLINE } });
    const envelope = await ok(deps, {
      file: "src/auth.ts",
      symbol: "UserService.revoke",
      surround: "imports",
    });
    if (!envelope.ok) throw new Error("unreachable");
    const imports = envelope.results.find(
      (r): r is ReadSegment => r.kind === "imports",
    ) as ReadSegment;
    expect(imports.start).toBe(1);
    expect(imports.end).toBe(2);
    expect(imports.text).toBe('import { a } from "./a";\nimport { b } from "./b";');
    expect(envelope.notes?.some((n) => n.includes("lexical scan"))).toBe(true);
  });
});

describe("read: secrets are redacted with visible markers (acceptance 3)", () => {
  test("sealing a read envelope replaces credentials with [REDACTED:*] markers", async () => {
    const secretFile = [
      "aws_access_key_id = AKIAIOSFODNN7EXAMPLE",
      'password = "hunter2-super-secret"',
      "plain line stays intact",
    ].join("\n");
    const deps = fakeDeps(secretFile);
    const envelope = await ok(deps, { file: ".env" });
    // Pre-seal, the raw envelope still carries the secret — the choke point
    // is sealEnvelope (SDD §7.1), which the protocol layer always applies.
    expect(JSON.stringify(envelope)).toContain("AKIAIOSFODNN7EXAMPLE");
    const sealed = sealEnvelope(envelope);
    expect(sealed.json).not.toContain("AKIAIOSFODNN7EXAMPLE");
    expect(sealed.json).toContain("[REDACTED:aws-key]");
    expect(sealed.json).toContain("[REDACTED:credential-assignment]");
    expect(sealed.json).toContain("plain line stays intact");
  });
});

// ------------------------------------------- Rust addressing (specs/009)
// The index now stores impl members qualified (`Type::member`) with the trait
// they came from, so `read` must address them the way Rust names them.

const RUST_SOURCE = [
  "pub struct Config;", // 1
  "", // 2
  "impl Config {", // 3
  "    pub fn load() -> u32 {", // 4
  "        1", // 5
  "    }", // 6
  "", // 7
  "    pub fn hello(&self) -> u32 {", // 8
  "        2", // 9
  "    }", // 10
  "}", // 11
  "", // 12
  "impl Greeter for Config {", // 13
  "    fn hello(&self) -> u32 {", // 14
  "        3", // 15
  "    }", // 16
  "}", // 17
  "",
].join(String.fromCharCode(10));

const RUST_OUTLINE: readonly FakeSymbol[] = [
  { name: "Config", kind: "struct", line: 1, endLine: 1, doc: "The configuration." },
  { name: "Config::load", kind: "method", line: 4, endLine: 6, doc: "Reads it." },
  { name: "Config::hello", kind: "method", line: 8, endLine: 10 },
  { name: "Config::hello", kind: "method", line: 14, endLine: 16, detail: "Greeter" },
];

describe("read: Rust symbol addressing (specs/009)", () => {
  test("a `::` address resolves the qualified member to its own extent", async () => {
    const deps = fakeDeps(RUST_SOURCE, { outlines: { "src/cfg.rs": RUST_OUTLINE } });
    const envelope = await ok(deps, {
      file: "src/cfg.rs",
      symbol: "Config::load",
      surround: "none",
    });
    const unit = unitOf(envelope);
    expect(unit.range).toEqual({ start: 4, end: 6 });
    expect(unit.symbol?.name).toBe("Config::load");
    // The doc summary rides along, so an agent sees what the item is for.
    expect(unit.symbol?.doc).toBe("Reads it.");
    expect(segmentsOf(envelope)[0]?.text).toContain("pub fn load()");
  });

  test("a bare member name falls back through the `::` split and stays addressable", async () => {
    const deps = fakeDeps(RUST_SOURCE, { outlines: { "src/cfg.rs": RUST_OUTLINE } });
    const envelope = await ok(deps, { file: "src/cfg.rs", symbol: "load", surround: "none" });
    expect(unitOf(envelope).range).toEqual({ start: 4, end: 6 });
  });

  test("a genuinely ambiguous member names the TRAIT in its candidates", async () => {
    const deps = fakeDeps(RUST_SOURCE, { outlines: { "src/cfg.rs": RUST_OUTLINE } });
    const envelope = await refused(
      deps,
      { file: "src/cfg.rs", symbol: "Config::hello" },
      "AMBIGUOUS_SYMBOL",
    );
    if (envelope.ok) throw new Error("unreachable");
    // The inherent member keeps its plain address; the trait member is
    // disambiguated with Rust's own fully-qualified syntax (specs/009 R2).
    expect(envelope.error.candidates).toEqual([
      "Config::hello (method) line 8",
      "<Config as Greeter>::hello (method) line 14",
    ]);
  });

  test("an undocumented symbol carries no doc key at all", async () => {
    const deps = fakeDeps(RUST_SOURCE, { outlines: { "src/cfg.rs": RUST_OUTLINE } });
    const envelope = await ok(deps, {
      file: "src/cfg.rs",
      symbol: "Config",
      surround: "none",
    });
    const unit = unitOf(envelope);
    expect(unit.symbol?.doc).toBe("The configuration.");
    const bare = await ok(fakeDeps(SOURCE, { outlines: { "src/auth.ts": OUTLINE } }), {
      file: "src/auth.ts",
      symbol: "UserService.revoke",
      surround: "none",
    });
    expect(Object.hasOwn(unitOf(bare).symbol ?? {}, "doc")).toBe(false);
  });
});
