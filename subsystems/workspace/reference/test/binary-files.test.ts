import { afterEach, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInProcessPorts } from "../src/adapters/inprocess";
import { JournalDb } from "../src/adapters/journal-db";
import { contentHash } from "../src/envelope";
import { checkConfirmGrant, KEY_LENGTH, mintConfirmGrant } from "../src/jail";
import { createWriteAccess } from "../src/protocol/write-access";
import { type FilesDeps, runFiles } from "../src/tools/files";
import { runUndo } from "../src/tools/undo";
import { runWriteBatch } from "../src/tools/write-engine";

let root: string | undefined;
let journal: JournalDb | undefined;
afterEach(() => {
  journal?.close();
  journal = undefined;
  if (root) rmSync(root, { recursive: true, force: true });
  root = undefined;
});

function setup() {
  root = mkdtempSync(join(tmpdir(), "workbench-binary-"));
  const ports = createInProcessPorts();
  const dbPath = join(root, "journal.db");
  journal = new JournalDb({ path: dbPath });
  const key = Buffer.alloc(KEY_LENGTH, 7);
  const deps: FilesDeps = {
    write: {
      engine: ports.engine,
      formatter: ports.formatter,
      journal,
      workspace: createWriteAccess({ root, stateDir: join(root, ".workbench") }),
    },
    confirm: {
      mint: (diffHash, contentHash) => mintConfirmGrant(key, diffHash, contentHash),
      check: (token) => checkConfirmGrant(key, token),
    },
    list: async () => [],
  };
  return { root, dbPath, deps };
}

// PNG signature, NUL, invalid UTF-8 and every byte value: decoding/re-encoding
// must never be part of file relocation or journal restoration.
const png = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  Buffer.from(Array.from({ length: 256 }, (_, i) => i)),
]);

test("binary overwrite is exact, preview is compact, and undo survives reopening the journal", async () => {
  const rig = setup();
  const original = Buffer.from([0, 255, 254, 253, 10]);
  writeFileSync(join(rig.root, "source.png"), png);
  writeFileSync(join(rig.root, "target.png"), original);
  const input = { op: "copy" as const, from: "source.png", to: "target.png" };
  const preview = await runFiles(rig.deps, { ...input, dryRun: true }, 1, "binary");
  expect(preview.ok).toBe(true);
  expect(JSON.stringify(preview)).toContain("Binary files");
  expect(readFileSync(join(rig.root, "target.png"))).toEqual(original);
  const gated = await runFiles(rig.deps, input, 1, "binary");
  expect(gated.ok).toBe(false);
  if (!gated.ok) expect(gated.error.code).toBe("CONFIRM_REQUIRED");
  const copied = await runFiles(rig.deps, { ...input, autoApply: true }, 1, "binary");
  expect(copied.ok).toBe(true);
  expect(readFileSync(join(rig.root, "target.png"))).toEqual(png);
  expect(JSON.stringify(copied).length).toBeLessThan(2000);
  journal?.close();
  journal = new JournalDb({ path: rig.dbPath });
  const restored = await runUndo({ ...rig.deps.write, journal }, {}, 1, "binary");
  expect(restored.ok).toBe(true);
  expect(readFileSync(join(rig.root, "target.png"))).toEqual(original);
});

test("binary move and delete are byte-exact through multi-step undo", async () => {
  const rig = setup();
  writeFileSync(join(rig.root, "source.png"), png);
  expect(
    (
      await runFiles(
        rig.deps,
        { op: "move", from: "source.png", to: "moved.png", autoApply: true },
        1,
        "binary",
      )
    ).ok,
  ).toBe(true);
  expect(readFileSync(join(rig.root, "moved.png"))).toEqual(png);
  expect(
    (await runFiles(rig.deps, { op: "delete", from: "moved.png", autoApply: true }, 1, "binary"))
      .ok,
  ).toBe(true);
  expect((await runUndo(rig.deps.write, { steps: 2 }, 1, "binary")).ok).toBe(true);
  expect(readFileSync(join(rig.root, "source.png"))).toEqual(png);
});

test("text access refuses invalid UTF-8 rather than replacing bytes", async () => {
  const rig = setup();
  writeFileSync(join(rig.root, "source.png"), png);
  await expect(rig.deps.write.workspace.read("source.png")).rejects.toMatchObject({
    code: "PATTERN_INVALID",
  });
});

test("binary operations skip parsers, formatters and importer scans regardless of extension", async () => {
  const rig = setup();
  writeFileSync(join(rig.root, "opaque.rs"), png);
  const unexpected = async (): Promise<never> => {
    throw new Error("binary reached a text operation");
  };
  const deps: FilesDeps = {
    ...rig.deps,
    list: unexpected,
    write: {
      ...rig.deps.write,
      engine: { ...rig.deps.write.engine, parse: unexpected },
      formatter: { ...rig.deps.write.formatter, format: unexpected },
    },
  };
  expect(
    (
      await runFiles(
        deps,
        { op: "move", from: "opaque.rs", to: "opaque.json", autoApply: true },
        1,
        "binary",
      )
    ).ok,
  ).toBe(true);
  expect(readFileSync(join(rig.root, "opaque.json"))).toEqual(png);
});

test("binary hashes anchor writes, confirmations and undo conflicts; identical copy is a no-op", async () => {
  const rig = setup();
  writeFileSync(join(rig.root, "source.png"), png);
  writeFileSync(join(rig.root, "target.png"), png);
  const input = { op: "copy" as const, from: "source.png", to: "target.png" };
  const noop = await runFiles(rig.deps, input, 1, "binary");
  expect(noop.ok && noop.results.length).toBe(0);
  expect(await rig.deps.write.journal.list(0)).toHaveLength(0);
  const stale = await runFiles(rig.deps, { ...input, expectedHash: "outdated" }, 1, "binary");
  expect(!stale.ok && stale.error.code).toBe("STALE_ANCHOR");

  writeFileSync(join(rig.root, "target.png"), Buffer.from([0, 128]));
  const gated = await runFiles(rig.deps, input, 1, "binary");
  if (gated.ok || gated.error.code !== "CONFIRM_REQUIRED") throw new Error("missing confirm gate");
  if (!("confirmToken" in gated.error) || typeof gated.error.confirmToken !== "string")
    throw new Error("missing confirmation token");
  const confirm = gated.error.confirmToken;
  writeFileSync(join(rig.root, "target.png"), Buffer.from([0, 129]));
  const expired = await runFiles(rig.deps, { ...input, confirm }, 1, "binary");
  expect(!expired.ok && expired.error.code).toBe("STALE_ANCHOR");
  const applied = await runFiles(
    rig.deps,
    { ...input, autoApply: true, expectedHash: contentHash(png) },
    1,
    "binary",
  );
  expect(applied.ok && applied.results[0]?.contentHash).toBe(contentHash(png));
  writeFileSync(join(rig.root, "target.png"), Buffer.from([0, 130]));
  const conflict = await runUndo(rig.deps.write, {}, 1, "binary");
  expect(conflict.ok && conflict.results[0]?.kind).toBe("conflict");
  expect(readFileSync(join(rig.root, "target.png"))).toEqual(Buffer.from([0, 130]));
});

test.each([true, false])(
  "undo restores text/binary overwrite in either direction (binary source: %s)",
  async (binarySource) => {
    const rig = setup();
    const before = binarySource ? Buffer.from("old text\n") : png;
    const after = binarySource ? png : Buffer.from("new text\n");
    writeFileSync(join(rig.root, "source.dat"), after);
    writeFileSync(join(rig.root, "target.dat"), before);
    expect(
      (
        await runFiles(
          rig.deps,
          { op: "copy", from: "source.dat", to: "target.dat", autoApply: true },
          1,
          "binary",
        )
      ).ok,
    ).toBe(true);
    expect(readFileSync(join(rig.root, "target.dat"))).toEqual(after);
    expect((await runUndo(rig.deps.write, {}, 1, "binary")).ok).toBe(true);
    expect(readFileSync(join(rig.root, "target.dat"))).toEqual(before);
  },
);

test("failed multi-file swap compensates with the exact binary before-content", async () => {
  const rig = setup();
  writeFileSync(join(rig.root, "target.png"), png);
  const workspace = rig.deps.write.workspace;
  const failing = {
    ...workspace,
    writeContent: async (path: string, content: string | Uint8Array) => {
      if (path === "fail.png") throw new Error("injected swap failure");
      if (!workspace.writeContent) throw new Error("missing byte access");
      await workspace.writeContent(path, content);
    },
  };
  await expect(
    runWriteBatch(
      { ...rig.deps.write, workspace: failing },
      {
        writes: [
          { path: "target.png", content: Buffer.from([0, 128]) },
          { path: "fail.png", content: png },
        ],
      },
      1,
    ),
  ).rejects.toThrow("injected swap failure");
  expect(readFileSync(join(rig.root, "target.png"))).toEqual(png);
});

test("journal listing reports binary metadata without claiming a parse or leaking payload", async () => {
  const rig = setup();
  writeFileSync(join(rig.root, "source.png"), png);
  await runFiles(rig.deps, { op: "copy", from: "source.png", to: "target.png" }, 1, "binary");
  const listed = await runUndo(rig.deps.write, { action: "list" }, 1, "binary");
  expect(listed.ok).toBe(true);
  if (!listed.ok) return;
  const row = listed.results[0];
  expect(row?.kind).toBe("op");
  if (row?.kind !== "op") return;
  expect(row.binary).toBe(true);
  expect(row.parseOk).toBeUndefined();
  expect(row.diff).toContain("Binary files");
  expect(row.diff?.length).toBeLessThan(400);
});
