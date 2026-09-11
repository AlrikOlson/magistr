# Phase 1 Data Model: LSP Resolver Plugin

No new persistent stores. The feature extends three existing records additively and
introduces two in-flight (never persisted) shapes.

## Extended: InstalledPluginRecord (`.workbench/plugins.json`)

Existing fields used, no schema change: `tier: "blessed"` is the gate for the
subprocess grant; `grants` must include the subprocess WIT import name for the
resolver plugin. Validation rule (new, host-side): the subprocess import is honored
only when `tier === "blessed"` — grant records on non-blessed plugins for this import
are ignored with a note, denied pre-instantiation (`PLUGIN_CAP_DENIED`).

## Extended: PluginHealthRecord (`.workbench/plugin-health.json`)

Existing shape `{ plugin, componentHash, reason, at }` reused verbatim for plugin
crashes. Additive entry kind for per-server breakers:

- **Server health entry**: `{ plugin, componentHash, reason, at, server: <spec-id> }`
  — `server` names the detection-table spec whose process crashed/hung/answered
  malformed data. Absent `server` = whole-plugin breach (today's semantics).

State transitions (per plugin AND per server spec, independently):

```
healthy --crash/timeout/malformed--> unhealthy (record appended)
unhealthy --component hash changes--> healthy (existing clear semantics)
unhealthy --bounded lazy retry succeeds--> healthy (retry never blocks a request)
unhealthy --retry budget exhausted--> unhealthy (short-circuit, no engine touch)
```

Invariants:
- Consulted per request from disk; no in-memory session state (statelessness suite).
- One server's `unhealthy` never affects another server's entries or the plugin's
  other capabilities (spec edge case: per-language independence).
- While any relevant entry is unhealthy, the answer lane is symbolic — never a wait.

## Extended: resolve results (`ResolverPort` v2, additive)

`SourceLocation` edges gain provenance:

- **provenance**: `"resolver-lsp" | "tier1-symbols" | "tier1-imports" | "tier1-text"`
  (extends the existing references-lane vocabulary; exact strings pinned by test)
- **confidence**: existing `"high" | "medium" | "low"` vocabulary

Validation rules:
- A `resolver-lsp` edge exists only if a healthy resolver produced it in this request.
- Mixed answers are legal; provenance is per-edge (spec US3 scenario 3).
- Every resolver-provided path passes workspace-jail validation before enveloping
  (FR-009); a jail rejection marks the answer malformed → health record, symbolic
  fallback for the whole request.

## New (in-flight only): LanguageServerSpec (detection table)

```
{ specId, languageId, binary, args, transport: "stdio", source: "detected" | "test-seam" }
```

- Host-curated: produced by `detect.ts`, never guest-supplied (research D2).
- The guest addresses servers by `specId`/`languageId` only.
- The test seam registers fake servers; `source` distinguishes it in notes/logs.

## New (in-flight only): subprocess capability session

```
{ specId, pid?, state: starting | ready | dead, stdio: framed pipes }
```

- Lifecycle owned by the host component lane; hard caps: one live server per spec,
  bounded spawn rate, kill on plugin teardown/fuel exhaustion/timeout.
- Never serialized, never crosses the envelope.

## Relationships

```
InstalledPluginRecord (tier=blessed, grants ⊇ subprocess import)
    └── enables → subprocess sessions (per LanguageServerSpec)
                        └── failures append → PluginHealthRecord (server-scoped)
                                    └── consulted by → resolver-plugin adapter
                                                └── labels → resolve edges (provenance)
```
