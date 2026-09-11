/**
 * The rustfmt formatter adapter (specs/012-rust-rustfmt-formatter,
 * chunk:rust-rustfmt-formatter):
 *
 *   SC-002 degradation with INJECTED fakes (no toolchain): a missing binary, a
 *          binary that exits non-zero, a binary that hangs — each applies the
 *          verified unformatted content through the write engine with a note
 *          naming the cause; never an error envelope;
 *   SC-001 (toolchain-gated) a .rs write applies rustfmt's output, a .ts file
 *          in the same batch is Biome-formatted, .md passes through,
 *          format:false restores exact bytes;
 *   SC-003 (toolchain-gated where real) the pinned channel's rustfmt is the one
 *          resolved, an uninstalled pin degrades with a note, the edition is
 *          read from [package] or [workspace.package], a rustfmt.toml option is
 *          reflected;
 *   SC-004 (toolchain-gated) cold and warm per-edit overhead recorded.
 *
 * The fakes are `bun -e <script>` so they run identically on every platform;
 * the adapter appends `--edition <e>` after the seam's args (`--` keeps bun from eating it).
 */

import { afterEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { BiomeFormatter } from "../src/adapters/biome";
import { createInProcessPorts } from "../src/adapters/inprocess";
import {
  editionFor,
  hasToolchainPin,
  RustfmtFormatter,
  resolveRustfmt,
  routingFormatter,
} from "../src/adapters/rustfmt";
import type { ResponseEnvelope } from "../src/envelope";
import { PORT_VERSIONS } from "../src/ports";
import {
  runWriteBatch,
  type WriteEngineDeps,
  type WriteReport,
  type WriteWorkspaceAccess,
} from "../src/tools/write-engine";

const roots: string[] = [];
function tempRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "wb-rustfmt-"));
  roots.push(root);
  return root;
}
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

/** A fake rustfmt: `bun -e <script>`; the adapter appends --edition <e>. */
const FAKE = {
  /** Echo stdin upper-cased (observable formatting), exit 0. */
  upper:
    "process.stdin.on('data', d => process.stdout.write(String(d).toUpperCase())); process.stdin.on('end', () => process.exit(0));",
  /** Refuse: one stderr line, exit 1. */
  refuse:
    "process.stdin.resume(); process.stdin.on('end', () => { process.stderr.write('error: expected one of `!` or `::`, found `(`\\n'); process.exit(1); });",
  /** Hang forever (the timeout must kill it). */
  hang: "process.stdin.resume(); setInterval(() => {}, 1000);",
  /** Print the edition argument it received, exit 0 (proves --edition is passed). */
  edition:
    "const i = process.argv.indexOf('--edition'); process.stdin.resume(); process.stdin.on('end', () => { process.stdout.write('edition=' + process.argv[i + 1] + '\\n'); process.exit(0); });",
};

function fake(root: string, script: string, extra: { timeoutMs?: number } = {}): RustfmtFormatter {
  return new RustfmtFormatter({
    root,
    binary: process.execPath,
    binaryArgs: ["-e", script, "--"],
    ...(extra.timeoutMs === undefined ? {} : { timeoutMs: extra.timeoutMs }),
  });
}

function accessOf(files: Map<string, string>): WriteWorkspaceAccess {
  return {
    read: (path) => {
      const content = files.get(path);
      if (content === undefined) {
        const error = new Error(`ENOENT: ${path}`) as NodeJS.ErrnoException;
        error.code = "ENOENT";
        return Promise.reject(error);
      }
      return Promise.resolve(content);
    },
    write: (path, content) => {
      files.set(path, content);
      return Promise.resolve();
    },
    remove: (path) => {
      files.delete(path);
      return Promise.resolve();
    },
  };
}

function makeDeps(formatter: WriteEngineDeps["formatter"]): {
  deps: WriteEngineDeps;
  files: Map<string, string>;
} {
  const ports = createInProcessPorts();
  const deps: WriteEngineDeps = {
    engine: ports.engine,
    formatter,
    journal: ports.journal,
    workspace: accessOf(ports.workspace.files),
  };
  return { deps, files: ports.workspace.files };
}

function ok(envelope: ResponseEnvelope<WriteReport>): readonly WriteReport[] {
  if (!envelope.ok) throw new Error(`expected ok envelope, got ${envelope.error.code}`);
  return envelope.results;
}

const RUST_SRC = "fn main(){let x=1;}\n";

// ---------------------------------------------------------------------------
// Pure pieces (no toolchain)

describe("editionFor (R3)", () => {
  const files = new Map<string, string>([
    [
      "Cargo.toml",
      '[workspace]\nmembers = ["crates/a", "crates/b"]\n\n[workspace.package]\nedition = "2024"\n',
    ],
    ["crates/a/Cargo.toml", '[package]\nname = "a"\nedition.workspace = true\n'],
    [
      "crates/b/Cargo.toml",
      '[package]\nname = "b"\nedition = "2018"\n\n[dependencies]\nx = { path = "../x" }\n',
    ],
    ["crates/c/Cargo.toml", '[package]\nname = "c"\n'],
  ]);
  const read = (rel: string) => files.get(rel) ?? null;

  test("own [package] edition wins; inheritance reads the root [workspace.package]; default 2021", () => {
    expect(editionFor("crates/b/src/lib.rs", read)).toBe("2018");
    expect(editionFor("crates/a/src/deep/mod.rs", read)).toBe("2024");
    expect(editionFor("crates/c/src/lib.rs", read)).toBe("2024"); // no edition ⇒ the workspace's
    expect(editionFor("src/lib.rs", read)).toBe("2024"); // a root file with no [package]: the workspace edition is the best answer
    expect(editionFor("loose.rs", () => null)).toBe("2021");
    expect(editionFor("x/y.rs", () => '[package]\nname = "odd"\nedition = "1999"\n')).toBe("2021");
  });
});

describe("routingFormatter (R1)", () => {
  test("routes by extension and falls back", async () => {
    const calls: string[] = [];
    const mk = (name: string) => ({
      version: PORT_VERSIONS.formatter,
      format: (r: { path: string; content: string }) => {
        calls.push(`${name}:${r.path}`);
        return Promise.resolve({ content: r.content, changed: false });
      },
    });
    const router = routingFormatter({
      rules: [{ extensions: ["rs"], formatter: mk("rust") }],
      fallback: mk("biome"),
    });
    expect(router.version).toBe(PORT_VERSIONS.formatter);
    await router.format({ path: "src/a.rs", content: "" });
    await router.format({ path: "src/A.RS", content: "" });
    await router.format({ path: "src/a.ts", content: "" });
    await router.format({ path: "README.md", content: "" });
    expect(calls).toEqual(["rust:src/a.rs", "rust:src/A.RS", "biome:src/a.ts", "biome:README.md"]);
  });
});

describe("the adapter alone with injected fakes (R5/R8)", () => {
  test("carries the port version; non-.rs passes through without spawning", async () => {
    const f = fake(tempRoot(), FAKE.refuse);
    expect(f.version).toBe(PORT_VERSIONS.formatter);
    expect(await f.format({ path: "a.ts", content: "x" })).toEqual({
      content: "x",
      changed: false,
    });
  });

  test("formats through stdin/stdout and appends --edition from the manifest", async () => {
    const root = tempRoot();
    writeFileSync(join(root, "Cargo.toml"), '[package]\nname = "p"\nedition = "2024"\n');
    mkdirSync(join(root, "src"));
    const f = fake(root, FAKE.upper);
    expect(await f.format({ path: "src/main.rs", content: RUST_SRC })).toEqual({
      content: RUST_SRC.toUpperCase(),
      changed: true,
    });
    const e = fake(root, FAKE.edition);
    expect((await e.format({ path: "src/main.rs", content: RUST_SRC })).content).toBe(
      "edition=2024\n",
    );
    expect(e.editionOf("src/main.rs")).toBe("2024");
  });

  test("unresolvable ⇒ throws the install remedy; resolution is memoized", async () => {
    let asked = 0;
    const f = new RustfmtFormatter({
      root: tempRoot(),
      resolver: () => {
        asked += 1;
        return null;
      },
    });
    await expect(f.format({ path: "a.rs", content: RUST_SRC })).rejects.toThrow(
      /rustup component add rustfmt/,
    );
    await expect(f.format({ path: "a.rs", content: RUST_SRC })).rejects.toThrow(/not installed/);
    expect(asked).toBe(1);
  });

  test("non-zero exit ⇒ throws with rustfmt's first stderr line; a hang is killed at the timeout", async () => {
    await expect(
      fake(tempRoot(), FAKE.refuse).format({ path: "a.rs", content: RUST_SRC }),
    ).rejects.toThrow(/rustfmt exited 1: error: expected one of/);
    const t0 = performance.now();
    await expect(
      fake(tempRoot(), FAKE.hang, { timeoutMs: 400 }).format({ path: "a.rs", content: RUST_SRC }),
    ).rejects.toThrow(/killed by SIGTERM after 400ms/);
    expect(performance.now() - t0).toBeLessThan(5_000);
  }, 15_000);
});

// ---------------------------------------------------------------------------
// Through the write engine (SC-002 with fakes; SC-001 with the real binary)

describe("through the write engine: degradation is a note, never an error (SC-002)", () => {
  for (const [name, script, pattern] of [
    [
      "exits non-zero",
      FAKE.refuse,
      /the formatter failed \(rustfmt exited 1: error: expected one of/,
    ],
    ["hangs", FAKE.hang, /the formatter failed \(rustfmt was killed by SIGTERM after 300ms\)/],
  ] as const) {
    test(`a rustfmt that ${name}: the verified unformatted content is applied with the cause in the note`, async () => {
      const root = tempRoot();
      const { deps, files } = makeDeps(
        routingFormatter({
          rules: [{ extensions: ["rs"], formatter: fake(root, script, { timeoutMs: 300 }) }],
          fallback: new BiomeFormatter({ root }),
        }),
      );
      const envelope = await runWriteBatch(
        deps,
        {
          writes: [{ path: "src/main.rs", content: RUST_SRC }],
          dryRun: false,
          budget: 4000,
          origin: { tool: "edit", argsHash: "t", sessionId: "s" },
        },
        1,
      );
      const results = ok(envelope);
      expect(results[0]?.applied).toBe(true);
      expect(files.get("src/main.rs")).toBe(RUST_SRC);
      expect(envelope.notes?.some((n) => pattern.test(n))).toBe(true);
    }, 15_000);
  }

  test("a missing rustfmt: applied unformatted, the note names the install remedy", async () => {
    const root = tempRoot();
    const { deps, files } = makeDeps(
      routingFormatter({
        rules: [
          { extensions: ["rs"], formatter: new RustfmtFormatter({ root, resolver: () => null }) },
        ],
        fallback: new BiomeFormatter({ root }),
      }),
    );
    const envelope = await runWriteBatch(
      deps,
      {
        writes: [{ path: "src/main.rs", content: RUST_SRC }],
        dryRun: false,
        budget: 4000,
        origin: { tool: "edit", argsHash: "t", sessionId: "s" },
      },
      1,
    );
    ok(envelope);
    expect(files.get("src/main.rs")).toBe(RUST_SRC);
    expect(
      envelope.notes?.some((n) =>
        n.includes(
          "the formatter failed (rustfmt is not installed for this workspace's toolchain (rustup component add rustfmt)); applied the verified unformatted content",
        ),
      ),
    ).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// The real binary (toolchain-gated)

const REAL = resolveRustfmt(process.cwd());

describe("with the workspace's real rustfmt (toolchain-gated)", () => {
  test.skipIf(REAL === null)(
    "resolution honors the repo's rust-toolchain.toml pin and the binary runs",
    () => {
      expect(hasToolchainPin(process.cwd())).toBe(true);
      expect(REAL).not.toBeNull();
      // The pinned channel (rust-toolchain.toml at the repo root) names the binary's toolchain dir.
      expect(REAL as string).toContain("1.95.0");
    },
  );

  test("an UNINSTALLED pin resolves to nothing — never some other toolchain's rustfmt (R2)", () => {
    const root = tempRoot();
    writeFileSync(
      join(root, "rust-toolchain.toml"),
      '[toolchain]\nchannel = "1.0.0-no-such-channel"\n',
    );
    expect(hasToolchainPin(root)).toBe(true);
    expect(resolveRustfmt(root)).toBeNull();
  });

  test.skipIf(REAL === null)(
    "a .rs write applies rustfmt's output; .ts is Biome-formatted; .md passes; format:false bypasses (SC-001)",
    async () => {
      const root = tempRoot();
      writeFileSync(
        join(root, "Cargo.toml"),
        '[package]\nname = "p"\nversion = "0.1.0"\nedition = "2021"\n',
      );
      const { deps, files } = makeDeps(
        routingFormatter({
          rules: [{ extensions: ["rs"], formatter: new RustfmtFormatter({ root }) }],
          fallback: new BiomeFormatter({ root }),
        }),
      );
      const origin = { tool: "edit", argsHash: "t", sessionId: "s" };
      const envelope = await runWriteBatch(
        deps,
        {
          writes: [
            { path: "src/main.rs", content: RUST_SRC },
            { path: "src/x.ts", content: "const x=1\nfunction  f( ){return x}" },
            { path: "notes.md", content: "#  hi \n" },
          ],
          dryRun: false,
          budget: 4000,
          origin,
        },
        1,
      );
      ok(envelope);
      expect(files.get("src/main.rs")).toBe("fn main() {\n    let x = 1;\n}\n");
      expect(files.get("src/x.ts")).toContain("const x = 1;");
      expect(files.get("notes.md")).toBe("#  hi \n");
      // format:false (undo's flag): exact bytes, rustfmt not consulted.
      ok(
        await runWriteBatch(
          deps,
          {
            writes: [{ path: "src/main.rs", content: RUST_SRC }],
            dryRun: false,
            budget: 4000,
            origin,
            format: false,
          },
          1,
        ),
      );
      expect(files.get("src/main.rs")).toBe(RUST_SRC);
    },
    30_000,
  );

  test.skipIf(REAL === null)(
    "edition is honored (2024-only syntax formats under a 2024 manifest) and rustfmt.toml is discovered (SC-003)",
    async () => {
      const root = tempRoot();
      mkdirSync(join(root, "src"));
      const letChains =
        "fn f(a: Option<u32>) -> u32 { if let Some(x) = a && x > 1 { x } else { 0 } }\n";
      writeFileSync(
        join(root, "Cargo.toml"),
        '[workspace]\nmembers = []\n\n[workspace.package]\nedition = "2024"\n\n[package]\nname = "p"\nedition.workspace = true\n',
      );
      const f = new RustfmtFormatter({ root });
      expect(f.editionOf("src/lib.rs")).toBe("2024");
      const formatted = await f.format({ path: "src/lib.rs", content: letChains });
      expect(formatted.changed).toBe(true);
      expect(formatted.content).toContain("if let Some(x) = a\n        && x > 1");
      // The same source under a 2021 manifest is rejected by rustfmt — the edition is load-bearing.
      writeFileSync(join(root, "Cargo.toml"), '[package]\nname = "p"\nedition = "2021"\n');
      const g = new RustfmtFormatter({ root });
      await expect(g.format({ path: "src/lib.rs", content: letChains })).rejects.toThrow(
        /rustfmt exited 1/,
      );
      // rustfmt.toml from the file's directory upward.
      writeFileSync(join(root, "rustfmt.toml"), "max_width = 40\n");
      const h = new RustfmtFormatter({ root });
      const wide =
        "fn long_function_name(alpha: u32, beta: u32, gamma: u32) -> u32 { alpha + beta + gamma }\n";
      const out = await h.format({ path: "src/lib.rs", content: wide });
      expect(out.content.split("\n").every((l) => l.length <= 40)).toBe(true);
      expect(out.content).toContain("fn long_function_name(\n");
    },
    30_000,
  );

  test.skipIf(REAL === null)(
    "per-edit overhead recorded: cold (resolution + spawn) and warm (SC-004)",
    async () => {
      const root = tempRoot();
      writeFileSync(join(root, "Cargo.toml"), '[package]\nname = "p"\nedition = "2021"\n');
      mkdirSync(join(root, "src"));
      const body = Array.from({ length: 20 }, (_, i) => `pub fn f${i}(a:u32)->u32{a+${i}}`).join(
        "\n",
      );
      const f = new RustfmtFormatter({ root });
      const t0 = performance.now();
      const first = await f.format({ path: "src/lib.rs", content: `${body}\n` });
      const cold = Math.round(performance.now() - t0);
      const t1 = performance.now();
      const second = await f.format({ path: "src/lib.rs", content: first.content });
      const warm = Math.round(performance.now() - t1);
      expect(first.changed).toBe(true);
      expect(second.changed).toBe(false);
      console.log(
        `[012 SC-004] rustfmt per-edit overhead: cold=${cold}ms warm=${warm}ms (20-fn file)`,
      );
    },
    30_000,
  );

  test.skipIf(REAL !== null)(
    "real-rustfmt chapter skipped: needs a runnable rustfmt for this workspace's toolchain (specs/012 R8)",
    () => {
      expect(REAL).toBeNull();
    },
  );
});
