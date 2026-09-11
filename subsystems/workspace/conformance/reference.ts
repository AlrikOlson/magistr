/** Run the shared consumer corpus through the retained reference's real addon. */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { loadVerifiedCore } from "../reference/src/adapters/core/loader";
import fixtures from "./engine.json";

const loaded = loadVerifiedCore({ force: "native" });
assert.equal(loaded.flavor, "native");
const core = loaded.binding;

for (const fixture of fixtures.parse) {
  const parsed = core.parse(fixture.language, fixture.source);
  assert.deepEqual(
    {
      language: parsed.language,
      root_kind: parsed.rootKind,
      has_error: parsed.hasError,
    },
    {
      language: fixture.language,
      root_kind: fixture.root_kind,
      has_error: fixture.has_error,
    },
  );
  assert.equal(parsed.errorCount > 0, fixture.has_error);
  assert.ok(parsed.nodeCount > 1);
}
assert.throws(() => core.parse(fixtures.errors.unknown_language, "x"), /unknown language/);

const search = fixtures.search;
assert.deepEqual(
  core.search(search.pattern, search.source, false).map((hit) => ({
    line: hit.line,
    start: hit.start,
    end: hit.end,
    line_text: hit.lineText,
  })),
  search.hits,
);
assert.throws(
  () => core.search(fixtures.errors.search_pattern, search.source, false),
  /invalid search pattern/,
);

const rewrite = fixtures.rewrite;
const hits = core.structuralSearch(rewrite.language, rewrite.source, rewrite.pattern);
assert.deepEqual(hits.map((hit) => hit.line), rewrite.match_lines);
assert.ok(hits.every((hit) => hit.kind === "call_expression"));
const rewritten = core.rewrite(
  rewrite.language,
  rewrite.source,
  rewrite.pattern,
  rewrite.replacement,
);
assert.equal(rewritten.matchCount, hits.length);
assert.equal(rewritten.output, rewrite.output);
assert.throws(
  () => core.rewrite(rewrite.language, rewrite.source, fixtures.errors.rewrite_pattern, "x"),
  /invalid rewrite pattern/,
);
assert.throws(
  () => core.structuralSearch(rewrite.language, rewrite.source, fixtures.errors.structural_pattern),
  /invalid structural pattern/,
);

const extract = fixtures.extract;
const extracted = await core.extract(extract.language, extract.source);
assert.deepEqual(
  extracted.symbols.map((symbol) => [symbol.name, symbol.kind, symbol.startLine, symbol.startCol]),
  extract.symbols,
);
assert.deepEqual(extracted.imports, extract.imports);
assert.deepEqual(extracted.testRanges, []);
await assert.rejects(
  () => core.extract(fixtures.errors.unknown_language, extract.source),
  /unknown language/,
);

assert.ok(core.sandboxRunComponent, "the native reference must include the sandbox");
assert.ok(core.sandboxRunWasiModule, "the native reference must include WASI");
const policy = { fuel: 1_000_000, memoryCapBytes: 10 * 65536 };
const guest = (name: string) => readFile(new URL(`./${name}.wat`, import.meta.url));

for (const [name, exportName, expected] of [
  ["spin", "spin", "out-of-fuel"],
  ["hog", "hog", "memory-cap"],
] as const) {
  const bytes = await guest(name);
  const first = await core.sandboxRunComponent(bytes, exportName, [], [], policy);
  const second = await core.sandboxRunComponent(bytes, exportName, [], [], policy);
  assert.equal(first.outcome, expected);
  assert.equal(second.outcome, expected);
  assert.equal(first.fuelUsed, second.fuelUsed);
}
const crash = await core.sandboxRunComponent(await guest("crash"), "crash", [], [], policy);
assert.equal(crash.outcome, "guest-trap");
const alive = await core.sandboxRunComponent(await guest("add"), "add", [19, 23], [], policy);
assert.equal(alive.outcome, "ok");
assert.deepEqual(alive.results, ["U32(42)"]);
assert.ok(alive.fuelUsed > 0);

const observe = await guest("observe");
const denied = await core.sandboxRunComponent(observe, "run", [], [], policy);
assert.equal(denied.outcome, "cap-denied");
assert.equal(denied.detail, "observe");
assert.equal(denied.fuelUsed, 0);
const allowed = await core.sandboxRunComponent(observe, "run", [], ["observe"], policy);
assert.equal(allowed.outcome, "ok");
assert.deepEqual(allowed.observations, [7, 11, 13]);
assert.deepEqual(allowed.results, ["U32(3)"]);
const invalid = await core.sandboxRunComponent(Buffer.from("not wasm"), "run", [], [], policy);
assert.equal(invalid.outcome, "invalid-guest");

const probe = await core.sandboxRunWasiModule(await guest("preopen_probe"), ["probe"], policy);
assert.equal(probe.outcome, "guest-exit");
assert.equal(probe.exitCode, 8);
const hello = await core.sandboxRunWasiModule(await guest("hello_wasi"), ["hello"], policy);
assert.equal(hello.outcome, "ok");
assert.equal(hello.stdout, "jailed");
assert.equal(hello.stderr, "");

process.stdout.write("Reference native addon passed shared engine and sandbox conformance fixtures.\n");
