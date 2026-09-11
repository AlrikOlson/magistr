/**
 * Shared vocabulary of the perf benches (specs/016-rust-bench-real-repo):
 * argument parsing, the `--root` guard rails, the tier-0 probe default,
 * LOC counted from the index's own files table, reference-query sampling
 * from the symbols table, percentiles, the constitution's gates judged as
 * written (tier-1 normalized per 500k LOC), machine and git facts, and the
 * `workbench-bench/1` JSON report every bench writes with `--json`.
 *
 * Nothing here runs inside a timed window: LOC counting and sampling happen
 * after the build settles.
 */

import type { Database } from "bun:sqlite";
import { execFileSync } from "node:child_process";
import {
  closeSync,
  existsSync,
  mkdirSync,
  openSync,
  readFileSync,
  readSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { arch, cpus, platform, totalmem } from "node:os";
import path from "node:path";

export const BENCH_SCHEMA = "workbench-bench/1";

/** Constitution VI / SDD §7.3.2 / §6.3 gates, as written. */
export const TIER0_GATE_S = 5;
export const TIER1_GATE_S = 60;
export const TIER1_GATE_LOC = 500_000;
export const REFS_P95_GATE_MS = 50;
export const IPC_P95_GATE_MS = 1;

/** Tier-1 poll bound under `--root` when `--deadline` is not given. */
export const ROOT_DEADLINE_S = 900;
/** The query every references run includes as its guaranteed miss. */
export const MISS_NAME = "bench_refs_no_such_symbol";
/** Hot / rare samples per role in `--root` mode. */
export const SAMPLES_PER_ROLE = 3;
/** Names shorter than this are not sampled as hot (`_`, `x`, `ok`…). */
export const MIN_HOT_NAME_LENGTH = 3;

export class BenchUsageError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BenchUsageError";
  }
}

export interface BenchArgs {
  readonly root?: string;
  readonly probe?: string;
  readonly json?: string;
  readonly keepIndex: boolean;
  readonly deadlineS?: number;
  readonly loc: number;
  readonly stub: boolean;
  readonly reportOnly: boolean;
}

/** `--flag value` pairs and boolean switches; unknown flags are ignored as before. */
export function parseBenchArgs(argv: readonly string[]): BenchArgs {
  const value = (flag: string): string | undefined => {
    const at = argv.indexOf(flag);
    if (at < 0) return undefined;
    const next = argv[at + 1];
    if (next === undefined || next.startsWith("--")) {
      throw new BenchUsageError(`${flag} needs a value`);
    }
    return next;
  };
  const number = (flag: string, fallback: number): number => {
    const raw = value(flag);
    if (raw === undefined) return fallback;
    const parsed = Number(raw);
    if (!Number.isFinite(parsed) || parsed <= 0) {
      throw new BenchUsageError(`${flag} must be a positive number, got ${raw}`);
    }
    return parsed;
  };
  const deadlineRaw = value("--deadline");
  const args: BenchArgs = {
    keepIndex: argv.includes("--keep-index"),
    loc: number("--loc", 500_000),
    stub: argv.includes("--stub"),
    reportOnly: argv.includes("--report-only"),
  };
  const root = value("--root");
  const probe = value("--probe");
  const json = value("--json");
  return {
    ...args,
    ...(root !== undefined ? { root } : {}),
    ...(probe !== undefined ? { probe } : {}),
    ...(json !== undefined ? { json } : {}),
    ...(deadlineRaw !== undefined ? { deadlineS: number("--deadline", 0) } : {}),
  };
}

/** Absolute root; must be an existing directory. */
export function resolveRoot(root: string): string {
  const absolute = path.resolve(root);
  let isDir = false;
  try {
    isDir = statSync(absolute).isDirectory();
  } catch {
    isDir = false;
  }
  if (!isDir) throw new BenchUsageError(`--root ${root} is not an existing directory`);
  return absolute;
}

export function indexDirFor(root: string): string {
  return path.join(root, ".workbench");
}

/**
 * A warm index is not a cold build, and the bench must never delete state it
 * did not create: a pre-existing `.workbench/` under the root is refused
 * unless `--keep-index` says the caller knows.
 */
export function guardIndexDir(root: string, keepIndex: boolean): void {
  if (existsSync(indexDirFor(root)) && !keepIndex) {
    throw new BenchUsageError(
      `${indexDirFor(root)} already exists — a warm index is not a cold build, and the bench ` +
        "will not delete state it did not create. Remove it, or pass --keep-index to measure anyway.",
    );
  }
}

export function removeIndexDir(root: string): void {
  rmSync(indexDirFor(root), { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}

export interface Probe {
  /** Root-relative, forward-slashed. */
  readonly path: string;
  readonly pattern: string;
}

/** `<relpath>:<pattern>`, split at the LAST colon. */
export function parseProbe(spec: string): Probe {
  const at = spec.lastIndexOf(":");
  if (at <= 0 || at === spec.length - 1) {
    throw new BenchUsageError(`--probe must be <relpath>:<pattern>, got ${spec}`);
  }
  return { path: spec.slice(0, at).replaceAll("\\", "/"), pattern: spec.slice(at + 1) };
}

const PROBE_CANDIDATES = ["Cargo.toml", "README.md", "package.json"] as const;
const PROBE_TOKEN = /[A-Za-z_][A-Za-z0-9_]{2,}/;

/**
 * The first existing candidate file and the first identifier-like token of
 * its first non-empty line — reproducible run to run, and a real lexical hit
 * before any index exists.
 */
export function defaultProbe(root: string): Probe {
  for (const candidate of PROBE_CANDIDATES) {
    const file = path.join(root, candidate);
    if (!existsSync(file)) continue;
    const text = readFileSync(file, "utf8");
    for (const line of text.split("\n")) {
      const token = PROBE_TOKEN.exec(line);
      if (token !== null) return { path: candidate, pattern: token[0] };
    }
  }
  throw new BenchUsageError(
    `no default tier-0 probe under the root (none of ${PROBE_CANDIDATES.join(", ")} has an identifier); pass --probe <relpath>:<pattern>`,
  );
}

export interface LanguageCount {
  readonly language: string;
  readonly files: number;
  readonly loc: number;
}

export interface LocCount {
  /** Newlines over rows with a language and junk = 0. */
  readonly loc: number;
  /** Every row of the files table. */
  readonly files: number;
  readonly sourceFiles: number;
  readonly junkFiles: number;
  /** Sorted by loc descending, then language. */
  readonly languages: readonly LanguageCount[];
}

const COUNT_BUFFER = new Uint8Array(1 << 20);

/** Newlines in a file, streamed — a 200 MB test fixture must not be slurped. */
export function countNewlines(file: string): number {
  const fd = openSync(file, "r");
  try {
    let count = 0;
    for (;;) {
      const read = readSync(fd, COUNT_BUFFER, 0, COUNT_BUFFER.length, null);
      if (read <= 0) break;
      for (let i = 0; i < read; i += 1) if (COUNT_BUFFER[i] === 0x0a) count += 1;
    }
    return count;
  } finally {
    closeSync(fd);
  }
}

/**
 * LOC over exactly what the index covered: the files table after the build
 * settled, source rows only (a language, not junk). A row whose file has
 * vanished since is skipped.
 */
export function countLoc(db: Database, root: string): LocCount {
  const rows = db.query("SELECT path, language, junk FROM files ORDER BY path").all() as readonly {
    path: string;
    language: string | null;
    junk: number;
  }[];
  const perLanguage = new Map<string, { files: number; loc: number }>();
  let loc = 0;
  let sourceFiles = 0;
  let junkFiles = 0;
  for (const row of rows) {
    if (row.junk !== 0) junkFiles += 1;
    if (row.language === null || row.junk !== 0) continue;
    let lines: number;
    try {
      lines = countNewlines(path.join(root, row.path));
    } catch {
      continue;
    }
    sourceFiles += 1;
    loc += lines;
    const entry = perLanguage.get(row.language) ?? { files: 0, loc: 0 };
    entry.files += 1;
    entry.loc += lines;
    perLanguage.set(row.language, entry);
  }
  const languages = [...perLanguage.entries()]
    .map(([language, entry]) => ({ language, files: entry.files, loc: entry.loc }))
    .sort((a, b) => b.loc - a.loc || a.language.localeCompare(b.language));
  return { loc, files: rows.length, sourceFiles, junkFiles, languages };
}

export interface ReferenceQuery {
  readonly role: "hot" | "rare" | "miss";
  readonly name: string;
  /** Rows in the symbols table for the name (0 for the miss). */
  readonly definitions: number;
}

/**
 * Hot = the most frequent names of at least MIN_HOT_NAME_LENGTH characters
 * (the 200-definition and 200-file caps in play); rare = single-definition
 * names at the quartiles of the sorted list (the common case); miss = the
 * floor. A pure function of the symbols table, so the same checkout samples
 * the same queries.
 */
export function sampleReferenceQueries(db: Database): ReferenceQuery[] {
  const hot = db
    .query(
      "SELECT name, count(*) AS n FROM symbols WHERE length(name) >= ?1 " +
        "GROUP BY name ORDER BY n DESC, name LIMIT ?2",
    )
    .all(MIN_HOT_NAME_LENGTH, SAMPLES_PER_ROLE) as readonly { name: string; n: number }[];
  const singles = db
    .query("SELECT name FROM symbols GROUP BY name HAVING count(*) = 1 ORDER BY name")
    .all() as readonly { name: string }[];
  const rare: ReferenceQuery[] = [];
  if (singles.length > 0) {
    const seen = new Set<string>();
    for (const quartile of [1, 2, 3]) {
      const row = singles[Math.floor((singles.length * quartile) / 4)] ?? singles[0];
      if (row === undefined || seen.has(row.name)) continue;
      seen.add(row.name);
      rare.push({ role: "rare", name: row.name, definitions: 1 });
    }
  }
  return [
    ...hot.map((row): ReferenceQuery => ({ role: "hot", name: row.name, definitions: row.n })),
    ...rare,
    { role: "miss", name: MISS_NAME, definitions: 0 },
  ];
}

export interface Percentiles {
  readonly p50: number;
  readonly p95: number;
  readonly p99: number;
  readonly max: number;
}

/** Percentiles of a sample list (sorted here; an empty list answers zeros). */
export function percentiles(samples: readonly number[]): Percentiles {
  const sorted = [...samples].sort((a, b) => a - b);
  const at = (q: number): number =>
    sorted[Math.min(Math.floor(q * sorted.length), sorted.length - 1)] ?? 0;
  return { p50: at(0.5), p95: at(0.95), p99: at(0.99), max: sorted[sorted.length - 1] ?? 0 };
}

export interface Gate {
  readonly name: "tier0" | "tier1" | "references_p95" | "ipc_p95";
  /** The judged value (tier1: seconds normalized per 500k LOC). */
  readonly value: number;
  readonly limit: number;
  readonly unit: "s" | "ms";
  readonly pass: boolean;
  readonly note?: string;
}

const round = (n: number, digits: number): number => Number(n.toFixed(digits));

export function tier0Gate(tier0S: number): Gate {
  const value = round(tier0S, 3);
  return { name: "tier0", value, limit: TIER0_GATE_S, unit: "s", pass: value <= TIER0_GATE_S };
}

/**
 * The constitution's tier-1 gate is a throughput claim about 500k LOC; over a
 * larger corpus the honest comparison is the rate, and the note says so. A
 * build that never reached tier 1 fails whatever the clock says.
 */
export function tier1Gate(
  tier1S: number,
  loc: number,
  reached: boolean,
  progress?: { readonly filesIndexed: number; readonly filesTotal: number },
): Gate {
  if (!reached) {
    const where =
      progress === undefined ? "" : ` (${progress.filesIndexed}/${progress.filesTotal} files)`;
    return {
      name: "tier1",
      value: round(tier1S, 3),
      limit: TIER1_GATE_S,
      unit: "s",
      pass: false,
      note: `tier 1 not reached within ${round(tier1S, 1)}s${where}`,
    };
  }
  const normalized = loc > 0 ? (tier1S * TIER1_GATE_LOC) / loc : tier1S;
  const value = round(normalized, 3);
  return {
    name: "tier1",
    value,
    limit: TIER1_GATE_S,
    unit: "s",
    pass: value <= TIER1_GATE_S,
    note: `normalized per ${TIER1_GATE_LOC} LOC; measured ${round(tier1S, 2)}s over ${loc} LOC`,
  };
}

export function refsP95Gate(p95Ms: number, loc: number): Gate {
  const value = round(p95Ms, 3);
  return {
    name: "references_p95",
    value,
    limit: REFS_P95_GATE_MS,
    unit: "ms",
    pass: value <= REFS_P95_GATE_MS,
    note: `constitution states the gate at 1M LOC; measured over ${loc} LOC`,
  };
}

export function ipcP95Gate(p95Ms: number): Gate {
  const value = round(p95Ms, 4);
  return {
    name: "ipc_p95",
    value,
    limit: IPC_P95_GATE_MS,
    unit: "ms",
    pass: value <= IPC_P95_GATE_MS,
  };
}

export interface RepoFacts {
  readonly url?: string;
  readonly commit: string;
}

/** `git -C <root> rev-parse HEAD` and the origin URL; absent when not a checkout. */
export function gitFacts(root: string): RepoFacts | undefined {
  const git = (...args: string[]): string | undefined => {
    try {
      return execFileSync("git", ["-C", root, ...args], {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
        timeout: 10_000,
      }).trim();
    } catch {
      return undefined;
    }
  };
  const commit = git("rev-parse", "HEAD");
  if (commit === undefined || commit === "") return undefined;
  const url = git("remote", "get-url", "origin");
  return url === undefined || url === "" ? { commit } : { url, commit };
}

export type CoreFlavorLabel = "native" | "wasm" | "stub" | "unknown";

export interface MachineFacts {
  readonly platform: string;
  readonly arch: string;
  readonly cpu: string;
  readonly cores: number;
  readonly memoryGb: number;
  readonly bun: string;
  readonly coreFlavor: CoreFlavorLabel;
  readonly workbenchCommit?: string;
}

export function machineFacts(coreFlavor: CoreFlavorLabel): MachineFacts {
  const workbenchRoot = path.resolve(import.meta.dirname, "..");
  const commit = gitFacts(workbenchRoot)?.commit;
  const cpu = cpus()[0]?.model.trim() ?? "unknown";
  return {
    platform: platform(),
    arch: arch(),
    cpu,
    cores: cpus().length,
    memoryGb: round(totalmem() / 1024 ** 3, 1),
    bun: Bun.version,
    coreFlavor,
    ...(commit !== undefined ? { workbenchCommit: commit } : {}),
  };
}

export interface CorpusFacts extends LocCount {
  readonly kind: "synthetic" | "root";
  readonly root?: string;
  readonly repo?: RepoFacts;
}

export interface BenchReport {
  readonly schema: typeof BENCH_SCHEMA;
  readonly script: "bench-index" | "bench-refs" | "bench-ipc";
  readonly profile: string;
  readonly recordedAt: string;
  readonly corpus?: CorpusFacts;
  readonly machine: MachineFacts;
  readonly measurements: Readonly<Record<string, unknown>>;
  readonly gates: readonly Gate[];
}

/** Synthetic profiles are named by their LOC target: 500000 → synthetic-500k. */
export function profileName(args: Pick<BenchArgs, "root" | "loc">): string {
  if (args.root !== undefined) return "root";
  const loc = args.loc;
  if (loc % 1_000_000 === 0) return `synthetic-${loc / 1_000_000}m`;
  if (loc % 1000 === 0) return `synthetic-${loc / 1000}k`;
  return `synthetic-${loc}`;
}

export function writeReport(file: string, report: BenchReport): void {
  mkdirSync(path.dirname(path.resolve(file)), { recursive: true });
  writeFileSync(file, `${JSON.stringify(report, null, 2)}\n`);
}

/** Console-side gate line, shared so the three benches read alike. */
export function describeGate(gate: Gate): string {
  const verdict = gate.pass ? "OK" : "FAIL";
  const note = gate.note === undefined ? "" : ` — ${gate.note}`;
  return `${verdict}: ${gate.name} ${gate.value}${gate.unit} (limit ${gate.limit}${gate.unit})${note}`;
}
