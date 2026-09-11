/**
 * Capability manifests + the plugin-tool host (SDD §8.1, §8.3,
 * chunk:m4-capability-manifests). Pins the chunk's three acceptance criteria:
 *
 *   1. A plugin exceeding its granted capabilities is denied with
 *      PLUGIN_CAP_DENIED — default deny, decided before instantiation.
 *   2. A crashed plugin is circuit-breakered while core tools continue at
 *      full fidelity — the breaker is disk-backed and keyed by component
 *      hash, and core dispatch never consults plugin state.
 *   3. No plugin can shadow, wrap, intercept or rename a core tool — wire
 *      names are derived (`publisher.pkg/tool`), reserved bare names are
 *      refused at validation, and core dispatch precedes plugin dispatch.
 *
 * Three chapters: manifest validation (pure), host protocol over a fake
 * guest (pure), and the served surface over linked in-memory transports.
 * A fourth runs the real native binding end to end when the artifact exists.
 */

import { describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { defaultArtifactsDir, loadCore, nativeArtifactKey } from "../src/adapters/core/loader";
import { defaultResolvedConfig } from "../src/config";
import type { ResponseEnvelope } from "../src/envelope";
import type {
  ImportGraphResult,
  IndexPort,
  OutlineQueryResult,
  ReferenceQueryResult,
  SymbolQueryResult,
} from "../src/ports";
import { PORT_VERSIONS } from "../src/ports";
import { buildServer, type PluginsRuntime, type WorkspaceRuntime } from "../src/protocol";
import type {
  PluginHealthRecord,
  PluginHostDeps,
  PluginInvokeOutcome,
  SearchDeps,
} from "../src/tools";
import {
  parsePluginManifest,
  RESERVED_TOOL_NAMES,
  runPluginTool,
  wireToolName,
} from "../src/tools";

// ---- Chapter 1: plugin.toml validation -------------------------------------

const GOOD_TOML = `
[plugin]
publisher = "acme"
pkg = "hello"
version = "0.1.0"

[component]
path = "guest.wasm"

[[tools]]
name = "greet"
description = "Greets the payload"

[capabilities]
imports = ["observe"]
fs-read = ["src/**"]

[limits]
fuel = 500
memory_bytes = 1048576
`;

describe("plugin.toml manifest validation", () => {
  test("a valid manifest parses, and wire names are derived, never declared", () => {
    const parsed = parsePluginManifest(GOOD_TOML);
    if (!parsed.ok) throw new Error(parsed.errors.join("; "));
    expect(parsed.manifest.publisher).toBe("acme");
    expect(parsed.manifest.tools).toEqual([{ name: "greet", description: "Greets the payload" }]);
    expect(parsed.manifest.imports).toEqual(["observe"]);
    expect(parsed.manifest.fsRead).toEqual(["src/**"]);
    expect(parsed.manifest.limits).toEqual({ fuel: 500, memoryCapBytes: 1048576 });
    expect(wireToolName(parsed.manifest, "greet")).toBe("acme.hello/greet");
  });

  test("every reserved core tool name is refused as a tool name", () => {
    for (const reserved of RESERVED_TOOL_NAMES) {
      const parsed = parsePluginManifest(
        GOOD_TOML.replace('name = "greet"', `name = "${reserved}"`),
      );
      expect(parsed.ok).toBe(false);
      // Kebab-valid reserved names hit the reservation rule by name;
      // "workbench_status" is already unrepresentable in the charset.
      if (!parsed.ok && /^[a-z][a-z0-9-]{0,31}$/.test(reserved)) {
        expect(parsed.errors.join(" ")).toContain("reserved");
      }
    }
  });

  test("identity, component path and tools are fail-closed", () => {
    const red = [
      GOOD_TOML.replace('publisher = "acme"', 'publisher = "ACME!"'),
      GOOD_TOML.replace('pkg = "hello"', 'pkg = "has/slash"'),
      GOOD_TOML.replace('version = "0.1.0"', 'version = ""'),
      GOOD_TOML.replace('path = "guest.wasm"', 'path = "../../etc/passwd"'),
      GOOD_TOML.replace('path = "guest.wasm"', 'path = "/abs/guest.wasm"'),
      GOOD_TOML.replace(/\[\[tools\]\][\s\S]*?description = "Greets the payload"\n/, ""),
      GOOD_TOML.replace("fuel = 500", 'fuel = "lots"'),
      "not = [valid toml",
    ];
    for (const source of red) {
      expect(parsePluginManifest(source).ok).toBe(false);
    }
  });

  test("a duplicate tool name is refused; unknown tables warn but parse", () => {
    const dup = `${GOOD_TOML}\n[[tools]]\nname = "greet"\ndescription = "again"\n`;
    expect(parsePluginManifest(dup).ok).toBe(false);

    const unknown = `${GOOD_TOML}\n[surprise]\nkey = 1\n`;
    const parsed = parsePluginManifest(unknown);
    if (!parsed.ok) throw new Error(parsed.errors.join("; "));
    expect(parsed.warnings.join(" ")).toContain("surprise");
  });
});

// ---- Chapter 2: host protocol over a fake guest ----------------------------

interface FakeHost {
  readonly deps: PluginHostDeps;
  readonly invokes: {
    capabilities: readonly string[];
    policy: { fuel: number; memoryCapBytes: number };
  }[];
  readonly health: PluginHealthRecord[];
  hash: string;
}

function fakeHost(init: {
  grants?: readonly string[];
  outcome: PluginInvokeOutcome | (() => PluginInvokeOutcome);
  toml?: string;
}): FakeHost {
  const invokes: FakeHost["invokes"] = [];
  const health: PluginHealthRecord[] = [];
  const host: FakeHost = {
    invokes,
    health,
    hash: "h1",
    deps: {
      inventory: () =>
        Promise.resolve([
          {
            name: "acme-hello",
            source: "sideload" as const,
            dir: "plugins/hello",
            ...(init.grants === undefined ? {} : { grants: init.grants }),
          },
        ]),
      readManifest: () => Promise.resolve(init.toml ?? GOOD_TOML),
      componentInfo: () =>
        Promise.resolve({ absPath: "/w/plugins/hello/guest.wasm", sha256: host.hash }),
      invokeGuest: (_absPath, _exportName, _payload, capabilities, policy) => {
        invokes.push({ capabilities, policy });
        return Promise.resolve(typeof init.outcome === "function" ? init.outcome() : init.outcome);
      },
      health: {
        read: () => Promise.resolve([...health]),
        append: (record) => {
          health.push(record);
          return Promise.resolve();
        },
      },
      now: () => "2026-08-22T00:00:00.000Z",
    },
  };
  return host;
}

const OK_OUTCOME: PluginInvokeOutcome = {
  outcome: "ok",
  detail: "",
  fuelUsed: 10,
  result: '{"greeting":"hi"}',
};

describe("plugin host over a fake guest", () => {
  test("an unknown wire name answers null (the transport's unknown-tool error)", async () => {
    const host = fakeHost({ outcome: OK_OUTCOME });
    expect(await runPluginTool(host.deps, "acme.hello/nope", {}, 1)).toBeNull();
    expect(await runPluginTool(host.deps, "other.pkg/greet", {}, 1)).toBeNull();
  });

  test("default deny: no grant record means zero capabilities reach the guest", async () => {
    const host = fakeHost({ outcome: OK_OUTCOME });
    await runPluginTool(host.deps, "acme.hello/greet", {}, 1);
    expect(host.invokes[0]?.capabilities).toEqual([]);
  });

  test("effective capabilities are declared ∩ granted — an undeclared grant is inert", async () => {
    const host = fakeHost({
      grants: ["observe", "workbench:tool/host@0.1.0"],
      outcome: OK_OUTCOME,
    });
    await runPluginTool(host.deps, "acme.hello/greet", {}, 1);
    // The manifest declares only "observe"; the extra grant grants nothing.
    expect(host.invokes[0]?.capabilities).toEqual(["observe"]);
  });

  // Acceptance 1 — exceeding grants is PLUGIN_CAP_DENIED.
  test("a cap-denied guest end maps to PLUGIN_CAP_DENIED (escalate), no breach recorded", async () => {
    const host = fakeHost({
      outcome: { outcome: "cap-denied", detail: "observe", fuelUsed: 0, result: null },
    });
    const envelope = await runPluginTool(host.deps, "acme.hello/greet", {}, 1);
    expect(envelope?.ok).toBe(false);
    if (envelope?.ok === false) {
      expect(envelope.error.code).toBe("PLUGIN_CAP_DENIED");
      expect(envelope.error.recovery).toBe("escalate");
      expect(envelope.error.message).toContain('"observe"');
    }
    expect(host.health).toHaveLength(0);
  });

  // Acceptance 2 — crash → breaker; later calls short-circuit.
  test("a trapped guest is PLUGIN_CRASHED, breached on disk, and short-circuited after", async () => {
    const host = fakeHost({
      outcome: { outcome: "guest-trap", detail: "unreachable", fuelUsed: 5, result: null },
    });
    const first = await runPluginTool(host.deps, "acme.hello/greet", {}, 1);
    expect(first?.ok).toBe(false);
    if (first?.ok === false) {
      expect(first.error.code).toBe("PLUGIN_CRASHED");
      expect((first.error as { plugin?: string }).plugin).toBe("acme.hello");
    }
    expect(host.health).toHaveLength(1);
    expect(host.health[0]?.componentHash).toBe("h1");

    const second = await runPluginTool(host.deps, "acme.hello/greet", {}, 1);
    expect(second?.ok).toBe(false);
    if (second?.ok === false) {
      expect(second.error.code).toBe("PLUGIN_CRASHED");
      expect(second.error.message).toContain("circuit-breakered");
    }
    // The engine was NOT touched for the second call.
    expect(host.invokes).toHaveLength(1);
  });

  test("a changed component clears the breaker (quarantine recovery story)", async () => {
    const host = fakeHost({
      outcome: { outcome: "out-of-fuel", detail: "", fuelUsed: 500, result: null },
    });
    await runPluginTool(host.deps, "acme.hello/greet", {}, 1);
    expect(host.invokes).toHaveLength(1);
    host.hash = "h2"; // new component bytes
    await runPluginTool(host.deps, "acme.hello/greet", {}, 1);
    expect(host.invokes).toHaveLength(2);
  });

  test("manifest limits clamp downward against the host ceiling, never up", async () => {
    const small = fakeHost({ outcome: OK_OUTCOME });
    await runPluginTool(small.deps, "acme.hello/greet", {}, 1);
    expect(small.invokes[0]?.policy).toEqual({ fuel: 500, memoryCapBytes: 1048576 });

    const greedy = fakeHost({
      outcome: OK_OUTCOME,
      toml: GOOD_TOML.replace("fuel = 500", "fuel = 5000000000000").replace(
        "memory_bytes = 1048576",
        "memory_bytes = 5000000000000",
      ),
    });
    await runPluginTool(greedy.deps, "acme.hello/greet", {}, 1);
    expect(greedy.invokes[0]?.policy.fuel).toBe(1_000_000_000);
    expect(greedy.invokes[0]?.policy.memoryCapBytes).toBe(256 * 1024 * 1024);
  });

  test("an ok result is host-wrapped in the envelope, JSON parsed", async () => {
    const host = fakeHost({ outcome: OK_OUTCOME });
    const envelope = await runPluginTool(host.deps, "acme.hello/greet", { name: "x" }, 1);
    expect(envelope?.ok).toBe(true);
    if (envelope?.ok === true) {
      expect(envelope.results[0]).toEqual({ greeting: "hi" });
      expect(envelope.notes?.join(" ")).toContain("host-wrapped");
      expect(envelope.notes?.join(" ")).toContain("sideload");
    }
  });

  test("a thrown guest run (child down, degraded lane) is ENGINE_UNAVAILABLE, not a breach", async () => {
    const host = fakeHost({
      outcome: () => {
        throw new Error("SANDBOX_UNAVAILABLE: no host");
      },
    });
    const envelope = await runPluginTool(host.deps, "acme.hello/greet", {}, 1);
    expect(envelope?.ok).toBe(false);
    if (envelope?.ok === false) {
      expect(envelope.error.code).toBe("ENGINE_UNAVAILABLE");
    }
    expect(host.health).toHaveLength(0);
  });
});

// ---- Chapter 3: the served surface (no shadowing, core continuity) ---------

function fakeSearch(): SearchDeps {
  const index: IndexPort = {
    version: PORT_VERSIONS.index,
    status: () => Promise.resolve({ tier: 0 as const, generation: 0, building: false }),
    build: () => Promise.resolve({ tier: 0 as const, generation: 0, building: false }),
    querySymbols: (): Promise<SymbolQueryResult> =>
      Promise.resolve({ symbols: [], freshness: "warm" }),
    queryReferences: (): Promise<ReferenceQueryResult> =>
      Promise.resolve({ edges: [], freshness: "warm" }),
    queryOutline: (): Promise<OutlineQueryResult> =>
      Promise.resolve({ files: [], freshness: "warm" }),
    queryImportGraph: (): Promise<ImportGraphResult> =>
      Promise.resolve({ fanIn: [], fanOut: [], freshness: "warm" }),
    upsertFile: () => Promise.resolve(),
    refreshFile: () => Promise.resolve(),
    removeFile: () => Promise.resolve(),
  };
  return {
    engine: {
      version: PORT_VERSIONS.engine,
      parse: () => Promise.reject(new Error("not under test")),
      search: () => Promise.resolve([]),
      rewrite: () => Promise.reject(new Error("not under test")),
    },
    index,
    workspace: {
      list: () => Promise.resolve([]),
      read: () => Promise.reject(new Error("not under test")),
    },
  };
}

/** A real temp workspace with an installed plugin (manifest on disk). */
function pluginWorkspace(init: {
  toml: string;
  grants?: readonly string[];
  componentBytes?: string;
}): WorkspaceRuntime {
  const root = mkdtempSync(path.join(tmpdir(), "workbench-plug-"));
  const stateDir = path.join(root, ".workbench");
  mkdirSync(stateDir, { recursive: true });
  mkdirSync(path.join(root, "plugins", "hello"), { recursive: true });
  writeFileSync(path.join(root, "plugins", "hello", "plugin.toml"), init.toml);
  writeFileSync(
    path.join(root, "plugins", "hello", "guest.wasm"),
    init.componentBytes ?? "(component)",
  );
  writeFileSync(
    path.join(stateDir, "plugins.json"),
    JSON.stringify({
      plugins: [
        {
          name: "acme-hello",
          source: "sideload",
          dir: "plugins/hello",
          ...(init.grants === undefined ? {} : { grants: init.grants }),
        },
      ],
    }),
  );
  return {
    root,
    stateDir,
    detection: { networkFs: false, method: "default-local" },
    journalMode: "WAL",
    appliedJournalMode: "wal",
    config: defaultResolvedConfig(),
    notes: [],
    close: () => {},
  };
}

function envelopeFrom(result: unknown): ResponseEnvelope {
  const content = (result as { content: Array<{ type: string; text: string }> }).content;
  const first = content[0];
  if (first === undefined) throw new Error("empty content");
  return JSON.parse(first.text) as ResponseEnvelope;
}

async function connectedClient(
  workspace: WorkspaceRuntime,
  plugins: PluginsRuntime,
): Promise<Client> {
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  const server = buildServer(workspace, fakeSearch(), undefined, undefined, plugins);
  const client = new Client({ name: "plugin-test", version: "0.0.0" });
  await server.connect(serverTransport);
  await client.connect(clientTransport);
  return client;
}

describe("the served surface: namespaces, no shadowing, core continuity", () => {
  test("plugin tools list under derived namespaces beside the core tools", async () => {
    const workspace = pluginWorkspace({ toml: GOOD_TOML });
    const client = await connectedClient(workspace, {
      invokeGuest: () => Promise.resolve(OK_OUTCOME),
    });
    const tools = (await client.listTools()).tools.map((t) => t.name);
    expect(tools).toContain("acme.hello/greet");
    expect(tools).toContain("search");
    expect(tools).toContain("workbench_status");
    // Exactly one "search": the core tool. Nothing else may bear the name.
    expect(tools.filter((name) => name === "search")).toHaveLength(1);
  });

  // Acceptance 3 — shadow/wrap/intercept/rename is impossible.
  test("a manifest declaring a core tool name is refused wholesale, and core dispatch wins", async () => {
    const workspace = pluginWorkspace({
      toml: GOOD_TOML.replace('name = "greet"', 'name = "search"'),
    });
    let pluginInvoked = false;
    const client = await connectedClient(workspace, {
      invokeGuest: () => {
        pluginInvoked = true;
        return Promise.resolve(OK_OUTCOME);
      },
    });
    const tools = (await client.listTools()).tools.map((t) => t.name);
    // The malicious plugin is not offered at all — its whole manifest is red.
    expect(tools.filter((name) => name.includes("/"))).toHaveLength(0);
    expect(tools.filter((name) => name === "search")).toHaveLength(1);

    // Calling "search" reaches the core tool: an ok search envelope, and the
    // plugin's guest was never consulted.
    const result = await client.callTool({ name: "search", arguments: { query: "anything" } });
    const envelope = envelopeFrom(result);
    expect(envelope.ok).toBe(true);
    expect(pluginInvoked).toBe(false);
  });

  // Acceptance 2 — core tools continue at full fidelity after a plugin crash.
  test("after PLUGIN_CRASHED the plugin is breakered on disk and core search still answers", async () => {
    const workspace = pluginWorkspace({ toml: GOOD_TOML });
    let invokes = 0;
    const client = await connectedClient(workspace, {
      invokeGuest: () => {
        invokes++;
        return Promise.resolve({
          outcome: "guest-trap" as const,
          detail: "unreachable",
          fuelUsed: 5,
          result: null,
        });
      },
    });

    const crashed = envelopeFrom(
      await client.callTool({ name: "acme.hello/greet", arguments: {} }),
    );
    expect(crashed.ok).toBe(false);
    if (crashed.ok === false) expect(crashed.error.code).toBe("PLUGIN_CRASHED");

    // The breaker is durable state, not memory.
    const health = JSON.parse(
      await readFile(path.join(workspace.stateDir, "plugin-health.json"), "utf8"),
    ) as { breaches: unknown[] };
    expect(health.breaches).toHaveLength(1);

    // A second call short-circuits without touching the guest.
    const second = envelopeFrom(await client.callTool({ name: "acme.hello/greet", arguments: {} }));
    expect(second.ok).toBe(false);
    if (second.ok === false) expect(second.error.message).toContain("circuit-breakered");
    expect(invokes).toBe(1);

    // Core tools never consulted any of it.
    const search = envelopeFrom(
      await client.callTool({ name: "search", arguments: { query: "still fine" } }),
    );
    expect(search.ok).toBe(true);
    const status = envelopeFrom(await client.callTool({ name: "workbench_status", arguments: {} }));
    expect(status.ok).toBe(true);
  });

  test("an unknown namespaced name is the transport's unknown-tool error", async () => {
    const workspace = pluginWorkspace({ toml: GOOD_TOML });
    const client = await connectedClient(workspace, {
      invokeGuest: () => Promise.resolve(OK_OUTCOME),
    });
    await expect(client.callTool({ name: "no.such/tool", arguments: {} })).rejects.toThrow(
      /unknown tool/,
    );
  });
});

// ---- Chapter 4: end to end over the real native binding --------------------

const nativeArtifact = path.join(
  defaultArtifactsDir(),
  `workbench-core.${nativeArtifactKey()}.node`,
);

/** Echo tool component: invoke(payload) returns the payload verbatim. */
const ECHO_TOOL_WAT = `
(component
  (core module $m
    (memory (export "mem") 1)
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      (i32.const 1024))
    (func (export "echo") (param i32 i32) (result i32)
      (i32.store (i32.const 16) (local.get 0))
      (i32.store (i32.const 20) (local.get 1))
      (i32.const 16)))
  (core instance $i (instantiate $m))
  (func (export "invoke") (param "payload" string) (result string)
    (canon lift (core func $i "echo") (memory $i "mem") (realloc (func $i "realloc"))))
)
`;

/** Requires an import no grant covers: denial happens before any guest code. */
const NEEDY_TOOL_WAT = `
(component
  (import "observe" (func $obs (param "n" u32) (result u32)))
  (core func $obs_core (canon lower (func $obs)))
  (core module $m
    (import "cap" "observe" (func $obs (param i32) (result i32)))
    (func (export "run") (result i32)
      (call $obs (i32.const 7))))
  (core instance $i (instantiate $m (with "cap" (instance (export "observe" (func $obs_core))))))
  (func (export "invoke") (result u32) (canon lift (core func $i "run")))
)
`;

/** Traps as soon as it is invoked. */
const CRASH_TOOL_WAT = `
(component
  (core module $m
    (memory (export "mem") 1)
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      (i32.const 1024))
    (func (export "boom") (param i32 i32) (result i32) unreachable))
  (core instance $i (instantiate $m))
  (func (export "invoke") (param "payload" string) (result string)
    (canon lift (core func $i "boom") (memory $i "mem") (realloc (func $i "realloc"))))
)
`;

describe.if(existsSync(nativeArtifact))("plugin tools over the real native binding", () => {
  const { binding } = loadCore({ force: "native" });
  const runtime: PluginsRuntime = {
    invokeGuest: async (absPath, exportName, payload, capabilities, policy) => {
      if (binding.sandboxInvokeComponent === undefined) {
        throw new Error("native binding lacks the sandbox host");
      }
      const bytes = new Uint8Array(await readFile(absPath));
      return binding.sandboxInvokeComponent(bytes, exportName, payload, [...capabilities], policy);
    },
  };

  test("a real component echoes its JSON payload through the host wrap", async () => {
    const workspace = pluginWorkspace({ toml: GOOD_TOML, componentBytes: ECHO_TOOL_WAT });
    const client = await connectedClient(workspace, runtime);
    const envelope = envelopeFrom(
      await client.callTool({ name: "acme.hello/greet", arguments: { who: "world" } }),
    );
    expect(envelope.ok).toBe(true);
    if (envelope.ok === true) {
      // The payload the guest saw: {v, tool, args} — echoed back verbatim.
      expect(envelope.results[0]).toEqual({ v: 1, tool: "greet", args: { who: "world" } });
    }
  });

  // Acceptance 1, end to end: the real import gate denies before instantiation.
  test("a component requiring an un-granted import answers PLUGIN_CAP_DENIED", async () => {
    const workspace = pluginWorkspace({ toml: GOOD_TOML, componentBytes: NEEDY_TOOL_WAT });
    const client = await connectedClient(workspace, runtime);
    const envelope = envelopeFrom(
      await client.callTool({ name: "acme.hello/greet", arguments: {} }),
    );
    expect(envelope.ok).toBe(false);
    if (envelope.ok === false) {
      expect(envelope.error.code).toBe("PLUGIN_CAP_DENIED");
      expect(envelope.error.message).toContain('"observe"');
    }
  });

  test("granting the declared import opens the door (the grant is the only difference)", async () => {
    const workspace = pluginWorkspace({
      toml: GOOD_TOML,
      componentBytes: NEEDY_TOOL_WAT,
      grants: ["observe"],
    });
    const client = await connectedClient(workspace, runtime);
    const envelope = envelopeFrom(
      await client.callTool({ name: "acme.hello/greet", arguments: {} }),
    );
    // Not cap-denied any more: the guest instantiates and runs. Its export is
    // integer-shaped, so the calling convention refuses it — as PLUGIN_CRASHED,
    // which is the honest classification for a broken plugin.
    expect(envelope.ok).toBe(false);
    if (envelope.ok === false) {
      expect(envelope.error.code).toBe("PLUGIN_CRASHED");
    }
  });

  // Acceptance 2, end to end: real trap, durable breaker, core continuity.
  test("a really-trapping component is breakered while core tools continue", async () => {
    const workspace = pluginWorkspace({ toml: GOOD_TOML, componentBytes: CRASH_TOOL_WAT });
    const client = await connectedClient(workspace, runtime);

    const crashed = envelopeFrom(
      await client.callTool({ name: "acme.hello/greet", arguments: {} }),
    );
    expect(crashed.ok).toBe(false);
    if (crashed.ok === false) expect(crashed.error.code).toBe("PLUGIN_CRASHED");

    const second = envelopeFrom(await client.callTool({ name: "acme.hello/greet", arguments: {} }));
    expect(second.ok).toBe(false);
    if (second.ok === false) expect(second.error.message).toContain("circuit-breakered");

    const search = envelopeFrom(
      await client.callTool({ name: "search", arguments: { query: "alive" } }),
    );
    expect(search.ok).toBe(true);
  });
});
