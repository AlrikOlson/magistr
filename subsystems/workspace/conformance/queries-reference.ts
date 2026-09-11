/** Real retained native addon and index, in a disposable root independent of native tests. */
import assert from "node:assert/strict";
import {
	mkdir,
	mkdtemp,
	readFile,
	rm,
	stat,
	writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { loadVerifiedCore } from "../reference/src/adapters/core/loader";
import { IndexerHost } from "../reference/src/adapters/engine-ipc/indexer";
import { IpcIndexAdapter } from "../reference/src/adapters/engine-ipc/ipc-index";
import { runRead } from "../reference/src/tools/read";
import { runSearch } from "../reference/src/tools/search";
import { runMap } from "../reference/src/tools/map";
import { PORT_VERSIONS } from "../reference/src/ports";
import { sealEnvelope } from "../reference/src/envelope";
import fixtures from "./queries.json";

const root = await mkdtemp(join(tmpdir(), "magistr-query-reference-"));
const loaded = loadVerifiedCore({ force: "native" });
assert.equal(loaded.flavor, "native");
const core = loaded.binding;
const host = new IndexerHost(core);
const adapter = new IpcIndexAdapter(
	{ call: (method, params) => host.handle(method, params) } as never,
	root,
);
const cold = new IpcIndexAdapter(
	{
		call: async () => {
			throw new Error("cold index");
		},
	} as never,
	root,
);
try {
	await mkdir(join(root, ".git"));
	for (const file of fixtures.files) {
		await mkdir(join(root, file.path, ".."), { recursive: true });
		await writeFile(join(root, file.path), file.content);
	}
	await adapter.build(root);
	const deadline = Date.now() + 15_000;
	while ((await adapter.status()).building) {
		assert.ok(Date.now() < deadline, "reference index build deadline");
		await Bun.sleep(5);
	}
	assert.equal((await adapter.status()).tier, 1);
	const workspace = {
		list: async () =>
			(await core.scanWorkspace(root)).map((f) => ({
				path: f.path,
				junk: f.junk,
			})),
		read: (path) => readFile(join(root, path), "utf8"),
		stat: async (path) => {
			const m = await stat(join(root, path));
			return { mtimeMs: m.mtimeMs, size: m.size };
		},
	};
	const language = (path) =>
		path.endsWith(".rs")
			? "rust"
			: path.endsWith(".ts")
				? "typescript"
				: undefined;
	const engine = {
		version: PORT_VERSIONS.engine,
		search: async (request) => {
			const matches = [];
			for (const path of request.paths) {
				const source = await workspace.read(path);
				if (request.mode === "structural") {
					const lang = language(path);
					if (!lang) continue;
					for (const hit of core.structuralSearch(
						lang,
						source,
						request.pattern,
					)) {
						matches.push({
							location: { path, line: hit.line, column: hit.column },
							excerpt: hit.lineText,
							kind: hit.kind,
							endLine: hit.endLine,
						});
					}
				} else {
					for (const hit of core.search(request.pattern, source, false)) {
						matches.push({
							location: { path, line: hit.line, column: hit.start + 1 },
							excerpt: hit.lineText,
						});
					}
				}
			}
			return matches;
		},
	};
	const results = [];
	for (const test of fixtures.cases) {
		const index = test.warm ? adapter : cold;
		let response;
		if (test.tool === "read")
			response = await runRead({ index, workspace }, test.input, 1);
		else if (test.tool === "search")
			response = await runSearch(
				{ index, workspace, engine } as never,
				test.input,
				1,
			);
		else
			response = await runMap(
				{ index, workspace, plugins: async () => [] },
				test.input,
				1,
			);
		results.push({ name: test.name, response: sealEnvelope(response).envelope });
	}
	process.stdout.write(JSON.stringify(results));
} finally {
	host.close();
	await rm(root, { recursive: true, force: true });
}
