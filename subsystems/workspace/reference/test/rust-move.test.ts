/**
 * Rust-aware files.move (specs/010-rust-move-aware-files) — chunk:rust-move-aware-files:
 *
 *   SC-001 rename in place rewrites the declaration and every crate::/super::/
 *          self::/uniform path; cargo check green (toolchain-gated);
 *   SC-002 move under another parent removes/adds the declaration, re-roots the
 *          moved file's super::, rewrites importers absolute where needed;
 *          missing parent => removal + note, nothing synthesized; cargo check green;
 *   SC-003 Cargo.toml target path entries follow the file; unaffected manifest
 *          absent; [workspace] members untouched (verification);
 *   SC-004 CONFIRM_REQUIRED with the aggregated diff; dryRun applies nothing;
 *   SC-005 every negative shape yields its specific note.
 *
 * The pure module is pinned over in-memory files; the rig tests run the real
 * files tool over the in-process ports; the compile proofs run the real
 * composed write access over a temp crate and ask cargo (skipped with a
 * reason when cargo is absent — the 008 pattern).
 */

import { afterEach, describe, expect, test } from "bun:test";
import {
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative } from "node:path";
import { createInProcessPorts, type InProcessPorts } from "../src/adapters/inprocess";
import type { ResponseEnvelope } from "../src/envelope";
import { checkConfirmGrant, KEY_LENGTH, mintConfirmGrant } from "../src/jail";
import { createWriteAccess } from "../src/protocol/write-access";
import { type FilesDeps, type FilesInput, runFiles } from "../src/tools/files";
import {
  buildModuleTree,
  crateDirOf,
  declaredModules,
  findDeclaration,
  insertDeclaration,
  locateModule,
  manifestTargetPaths,
  planRustMove,
  removeDeclaration,
  renameDeclaration,
  rewriteManifestPaths,
  rewritePaths,
} from "../src/tools/rust-move";
import type { WriteEngineDeps, WriteReport } from "../src/tools/write-engine";

// ---------------------------------------------------------------------------
// Fixture crate (research R10): one leaf module referenced every way.

const FIXTURE: Record<string, string> = {
  "Cargo.toml": '[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2021"\n',
  "src/lib.rs":
    "//! Fixture crate.\n\npub mod a;\npub mod c;\npub mod util;\n\npub use a::b::helper as root_helper;\n\n" +
    "pub fn run() -> u32 {\n    crate::a::b::helper() + a::b::helper() + c::from_c()\n}\n",
  "src/a.rs":
    "pub mod b;\npub mod other;\n\npub fn sibling() -> u32 {\n    1\n}\n\n" +
    "pub fn via_self() -> u32 {\n    self::b::helper()\n}\n\n" +
    "pub fn via_uniform() -> u32 {\n    b::helper()\n}\n",
  "src/a/b.rs":
    "use super::sibling;\n\n/// Helper.\npub fn helper() -> u32 {\n    crate::util::one() + sibling() + super::sibling()\n}\n",
  "src/a/other.rs": "pub fn twice() -> u32 {\n    super::b::helper() * 2\n}\n",
  "src/util.rs": "pub fn one() -> u32 {\n    1\n}\n",
  "src/c.rs": "pub mod d;\n\npub fn from_c() -> u32 {\n    crate::a::b::helper() + d::dd()\n}\n",
  "src/c/d.rs": "pub fn dd() -> u32 {\n    super::super::a::b::helper()\n}\n",
};

function listing(files: ReadonlyMap<string, string> | Record<string, string>) {
  const paths = files instanceof Map ? [...files.keys()] : Object.keys(files);
  return paths.map((path) => ({ path, junk: path.startsWith("target/") }));
}

function planOver(seed: Record<string, string>, from: string, to: string) {
  const files = new Map(Object.entries(seed));
  return planRustMove({
    from,
    to,
    fromContent: files.get(from) as string,
    files: listing(files),
    read: (path) => Promise.resolve(files.get(path) ?? null),
  });
}

function contentOf(plan: Awaited<ReturnType<typeof planOver>>, path: string): string | undefined {
  return plan.writes.find((w) => w.path === path)?.content;
}

// ---------------------------------------------------------------------------
// Pure rules

describe("module tree derivation (R1)", () => {
  const tree = buildModuleTree(
    new Set([
      "src/lib.rs",
      "src/main.rs",
      "src/a.rs",
      "src/a/b.rs",
      "src/a/b/c.rs",
      "src/m/mod.rs",
      "src/m/x.rs",
      "src/bin/tool.rs",
      "src/bin/big/main.rs",
      "src/bin/big/part.rs",
      "tests/it.rs",
      "tests/it/helper.rs",
      "tests/suite/main.rs",
      "tests/suite/common.rs",
      "build.rs",
      "src/orphan/deep.rs",
      "src/tools/cli.rs",
    ]),
    new Set(["src/tools/cli.rs"]),
  );

  test("crateDirOf picks the nearest Cargo.toml, else the workspace root", () => {
    const files = new Set([
      "Cargo.toml",
      "crates/x/Cargo.toml",
      "crates/x/src/lib.rs",
      "other/y.rs",
    ]);
    expect(crateDirOf("crates/x/src/lib.rs", files)).toBe("crates/x");
    expect(crateDirOf("other/y.rs", files)).toBe("");
    expect(crateDirOf("lonely.rs", new Set(["lonely.rs"]))).toBe("");
  });

  test("lib.rs and main.rs share one src/ tree; <name>.rs and <name>/mod.rs are modules", () => {
    expect(locateModule("src/a/b.rs", tree)).toEqual({
      roots: ["src/lib.rs", "src/main.rs"],
      rootDir: "src",
      segments: ["a", "b"],
      parents: ["src/a.rs"],
    });
    expect(locateModule("src/a/b/c.rs", tree)?.segments).toEqual(["a", "b", "c"]);
    expect(locateModule("src/a/b/c.rs", tree)?.parents).toEqual(["src/a/b.rs"]);
    expect(locateModule("src/m/mod.rs", tree)).toMatchObject({
      segments: ["m"],
      parents: ["src/lib.rs", "src/main.rs"],
    });
    expect(locateModule("src/m/x.rs", tree)).toMatchObject({
      segments: ["m", "x"],
      parents: ["src/m/mod.rs"],
    });
    expect(locateModule("src/lib.rs", tree)).toMatchObject({ segments: [], parents: [] });
  });

  test("targets are their own roots: src/bin, tests, manifest paths, build.rs", () => {
    expect(locateModule("src/bin/tool.rs", tree)?.segments).toEqual([]);
    expect(locateModule("src/bin/big/part.rs", tree)).toMatchObject({
      roots: ["src/bin/big/main.rs"],
      segments: ["part"],
    });
    expect(locateModule("tests/it/helper.rs", tree)).toMatchObject({
      roots: ["tests/it.rs"],
      segments: ["helper"],
    });
    expect(locateModule("tests/suite/common.rs", tree)).toMatchObject({
      roots: ["tests/suite/main.rs"],
      segments: ["common"],
    });
    expect(locateModule("build.rs", tree)?.segments).toEqual([]);
    expect(locateModule("src/tools/cli.rs", tree)?.segments).toEqual([]);
  });

  test("an orphan with no chain to a root is null", () => {
    expect(locateModule("src/orphan/deep.rs", tree)).toBeNull();
    expect(locateModule("README.md", tree)).toBeNull();
  });
});

describe("declarations (R1/R5)", () => {
  const parent =
    "//! docs\n#![allow(dead_code)]\n\n/// The b module.\n#[cfg(test)]\npub(crate) mod b; // trailing\nmod other;\nmod inline { }\n";

  test("declaredModules sees file and inline modules", () => {
    expect([...declaredModules(parent)].sort()).toEqual(["b", "inline", "other"]);
  });

  test("findDeclaration spans attributes and docs, reports pub and #[path]", () => {
    expect(findDeclaration(parent, "b")).toEqual({
      startLine: 3,
      declLine: 5,
      hasPathAttr: false,
      isPub: true,
    });
    expect(findDeclaration(parent, "other")).toEqual({
      startLine: 6,
      declLine: 6,
      hasPathAttr: false,
      isPub: false,
    });
    expect(findDeclaration(parent, "inline")).toBeNull();
    expect(findDeclaration(parent, "missing")).toBeNull();
    expect(findDeclaration('#[path = "x.rs"]\nmod b;\n', "b")?.hasPathAttr).toBe(true);
  });

  test("renameDeclaration keeps visibility, attributes and the trailing comment", () => {
    expect(renameDeclaration(parent, "b", "bee")).toContain(
      "#[cfg(test)]\npub(crate) mod bee; // trailing\n",
    );
    // Segment boundaries: `mod bb;` is not `mod b;`.
    expect(renameDeclaration("mod bb;\nmod b;\n", "b", "c")).toBe("mod bb;\nmod c;\n");
  });

  test("removeDeclaration drops the whole item; insertDeclaration lands after the last mod or after the inner block", () => {
    const item = findDeclaration(parent, "b");
    if (item === null) throw new Error("decl");
    expect(removeDeclaration(parent, item)).toBe(
      "//! docs\n#![allow(dead_code)]\n\nmod other;\nmod inline { }\n",
    );
    expect(insertDeclaration("mod x;\nmod y;\n\nfn f() {}\n", "pub mod b;")).toBe(
      "mod x;\nmod y;\npub mod b;\n\nfn f() {}\n",
    );
    expect(insertDeclaration("//! inner\n#![no_std]\nfn f() {}\n", "pub mod b;")).toBe(
      "//! inner\n#![no_std]\n\npub mod b;\n\nfn f() {}\n",
    );
    expect(insertDeclaration("fn f() {}\n", "mod b;")).toBe("mod b;\n\nfn f() {}\n");
  });
});

describe("path rewriting (R2/R3/R4)", () => {
  const ctx = (over: Partial<Parameters<typeof rewritePaths>[1]>) => ({
    resolveModule: ["a"] as readonly string[],
    emitModule: ["a"] as readonly string[],
    declaredBefore: new Set(["b"]),
    declaredAfter: new Set(["bee"]),
    oldModule: ["a", "b"] as readonly string[],
    newModule: ["a", "bee"] as readonly string[],
    ...over,
  });

  test("rename in place: crate::, self::, super:: and uniform paths keep their style", () => {
    const m = ctx({});
    const same = ctx({ resolveModule: m.resolveModule, emitModule: m.resolveModule });
    expect(rewritePaths("crate::a::b::helper()", same).content).toBe("crate::a::bee::helper()");
    expect(rewritePaths("self::b::helper()", same).content).toBe("self::bee::helper()");
    expect(rewritePaths("b::helper()", same).content).toBe("bee::helper()");
    const sibling = ctx({
      resolveModule: ["a", "other"],
      declaredBefore: new Set(),
      declaredAfter: new Set(),
    });
    const siblingSame = { ...sibling, emitModule: sibling.resolveModule };
    expect(rewritePaths("super::b::helper()", siblingSame).content).toBe("super::bee::helper()");
    expect(rewritePaths("use crate::a::b::*;\npub use crate::a::b;", same).content).toBe(
      "use crate::a::bee::*;\npub use crate::a::bee;",
    );
  });

  test("segment boundaries: a shared prefix is not the module", () => {
    const m = ctx({});
    const same = { ...m, emitModule: m.resolveModule };
    const out = rewritePaths("crate::a::bb::x; crate::ab::b::y; crate::a::b_x::z", same);
    expect(out.count).toBe(0);
    expect(out.content).toBe("crate::a::bb::x; crate::ab::b::y; crate::a::b_x::z");
  });

  test("directory move: relative forms that no longer reach become crate::-absolute", () => {
    const m = ctx({ newModule: ["c", "b"], declaredAfter: new Set() });
    const same = { ...m, emitModule: m.resolveModule };
    expect(rewritePaths("self::b::helper() + b::helper()", same).content).toBe(
      "crate::c::b::helper() + crate::c::b::helper()",
    );
    const deep = ctx({
      resolveModule: ["c", "d"],
      newModule: ["c", "b"],
      declaredBefore: new Set(),
      declaredAfter: new Set(),
    });
    const deepSame = { ...deep, emitModule: deep.resolveModule };
    expect(rewritePaths("super::super::a::b::helper()", deepSame).content).toBe(
      "super::super::c::b::helper()",
    );
  });

  test("the moved file: super:: is re-rooted when its parent changes; self:: and unrelated crate:: stay", () => {
    const moved = ctx({
      resolveModule: ["a", "b"],
      emitModule: ["c", "b"],
      newModule: ["c", "b"],
      declaredBefore: new Set(["inner"]),
      declaredAfter: new Set(["inner"]),
    });
    const out = rewritePaths(
      "use super::sibling;\nsuper::sibling() + crate::util::one() + self::inner::f() + inner::g()",
      moved,
    );
    expect(out.content).toBe(
      "use crate::a::sibling;\ncrate::a::sibling() + crate::util::one() + self::inner::f() + inner::g()",
    );
    expect(out.count).toBe(2);
  });

  test("a `use` binding of the module: the local name follows a rename and survives a move", () => {
    const root = ctx({
      resolveModule: [],
      declaredBefore: new Set(["a"]),
      declaredAfter: new Set(["a"]),
    });
    const same = { ...root, emitModule: root.resolveModule };
    expect(rewritePaths("use crate::a::b;\nfn f() -> u32 { b::helper() }", same).content).toBe(
      "use crate::a::bee;\nfn f() -> u32 { bee::helper() }",
    );
    expect(
      rewritePaths("use crate::a::b as bb;\nfn f() -> u32 { bb::helper() }", same).content,
    ).toBe("use crate::a::bee as bb;\nfn f() -> u32 { bb::helper() }");
    const moveRoot = { ...same, newModule: ["c", "b"] as readonly string[] };
    expect(rewritePaths("use crate::a::b;\nfn f() -> u32 { b::helper() }", moveRoot).content).toBe(
      "use crate::c::b;\nfn f() -> u32 { b::helper() }",
    );
  });

  test("brace-group members naming the module are reported, not rewritten (R2)", () => {
    const root = ctx({
      resolveModule: [],
      declaredBefore: new Set(["a"]),
      declaredAfter: new Set(["a"]),
    });
    const same = { ...root, emitModule: root.resolveModule };
    const out = rewritePaths(
      "use crate::util::one;\nuse crate::a::{b, other};\nuse crate::a::b::{helper};\n",
      same,
    );
    expect(out.content).toBe(
      "use crate::util::one;\nuse crate::a::{b, other};\nuse crate::a::bee::{helper};\n",
    );
    expect(out.unrewritten).toEqual([{ line: 2, text: "use crate::a::{b, other}" }]);
  });
});

describe("manifest target entries (R7)", () => {
  const manifest =
    '[package]\nname = "fixture"\nversion = "0.1.0"\n\n[workspace]\nmembers = ["crates/x"]\n\n[lib]\npath = "src/lib_root.rs"\n\n[[bin]]\nname = "cli"\npath = "./src/tools/cli.rs"\n\n[dependencies]\nother = { path = "../other" }\n';

  test("manifestTargetPaths reads [lib]/[[bin]] paths, normalized, and ignores dependencies", () => {
    expect(manifestTargetPaths(manifest)).toEqual(["src/lib_root.rs", "src/tools/cli.rs"]);
  });

  test("rewriteManifestPaths rewrites only the matching target entry and keeps the file well-formed", () => {
    const out = rewriteManifestPaths(manifest, "src/tools/cli.rs", "src/bin/cli.rs");
    expect(out.count).toBe(1);
    expect(out.content).toContain('path = "src/bin/cli.rs"');
    expect(out.content).toContain('members = ["crates/x"]');
    expect(out.content).toContain('other = { path = "../other" }');
    expect(() => Bun.TOML.parse(out.content)).not.toThrow();
    expect(rewriteManifestPaths(manifest, "src/nothing.rs", "x.rs")).toEqual({
      content: manifest,
      count: 0,
    });
  });
});

// ---------------------------------------------------------------------------
// The plan over the fixture crate

describe("planRustMove over the fixture crate", () => {
  test("rename in place (US1): declaration + every path style, moved file untouched", async () => {
    const plan = await planOver(FIXTURE, "src/a/b.rs", "src/a/bee.rs");
    expect(plan.skipped).toBeNull();
    expect(plan.oldModule).toEqual(["a", "b"]);
    expect(plan.newModule).toEqual(["a", "bee"]);
    expect(plan.declaration).toEqual({ kind: "rename", from: ["src/a.rs"], to: null });
    expect(contentOf(plan, "src/a.rs")).toBe(
      "pub mod bee;\npub mod other;\n\npub fn sibling() -> u32 {\n    1\n}\n\npub fn via_self() -> u32 {\n    self::bee::helper()\n}\n\npub fn via_uniform() -> u32 {\n    bee::helper()\n}\n",
    );
    expect(contentOf(plan, "src/lib.rs")).toContain("pub use a::bee::helper as root_helper;");
    expect(contentOf(plan, "src/lib.rs")).toContain(
      "crate::a::bee::helper() + a::bee::helper() + c::from_c()",
    );
    expect(contentOf(plan, "src/a/other.rs")).toContain("super::bee::helper() * 2");
    expect(contentOf(plan, "src/c.rs")).toContain("crate::a::bee::helper() + d::dd()");
    expect(contentOf(plan, "src/c/d.rs")).toContain("super::super::a::bee::helper()");
    expect(plan.movedContent).toBe(FIXTURE["src/a/b.rs"] as string);
    expect(plan.movedRewrites).toBe(0);
    expect(plan.manifest).toBeNull();
    expect(plan.unrewritten).toEqual([]);
    expect(plan.notes[0]).toBe(
      'rust: module crate::a::b → crate::a::bee; declaration renamed in "src/a.rs"; 8 paths rewritten across 5 files (contiguous crate::/super::/self::/uniform paths only; nothing synthesized)',
    );
    expect(plan.notes.some((n) => n.includes("from OTHER crates"))).toBe(true);
  });

  test("move under another parent (US2): declaration moves, super:: re-roots, importers go absolute where needed", async () => {
    const plan = await planOver(FIXTURE, "src/a/b.rs", "src/c/b.rs");
    expect(plan.skipped).toBeNull();
    expect(plan.newModule).toEqual(["c", "b"]);
    expect(plan.declaration).toEqual({ kind: "move", from: ["src/a.rs"], to: "src/c.rs" });
    expect(contentOf(plan, "src/a.rs")).toBe(
      "pub mod other;\n\npub fn sibling() -> u32 {\n    1\n}\n\npub fn via_self() -> u32 {\n    crate::c::b::helper()\n}\n\npub fn via_uniform() -> u32 {\n    crate::c::b::helper()\n}\n",
    );
    expect(contentOf(plan, "src/c.rs")).toBe(
      "pub mod d;\npub mod b;\n\npub fn from_c() -> u32 {\n    crate::c::b::helper() + d::dd()\n}\n",
    );
    expect(contentOf(plan, "src/lib.rs")).toContain("pub use c::b::helper as root_helper;");
    expect(contentOf(plan, "src/lib.rs")).toContain(
      "crate::c::b::helper() + c::b::helper() + c::from_c()",
    );
    expect(contentOf(plan, "src/a/other.rs")).toContain("crate::c::b::helper() * 2");
    expect(contentOf(plan, "src/c/d.rs")).toContain("super::super::c::b::helper()");
    expect(plan.movedContent).toBe(
      "use crate::a::sibling;\n\n/// Helper.\npub fn helper() -> u32 {\n    crate::util::one() + sibling() + crate::a::sibling()\n}\n",
    );
    expect(plan.movedRewrites).toBe(2);
    expect(plan.notes[0]).toContain('declaration moved from "src/a.rs" to "src/c.rs"');
    expect(plan.notes[0]).toContain("plus 2 paths re-rooted in the moved file");
  });

  test("missing destination parent (US2 scenario 4): removal + paths + the exact note, nothing synthesized", async () => {
    const plan = await planOver(FIXTURE, "src/a/b.rs", "src/zz/b.rs");
    expect(plan.declaration).toEqual({ kind: "remove-only", from: ["src/a.rs"], to: null });
    expect(contentOf(plan, "src/a.rs")).not.toContain("mod b;");
    expect(contentOf(plan, "src/lib.rs")).toContain("crate::zz::b::helper()");
    expect(plan.writes.map((w) => w.path)).not.toContain("src/zz.rs");
    expect(plan.notes).toContain(
      'no parent module file for "src/zz"/ ("src/zz.rs", "src/zz/mod.rs"): create one declaring `mod b;` and declare `mod zz;` in "src/lib.rs" yourself — nothing was synthesized',
    );
  });

  test("moving up to the root directory adds the declaration to the root that owns the tree", async () => {
    const plan = await planOver(FIXTURE, "src/a/b.rs", "src/b.rs");
    expect(plan.declaration).toEqual({ kind: "move", from: ["src/a.rs"], to: "src/lib.rs" });
    expect(contentOf(plan, "src/lib.rs")).toContain(
      "mod a;\npub mod c;\npub mod util;\npub mod b;\n",
    );
    expect(contentOf(plan, "src/lib.rs")).toContain("pub use b::helper as root_helper;");
    expect(contentOf(plan, "src/lib.rs")).toContain("crate::b::helper() + b::helper()");
  });

  test("a crate with both lib.rs and main.rs declaring the module: both declarations are rewritten", async () => {
    const seed = {
      ...FIXTURE,
      "src/main.rs":
        'mod a;\nmod c;\nmod util;\n\nfn main() {\n    println!("{}", a::b::helper());\n}\n',
    };
    const plan = await planOver(seed, "src/a/b.rs", "src/a/bee.rs");
    expect(plan.declaration?.from).toEqual(["src/a.rs"]);
    expect(contentOf(plan, "src/main.rs")).toContain("a::bee::helper()");
  });

  test("manifest facts (US3): a [[bin]] path follows the file; auto-discovered targets are noted; members untouched", async () => {
    const seed: Record<string, string> = {
      "Cargo.toml":
        '[package]\nname = "fx"\n\n[workspace]\nmembers = ["crates/x"]\n\n[[bin]]\nname = "cli"\npath = "src/tools/cli.rs"\n',
      "src/lib.rs": "pub fn lib() {}\n",
      "src/tools/cli.rs": "fn main() {}\n",
      "src/bin/other.rs": "fn main() {}\n",
    };
    const pinned = await planOver(seed, "src/tools/cli.rs", "src/bin/cli.rs");
    expect(pinned.manifest).toEqual({ path: "Cargo.toml", entries: 1 });
    expect(contentOf(pinned, "Cargo.toml")).toContain('path = "src/bin/cli.rs"');
    expect(contentOf(pinned, "Cargo.toml")).toContain('members = ["crates/x"]');
    expect(pinned.skipped?.reason).toContain("crate root named by a Cargo.toml target path");
    const auto = await planOver(seed, "src/bin/other.rs", "src/bin/renamed.rs");
    expect(auto.manifest).toBeNull();
    expect(auto.writes).toEqual([]);
    expect(auto.notes[0]).toContain("crate root discovered by location");
    const unrelated = await planOver(seed, "src/lib.rs", "src/lib2.rs");
    expect(unrelated.writes.map((w) => w.path)).not.toContain("Cargo.toml");
  });

  test("negatives (US4): non-leaf, undiscoverable, #[path], style conversion, outside the tree", async () => {
    const nonLeaf = await planOver(FIXTURE, "src/a.rs", "src/aa.rs");
    expect(nonLeaf.skipped?.reason).toBe(
      '"src/a.rs" has child modules (b.rs, other.rs) and directories are not movable in v1, so they cannot follow',
    );
    expect(nonLeaf.writes).toEqual([]);

    const orphan = await planOver(
      { ...FIXTURE, "src/orphan.rs": "pub fn o() {}\n" },
      "src/orphan.rs",
      "src/o2.rs",
    );
    expect(orphan.skipped?.reason).toBe('no `mod orphan;` declaration found in "src/lib.rs"');

    const noChain = await planOver(
      { ...FIXTURE, "src/q/deep.rs": "pub fn o() {}\n" },
      "src/q/deep.rs",
      "src/q/d2.rs",
    );
    expect(noChain.skipped?.reason).toContain("no module chain");

    const pathAttr = await planOver(
      { ...FIXTURE, "src/a.rs": '#[path = "b.rs"]\npub mod b;\n' },
      "src/a/b.rs",
      "src/a/bee.rs",
    );
    expect(pathAttr.skipped?.reason).toContain("#[path] attribute");

    const style = await planOver(FIXTURE, "src/c/d.rs", "src/c/d/mod.rs");
    expect(style.skipped?.reason).toContain("style conversion of the same module");
    const styleBack = await planOver(
      { ...FIXTURE, "src/m/mod.rs": "pub fn m() {}\n" },
      "src/m/mod.rs",
      "src/m.rs",
    );
    expect(styleBack.skipped?.reason).toContain("style conversion of the same module");

    const outside = await planOver(FIXTURE, "src/a/b.rs", "tests/b.rs");
    expect(outside.skipped?.reason).toContain("outside the module tree");
  });

  test("files under target/ are junk and never scanned; other crates' files are not touched", async () => {
    const seed = {
      ...FIXTURE,
      "target/debug/build/gen.rs": "use crate::a::b::helper;\n",
      "crates/other/Cargo.toml": '[package]\nname = "other"\n',
      "crates/other/src/lib.rs": "use fixture::a::b::helper;\nuse crate::a::b::x;\n",
      "tests/it.rs": "use crate::a::b::helper;\n",
    };
    const plan = await planOver(seed, "src/a/b.rs", "src/a/bee.rs");
    const touched = plan.writes.map((w) => w.path);
    expect(touched).not.toContain("target/debug/build/gen.rs");
    expect(touched).not.toContain("crates/other/src/lib.rs");
    expect(touched).not.toContain("tests/it.rs");
  });
});

// ---------------------------------------------------------------------------
// Through the files tool (in-process rig, mirrored on files-tool.test)

function enoent(path: string): NodeJS.ErrnoException {
  const error = new Error(`ENOENT: no such file: ${path}`) as NodeJS.ErrnoException;
  error.code = "ENOENT";
  return error;
}

interface Rig {
  readonly deps: FilesDeps;
  readonly ports: InProcessPorts;
  readonly files: Map<string, string>;
}

function makeRig(seed: Record<string, string>): Rig {
  const ports = createInProcessPorts();
  const files = ports.workspace.files;
  for (const [path, content] of Object.entries(seed)) files.set(path, content);
  const write: WriteEngineDeps = {
    engine: ports.engine,
    formatter: ports.formatter,
    journal: ports.journal,
    workspace: {
      read: (path) =>
        files.has(path) ? Promise.resolve(files.get(path) as string) : Promise.reject(enoent(path)),
      write: (path, content) => {
        files.set(path, content);
        return Promise.resolve();
      },
      remove: (path) => {
        files.delete(path);
        return Promise.resolve();
      },
    },
  };
  const key = Buffer.alloc(KEY_LENGTH, 7);
  const deps: FilesDeps = {
    write,
    confirm: {
      mint: (diffHash, contentHash) => mintConfirmGrant(key, diffHash, contentHash),
      check: (token) => checkConfirmGrant(key, token),
    },
    list: () => Promise.resolve(listing(files)),
  };
  return { deps, ports, files };
}

function run(rig: Rig, input: FilesInput): Promise<ResponseEnvelope<WriteReport>> {
  return runFiles(rig.deps, input, 1, "suite");
}

function refused(envelope: ResponseEnvelope<WriteReport>) {
  if (envelope.ok) throw new Error("expected an error envelope, got ok");
  return envelope.error as typeof envelope.error & Record<string, unknown>;
}

describe("files.move over a Rust crate (rig)", () => {
  test("is confirm-gated with the aggregated diff naming the parent and importer files (SC-004)", async () => {
    const rig = makeRig(FIXTURE);
    const gated = await run(rig, { op: "move", from: "src/a/b.rs", to: "src/a/bee.rs" });
    const error = refused(gated);
    expect(error.code).toBe("CONFIRM_REQUIRED");
    expect(typeof error["confirmToken"]).toBe("string");
    const notes = (gated.notes ?? []).join("\n");
    expect(notes).toContain("rust: module crate::a::b → crate::a::bee");
    expect(notes).toContain("src/a.rs");
    expect(notes).toContain("src/c/d.rs");
    expect(rig.files.get("src/a.rs")).toBe(FIXTURE["src/a.rs"]);
    expect(rig.files.has("src/a/bee.rs")).toBe(false);
  });

  test("dryRun previews the whole batch and applies nothing (SC-004)", async () => {
    const rig = makeRig(FIXTURE);
    const preview = await run(rig, {
      op: "move",
      from: "src/a/b.rs",
      to: "src/c/b.rs",
      dryRun: true,
    });
    if (!preview.ok) throw new Error(preview.error.code);
    expect(preview.results.length).toBeGreaterThanOrEqual(7);
    for (const row of preview.results) expect(row.applied).toBe(false);
    expect(rig.files.get("src/c.rs")).toBe(FIXTURE["src/c.rs"]);
    expect(await rig.ports.journal.list(0)).toHaveLength(0);
  });

  test("the confirm-token two-call flow applies the rename everywhere (SC-001)", async () => {
    const rig = makeRig(FIXTURE);
    const gated = await run(rig, { op: "move", from: "src/a/b.rs", to: "src/a/bee.rs" });
    const token = refused(gated)["confirmToken"] as string;
    const applied = await run(rig, {
      op: "move",
      from: "src/a/b.rs",
      to: "src/a/bee.rs",
      confirm: token,
    });
    if (!applied.ok) throw new Error(applied.error.code);
    expect(rig.files.has("src/a/b.rs")).toBe(false);
    expect(rig.files.get("src/a/bee.rs")).toBe(FIXTURE["src/a/b.rs"]);
    expect(rig.files.get("src/a.rs")).toContain("pub mod bee;");
    expect(rig.files.get("src/lib.rs")).toContain("crate::a::bee::helper()");
    expect(rig.files.get("src/c/d.rs")).toContain("super::super::a::bee::helper()");
  });

  test("a move with autoApply lands the declaration move and the manifest entry (SC-002/SC-003)", async () => {
    const rig = makeRig({
      ...FIXTURE,
      "Cargo.toml":
        '[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2021"\n\n[[bin]]\nname = "cli"\npath = "src/tools/cli.rs"\n',
      "src/tools/cli.rs": "fn main() {}\n",
    });
    const moved = await run(rig, {
      op: "move",
      from: "src/a/b.rs",
      to: "src/c/b.rs",
      autoApply: true,
    });
    if (!moved.ok) throw new Error(moved.error.code);
    expect(rig.files.get("src/a.rs")).not.toContain("mod b;");
    expect(rig.files.get("src/c.rs")).toContain("pub mod d;\npub mod b;\n");
    expect(rig.files.get("src/c/b.rs")).toContain("use crate::a::sibling;");
    const manifest = await run(rig, {
      op: "move",
      from: "src/tools/cli.rs",
      to: "src/bin/cli.rs",
      autoApply: true,
    });
    if (!manifest.ok) throw new Error(manifest.error.code);
    expect(rig.files.get("Cargo.toml")).toContain('path = "src/bin/cli.rs"');
    expect(() => Bun.TOML.parse(rig.files.get("Cargo.toml") as string)).not.toThrow();
  });

  test("negative shapes surface their notes and the move still proceeds as a plain move (SC-005)", async () => {
    const rig = makeRig({
      ...FIXTURE,
      "src/lib.rs": `${FIXTURE["src/lib.rs"]}use crate::a::{b, other};\n`,
    });
    const gated = await run(rig, { op: "move", from: "src/a/b.rs", to: "src/a/bee.rs" });
    expect((gated.notes ?? []).join("\n")).toContain(
      "not rewritten: src/lib.rs:12 — use-tree member names the moved module (use crate::a::{b, other}); split it by hand",
    );
    const nonLeaf = await run(rig, {
      op: "move",
      from: "src/a.rs",
      to: "src/aa.rs",
      autoApply: true,
    });
    if (!nonLeaf.ok) throw new Error(nonLeaf.error.code);
    expect((nonLeaf.notes ?? []).join("\n")).toContain("has child modules (b.rs, other.rs)");
    expect(rig.files.has("src/aa.rs")).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Compile proof (R10): the real composed write access over a temp crate, then cargo.

const CARGO = Bun.which("cargo");
const cargoWorks =
  CARGO !== null &&
  Bun.spawnSync([CARGO, "--version"], { stdout: "pipe", stderr: "pipe" }).exitCode === 0;

function seedCrate(root: string, seed: Record<string, string>): void {
  for (const [rel, content] of Object.entries(seed)) {
    const target = join(root, rel);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, content, "utf8");
  }
}

function walk(root: string, dir = root): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    if (name === ".workbench" || name === "target") continue;
    const full = join(dir, name);
    if (statSync(full).isDirectory()) out.push(...walk(root, full));
    else out.push(relative(root, full).replaceAll("\\", "/"));
  }
  return out;
}

function realRig(root: string): FilesDeps {
  const ports = createInProcessPorts();
  const access = createWriteAccess({ root, stateDir: join(root, ".workbench") });
  const key = Buffer.alloc(KEY_LENGTH, 9);
  return {
    write: {
      engine: ports.engine,
      formatter: ports.formatter,
      journal: ports.journal,
      workspace: access,
    },
    confirm: {
      mint: (diffHash, contentHash) => mintConfirmGrant(key, diffHash, contentHash),
      check: (token) => checkConfirmGrant(key, token),
    },
    list: () => Promise.resolve(walk(root).map((path) => ({ path, junk: false }))),
  };
}

function cargoCheck(root: string): { exitCode: number; stderr: string } {
  const proc = Bun.spawnSync([CARGO as string, "check", "--offline", "--quiet"], {
    cwd: root,
    stdout: "pipe",
    stderr: "pipe",
    env: { ...process.env, CARGO_TARGET_DIR: join(root, "target") },
  });
  return { exitCode: proc.exitCode, stderr: proc.stderr.toString() };
}

describe("compile proof (toolchain-gated)", () => {
  let root: string | null = null;
  afterEach(() => {
    if (root !== null) rmSync(root, { recursive: true, force: true });
    root = null;
  });

  test.skipIf(!cargoWorks)(
    "the fixture crate compiles before any move (sanity)",
    () => {
      root = mkdtempSync(join(tmpdir(), "workbench-rust-move-"));
      seedCrate(root, FIXTURE);
      const result = cargoCheck(root);
      expect(result.stderr).not.toContain("error");
      expect(result.exitCode).toBe(0);
    },
    120_000,
  );

  test.skipIf(!cargoWorks)(
    "rename in place: cargo check exits 0 after apply (SC-001)",
    async () => {
      root = mkdtempSync(join(tmpdir(), "workbench-rust-move-"));
      seedCrate(root, FIXTURE);
      const applied = await runFiles(
        realRig(root),
        { op: "move", from: "src/a/b.rs", to: "src/a/bee.rs", autoApply: true },
        1,
        "suite",
      );
      if (!applied.ok) throw new Error(`${applied.error.code}: ${applied.error.message}`);
      expect(readFileSync(join(root, "src/a.rs"), "utf8")).toContain("pub mod bee;");
      const result = cargoCheck(root);
      expect(result.stderr).not.toContain("error");
      expect(result.exitCode).toBe(0);
    },
    120_000,
  );

  test.skipIf(!cargoWorks)(
    "move under another parent: cargo check exits 0 after apply (SC-002)",
    async () => {
      root = mkdtempSync(join(tmpdir(), "workbench-rust-move-"));
      seedCrate(root, FIXTURE);
      const applied = await runFiles(
        realRig(root),
        { op: "move", from: "src/a/b.rs", to: "src/c/b.rs", autoApply: true },
        1,
        "suite",
      );
      if (!applied.ok) throw new Error(`${applied.error.code}: ${applied.error.message}`);
      expect(readFileSync(join(root, "src/c.rs"), "utf8")).toContain("pub mod b;");
      const result = cargoCheck(root);
      expect(result.stderr).not.toContain("error");
      expect(result.exitCode).toBe(0);
    },
    120_000,
  );

  test.skipIf(cargoWorks)(
    "compile proof skipped: needs a WORKING cargo on PATH (--version exits 0) (specs/010 R10)",
    () => {
      expect(cargoWorks).toBe(false);
    },
  );
});
