# Workbench MCP — Software Design Document

| | |
|---|---|
| **Version** | 1.4 |
| **Date** | August 21, 2026 |
| **Status** | Proposed — supersedes v1.0-draft in full |
| **Scope** | v1 of the Workbench MCP server, the plugin SDK, and the companion routing skill |
| **Companion docs** | `beyond-bash-brainstorm.md` (landscape & concept), `workbench-architecture-rigor-review.md` (rework record R1–R12), Appendix A (decision record incl. stakeholder session D1–D8), Appendix B (v2 fleet profile sketch) |

---

## 1. Purpose

Workbench is an MCP server that fully replaces the bash/grep/glob/sed tool loop in coding agents with seven typed tools: `search`, `map`, `read`, `edit`, `files`, `run`, and `script`, plus `undo`. It exists because the shell's *interface* — flat unranked text dumps, blind byte-level substitution, quoting hazards, exit-code errors — is hostile to language models even though its *engines* (ripgrep, coreutils) are excellent. Workbench keeps the engines and replaces the interface.

The design is governed by five laws. The first four derive from 2026 field evidence (§2); the fifth was added by the v1.1 architecture review to make the first four survivable over years of maintenance and an open ecosystem:

1. **Match grep's cost structure.** Zero config, no mandatory daemon, always fresh, degrades gracefully. Any fixed cost the shell doesn't have is a reason agents will fall back to the shell.
2. **Rank, don't dump.** ~49% of agent tool calls are search; execution latency is 0.4% of wall-clock. The win is output shape — ranked, grouped, deduped, budgeted — not speed.
3. **Verified writes.** A syntactically broken write is rejected *in the same tool call*. The agent never discovers a busted file three turns later.
4. **Narrow, typed escape hatches.** Project commands run through a declared allowlist (`run`); ad-hoc programmatic manipulation runs through a typed sandbox (`script`). There is no general shell anywhere.
5. **Every contract is versioned and negotiable; every extension runs sandboxed.** The envelope, tool schemas, config, index, journal, IPC, and plugin interfaces each carry a version and a written evolution rule. Third-party code executes only inside the capability sandbox, and the server process never opens a network connection.

### 1.1 Non-goals (v1 refusals)

- **No general shell execution.** `run` executes declared project commands only. Text/file munging via `run` is rejected with a redirect to `search`/`edit`.
- **No mutation of the core tool surface.** The seven tools plus `undo` are closed and immutable: no plugin may shadow, wrap, intercept, or rename them. Plugins may *add* tools only under a mandatory namespace (§8.3), and every plugin tool's output is host-wrapped in the standard envelope.
- **No embeddings or vector search in v1.** The three-modality search surface (lexical/structural/references) covers the load-bearing cases without index-staleness and false-negative risk. The `search.mode` enum reserves `semantic` (rejected with a clear error) so the API won't break when it lands post-v1. Revisit trigger: >20% of zero-hit searches in dogfood telemetry judged semantically answerable.
- **No graph database.** SQLite only. Neo4j/Kuzu/FalkorDB-class dependencies contradict law #1.
- **No LSP requirement.** Tree-sitter is the spine. LSP is a lazy, optional, background *upgrade* delivered as the first first-party plugin on the public SDK (§8.3); if it never becomes healthy, every tool still works at tree-sitter fidelity.
- **No multi-replica fleet in v1.** The HTTP profile is single-replica per workspace (§5.2). True fleet deployment is designed (Appendix B) but explicitly not built in v1; nothing in v1 forecloses it, and no v1 code may assume more than one server instance per workspace.
- **No deprecated MCP surface.** No Sampling, Roots, Logging, or HTTP+SSE (all deprecated in the 2026-07-28 spec). Long-running work uses the Tasks extension; mid-call interaction uses MRTR.
- **No cloud, no code egress, no server-side network.** All indexing and search is local to wherever the server runs. The server and engine processes never open outbound connections — plugin installation, updates, and crash-report upload all live in the CLI and run only on explicit user action. Telemetry is opt-in and stays on disk.

---

## 2. Grounding (evidence this design answers)

- **Entire's instrumented study**: search is ~49% of tool calls; a 10× search-latency improvement moved end-to-end task time by ~4%; ranking improvements (definitions first, source before tests/vendor, grouped+trimmed output) raised Hit@1 from 26%→34% and improved full-run outcomes. → Laws #2, ranking §7.4.
- **The "three modalities" decomposition** (lexical / structural / graph) and Sourcegraph's retreat from embeddings to BM25F-over-code-graph. → `search` design §9.1.
- **The grep cost-structure defense** (zero fixed cost, benign failures, no staleness — vs. LSP crashes and index rot). → Law #1, cold-start design §7.3, crash-domain architecture §6.
- **Local-first code graphs winning the market** (CodeGraph ~47k stars, GitNexus ~42k; 58–70% fewer tool calls) — pre-computed structure, on-device, over MCP. → Indexer §7.3, with SQLite instead of a graph DB.
- **Symbol-level and structural editing** (Serena LSP-editing, ast-grep rewrites, Morph fast-apply ~10.5k tok/s). → `edit` design §9.4.
- **Code Mode** (Cloudflare, FastMCP 3.1): agents write better code than tool-call chains; bash is an untyped code mode. → `script` design §9.7.
- **MCP 2026-07-28 spec** (stateless protocol core, MRTR, Tasks extension, cacheable list results, header-based routing). → Protocol layer §5.
- **The VS Code / browser-extension precedent**: ecosystems succeed when the platform's own features are built on the public extension API, and when untrusted extensions are sandboxed by construction rather than by review. → Extension architecture §8, gate M5 §13.
- **Stakeholder design session, 2026-08-20** (decisions D1–D8, recorded in Appendix A): single-replica v1 topology, full public SDK in v1, WASM capability sandbox, day-1 contract versioning, child-process engine, doctor + opt-in crash reports, signed registry + sideload, layered config with org policy.

---

## 3. Technology stack (August 2026)

All versions are "current stable at time of writing"; the repo pins exact versions in `bun.lock` / `Cargo.lock`.

| Layer | Choice | Rationale |
|---|---|---|
| Runtime | **Bun** (stable channel) | First-class TS execution, `bun:sqlite` built in, single-file binary compilation (`bun build --compile`) for the zero-install path, ~2–4× Node throughput on I/O-heavy workloads. The port layer (§6.4) keeps the core runtime-agnostic if the Bun bet ever needs revisiting. |
| Language | **TypeScript**, typechecked with the native compiler (`tsgo`) in CI | Boring-by-August-2026; native typecheck keeps CI fast. |
| MCP framework | **Official `@modelcontextprotocol/sdk`** (Tier-1 TS SDK), targeting the **2026-07-28 revision** | MRTR, Tasks extension, stateless HTTP profile, cacheable lists all land here first. |
| Native core | **One Rust crate `workbench-core`** exposed via **napi-rs**, prebuilt for linux-x64/arm64, darwin-arm64, win-x64; loaded **only by the engine child process** (§6.2) | Wraps `tree-sitter` (parsing, ~12 bundled grammars), the `grep` crate (ripgrep's engine as a library), and `ast-grep` (structural match/rewrite). One build matrix, one FFI boundary. A `wasm32-wasip1` build of the same crate ships in the same package as the engine child's fallback if the native addon fails to load. |
| Index store | **SQLite** via `bun:sqlite`, WAL mode on local filesystems (TRUNCATE journal on detected network FS, §5.2), FTS5, `PRAGMA user_version` on every database | Zero-dependency, single file (`.workbench/index.db`), survives restarts, versioned and migratable (§10.2). |
| File watching | Behind `WatcherPort` (§6.4): Bun's native `fs.watch` adapter + polling fallback adapter; debounced incremental re-index | Freshness by construction (§7.3.3); platform quirks isolated to adapters. |
| Sandbox substrate | **QuickJS-ng compiled to WASM** for `script`, plus the **Wasmtime component-model host** for plugins — one Wasmtime instance policy: fuel metering, memory caps, capability-object imports only | One sandbox engine, one escape suite, two guests (`script` programs and SDK plugins). No ambient FS, no network, deterministic kill. (§8.1, §9.7) |
| Plugin interfaces | **WIT (WASM component model)**, versioned per-interface | `workbench:lang-pack`, `workbench:resolver`, `workbench:runner`, `workbench:tool`, `workbench:ranking` (§8.2). |
| Formatting hook | **Biome** (bundled) with Prettier fallback via project config detection | Post-edit auto-format (§9.4). |
| Diff/patch | Pure-TS unified-diff implementation in the envelope layer | Diffs are a public output format (§4.2), kept dependency-light. |
| Signing & provenance | Sigstore-style keyless signing for releases and registry packages; SLSA-style build attestations; SBOM per release | Supply-chain integrity for a product whose prebuilt native binaries sit next to millions of users' source code (§11.4). |

**Riskiest dependency, named:** Bun's Node-API compatibility with `napi-rs`. The blast radius is now confined to the engine child (§6.2): if the native addon fails to load, the child selects the WASM build of `workbench-core` automatically; a startup self-test exercises parse/search/rewrite before the server reports ready; and a child-process crash at any later point degrades service to Tier-0 rather than ending it.

---

## 4. System overview

```
             ┌─────────────────────────────────────────────────────────────┐
             │                 workbench-mcp  (server process)             │
 stdio ─────►│ ┌────────────┐ ┌───────────────────────────────────────┐    │
 (local)     │ │ Protocol   │ │              Tool Layer               │    │
             │ │ 2026-07-28 │ │  search map read edit files run       │    │
 Streamable ►│ │ MRTR/Tasks │ │  script undo  ·  <ns>/plugin tools    │    │
 HTTP        │ │ negotiate  │ └──────┬──────────────────┬─────────────┘    │
 (1 replica) │ └────────────┘        │   PORT LAYER     │  (import-lint)   │
             │   ┌──────────────┐ ┌──▼───────┐  ┌───────▼──────┐           │
             │   │ Config       │ │ Query    │  │ Write Engine │           │
             │   │ 5-layer merge│ │ Engine   │  │ verify→format│           │
             │   └──────────────┘ │ rank+env │  │ +ws lock     │           │
             │   ┌──────────────┐ └──┬───────┘  └───┬──────────┘           │
             │   │ Journal      │    │              │                      │
             │   │ (durable,    │◄───┼──────────────┤                      │
             │   │  versioned)  │    │   EnginePort / IndexPort            │
             │   └──────────────┘    │   (versioned IPC, framed pipes)     │
             │  Security jail +      │              │                      │
             │  redaction at the ────┼──────────────┼── single egress      │
             │  envelope boundary    │              │                      │
             └───────────────────────┼──────────────┼──────────────────────┘
                                     ▼              ▼
             ┌─────────────────────────────────────────────────────────────┐
             │        workbench-mcp --engine  (supervised child)           │
             │  workbench-core (Rust napi / wasm fallback):                │
             │    tree-sitter (native, 12 bundled grammars)                │
             │    grep crate · ast-grep                                    │
             │  Indexer + Watcher (Tasks-integrated)                       │
             │  SQLite index writes (single writer, one process)          │
             │  Wasmtime extension host:                                   │
             │    script guest (QuickJS-ng) · plugin guests (WIT)          │
             │    third-party WASM grammars (long tail)                    │
             │  crash ⇒ supervised restart; service degrades to Tier-0,   │
             │  never dies                                                 │
             └─────────────────────────────────────────────────────────────┘
```

### 4.1 Component responsibilities

- **Protocol layer** — MCP 2026-07-28 request handling over stdio and single-replica Streamable HTTP; envelope-version negotiation via `_meta` (§4.2); MRTR orchestration for confirm flows; Tasks-extension lifecycle for background indexing; `ttlMs`/`cacheScope` on list results.
- **Port layer** — the enforced seam between the tool layer and all machinery (§6.4). Tools depend only on ports; adapters implement them; an import-graph lint in CI makes the dependency rule mechanical rather than aspirational.
- **Query engine** — executes the three search modalities through `EnginePort`/`IndexPort`, applies the ranking pipeline, emits ResponseEnvelopes.
- **Write engine** — the single mutation choke point (§7.5), serialized by the per-workspace write lock (§7.7): dry-run diff → (MRTR confirm) → parse-verify → apply → format → re-verify → journal → report new diagnostics.
- **Engine child** — a supervised child process (same binary, `--engine` mode) owning everything that touches untrusted bytes with native code: parsing, indexing, structural rewrite, the watcher, SQLite index writes, and the Wasmtime extension host (§6.2).
- **Extension host** — loads, meters, and capability-gates WASM plugin components and the `script` guest (§8).
- **Config subsystem** — five-layer merge with a tighten-only security lattice and a `doctor`-inspectable resolution trace (§7.2).
- **Journal** — durable, versioned, append-only op log with before/after blobs; powers `undo`; per-workspace (§7.6).
- **Security jail** — path canonicalization, allowlist enforcement, protected paths, secret redaction at the envelope boundary, key management, resource ceilings. Wraps every component including plugins; nothing reaches the FS except through it (§7.1).
- **Operations plane** — local tracing ring buffer, `workbench doctor`, opt-in crash reporting (§11).

### 4.2 The ResponseEnvelope (the load-bearing contract)

Every tool — core or plugin — returns the same envelope. It is a **one-way door**, so it is versioned from its first release and specified before any tool internals. It doubles as the MCP `outputSchema` (structured tool output) for every tool.

```jsonc
{
  "v": 1,                           // envelope schema version — REQUIRED, day 1
  "ok": true,
  "freshness": "warm",              // "cold" | "warming" | "warm"  (§7.3.3)
  "budget": { "requested": 4000, "used": 3180 },   // tokens, estimated
  "results": [ /* tool-specific, ALWAYS ranked & grouped (see per-tool) */ ],
  "truncation": {                   // present iff results were cut — never silent
    "dropped": 37,
    "reason": "budget",
    "narrow": ["kind:function", "path:src/**"],    // typed refinement hints
    "cursor": "eyJvZmZzZXQiOjUw..."                // resumable continuation
  },
  "notes": ["index warming: 62% of repo parsed; structural results may be incomplete"]
}
// or, on failure:
{
  "v": 1,
  "ok": false,
  "error": {
    "code": "AMBIGUOUS_SYMBOL",     // machine-readable taxonomy (§4.3)
    "recovery": "narrow",           // "retry" | "narrow" | "escalate" | "confirm" | "abort"
    "message": "3 symbols named `authenticate`",
    "candidates": ["UserService.authenticate", "AdminService.authenticate", "test helper authenticate"]
  }
}
```

**Versioning & negotiation.** Evolution within a major envelope version is strictly additive (new optional fields only); anything else bumps `v`. Clients may declare `workbench/maxEnvelopeVersion` and feature flags in `_meta` on any request — statelessly, per request, consistent with §5.1; the server responds in the highest mutually supported version and mirrors the resolved version in `v`. A client that declares nothing gets the latest, which is correct for freshly-prompted agents; harnesses that pin get stability. Tool input/output schemas are generated from `schemas/` (one source of truth) and carry a `schemaVersion` in tool annotations with the same additive rule. The deprecation policy is published before 1.0 and is expressed in releases, not calendar time: a deprecated surface survives at minimum through the next two minor releases and one major, warning in `notes` on every use.

Rationale: the Entire result (§2) shows output shape is where the tokens go. Budgeting is a *parameter*, truncation is a *manifest*, every error tells the model what class of next action recovers, and the version field is what lets all of that evolve for years without breaking the agents already depending on it.

### 4.3 Error taxonomy

`PATH_OUTSIDE_WORKSPACE`, `PROTECTED_PATH`, `AMBIGUOUS_SYMBOL`, `SYMBOL_NOT_FOUND`, `FILE_NOT_FOUND` (the addressed file does not exist in the workspace — the §7.1 jail admits nonexistent in-root paths by design, so ENOENT surfaces at the tool layer; recovery `narrow`, revise the path; v1.2 amendment), `PARSE_REJECTED` (edit would break syntax; includes parser message + rejected diff), `STALE_ANCHOR` (file changed since the diff was generated; includes fresh content hash), `PATTERN_INVALID`, `BUDGET_TOO_SMALL`, `COMMAND_NOT_DECLARED`, `RUN_REDIRECTED` (attempted text/file munging or shell execution via `run` — a shell operator, a shell as the program, or a munging program in a program position, §9.6/§1.1; distinct from `COMMAND_NOT_DECLARED` because a munging command can *be* declared by a human, and one-code-one-recovery forbids overloading; recovery `abort`, the message redirects to `search`/`edit`; v1.5 amendment, chunk:m3-run-tool), `SANDBOX_FUEL_EXCEEDED`, `SANDBOX_CAP_DENIED`, `SANDBOX_MEMORY_EXCEEDED` (the script breached its linear-memory cap and was terminated deterministically, §8.1 — a distinct outcome from fuel exhaustion and from capability denial, which one-code-one-recovery forbids folding together; recovery `narrow`, reduce the working set; v1.4 amendment, chunk:m4-script-sandbox), `INDEX_UNAVAILABLE` (answer degraded to cold tier; an error only when the mode strictly requires the index), `CONFIRM_REQUIRED`, `CONFIRM_MISMATCH` (confirmation token didn't match the offered diff), `UNSUPPORTED_VERSION` (client/server or on-disk format mismatch; recovery `abort`), `ENGINE_UNAVAILABLE` (engine child down/restarting; recovery `retry`), `ENGINE_BUSY` (bounded queue full; recovery `retry`, payload carries suggested backoff), `INPUT_QUARANTINED` (the input file repeatedly crashed the engine child and is quarantined from parsing, §6.2 — a deterministic local refusal until the file changes, so retry cannot help; recovery `escalate`, a human inspects the quarantine record via `doctor` and fixes or removes the file; v1.3 amendment), `PLUGIN_CAP_DENIED` (plugin exceeded its granted capabilities), `PLUGIN_CRASHED` (plugin trapped or exhausted fuel; names the plugin), `CONFIG_POLICY_DENIED` (org policy forbids the requested action; recovery `abort`, message names the policy layer). Each code maps to exactly one `recovery` class; that invariant is a schema test.

---

## 5. Protocol layer

### 5.1 Spec target

MCP revision **2026-07-28**. Consequences adopted deliberately:

- **Stateless core.** No `initialize` handshake dependency; each request self-describes via `_meta` (including envelope-version declaration and client/session identity, §7.6). All server state lives in SQLite, the journal, or config — **no in-memory session state anywhere** (enforced by the import-lint rule plus a CI test that round-robins requests across two server processes sharing one *local-disk* workspace). Statelessness in v1 buys transparent restart, trivial crash recovery, and an additive path to the v2 fleet profile — not multi-replica operation, which v1 explicitly refuses (§1.1).
- **MRTR (Multi Round-Trip Requests)** implement the dry-run→confirm contract (§9.4) *inside one tool call* via `resultType: "input_required"`. Fallback for non-MRTR clients: the same flow as two calls (`edit` returns a `confirmToken`; a second `edit` call with `{confirm: token}` applies). The token is an HMAC over (diff hash, file content hash) using the per-workspace key (§7.1); a stale confirm fails closed with `STALE_ANCHOR`.
- **Tasks extension** (`io.modelcontextprotocol/tasks`): background index builds and repo-wide structural rewrites run as tasks with `tasks/get` polling and progress updates.
- **Cacheable lists.** `tools/list` carries a long `ttlMs` (the core surface is static per version; the list version-bumps when plugin tools are installed/removed); `map` results carry a short `ttlMs` keyed to index generation.

### 5.2 Transports and topology

- **stdio** — the default local profile: `bunx workbench-mcp .` (or the compiled single binary `workbench-mcp .`).
- **Streamable HTTP, single replica per workspace** — for cloud sandboxes and remote use: exactly one server instance owns a workspace at a time, enforced by an advisory lock in `.workbench/`. A node hosting many workspaces runs one instance per workspace behind a thin supervisor/mux; `Mcp-Method`/`Mcp-Name` header routing works with zero gateway JSON parsing. Auth follows the spec's authorization framework (CIMD; no DCR).
- **Network-filesystem detection.** At startup the server detects whether `.workbench/` resides on a network filesystem (NFS, SMB, FUSE-cloud). If so: SQLite switches from WAL to TRUNCATE journal mode, concurrent instances are refused via the advisory lock, and every envelope's `notes` carries the degradation. SQLite WAL's shared-memory coordination is unsafe on network filesystems; the server never silently runs in a configuration that can corrupt its own index.
- **v2 fleet profile** — designed in Appendix B, not built in v1. Index ownership leaves the request path (per-replica index hydrated from content-addressed snapshots, or a dedicated index service); confirm-token keys move to a key service; the shared volume carries source only, never `.workbench`. Because v1 holds zero in-memory session state, promotion is additive.

---

## 6. Process architecture

### 6.1 Two roles, one binary

Workbench runs as two processes built from one binary, so version skew between them is impossible and the IPC protocol stays brutally simple.

**`workbench-mcp` (server)** owns the protocol layer, tool layer, query/write orchestration, config merge, journal, jail, and envelope/redaction boundary. It links **no code path that feeds untrusted bytes to a native parser**.

**`workbench-mcp --engine` (engine child)** owns everything in `workbench-core` — tree-sitter parsing, the grep engine, ast-grep rewrites — plus the watcher, the indexer, SQLite index *writes* (preserving the single-writer invariant inside a single process, where it is actually enforceable), and the Wasmtime extension host. Plugins execute here because they, too, parse untrusted bytes: a runaway plugin shares this crash domain instead of creating a new one.

### 6.2 Supervision and degradation

The server spawns and supervises the child: crash → automatic restart with exponential backoff; repeated crash on the same input quarantines the offending file from parsing rather than crash-looping. A request naming a quarantined path is refused locally with `INPUT_QUARANTINED` (recovery `escalate`, §4.3 v1.3 amendment), and the quarantine record's durable home is `.workbench/quarantine.jsonl` — appended on the strike that crosses the threshold, jail-protected (§7.1), and read by `doctor` (§11.2) without repo access. While the child is down, tools degrade exactly along the freshness model: lexical queries return via a conservative in-server fallback or `ENGINE_UNAVAILABLE` (recovery `retry`); envelopes explain in `notes`; the write engine refuses to apply unverifiable batches rather than applying blind. A grammar segfault is thereby converted from *total outage* — the worst possible failure for the agent's entire file substrate — into *one degraded answer* or *one rejected edit*.

### 6.3 IPC

A versioned, length-prefixed frame protocol over pipes: request-scoped, shared-nothing, carrying request IDs for cancellation propagation (§7.7). Perf gate: p95 round-trip overhead under 1 ms on a local pipe — noise against the §2 finding that execution latency is 0.4% of wall-clock, and CI-enforced so it stays noise.

### 6.4 Ports and adapters (the enforced seams)

The tool layer depends only on a fixed port vocabulary: `EnginePort` (parse/search/rewrite), `IndexPort`, `JournalPort`, `WatcherPort`, `SandboxPort`, `FormatterPort`, `ResolverPort`. Adapters implement them: the engine-child IPC adapter (production), the WASM fallback adapter (selected inside the child), and an in-process test adapter that keeps the statelessness and freshness suites fast. The dependency rule — protocol → tools → ports ← adapters, nothing reaches around a port — is enforced by an import-graph lint in CI, not by review vigilance.

This layer is why the rest of the document holds together: the child process is a deployment choice rather than an architecture; the §3 WASM fallback is an adapter swap; and the public SDK (§8) is substantially a re-export of the same port vocabulary — one interface language for first-party and third-party code, which is the property that lets internal seams go public without redesign.

---

## 7. Core subsystems

### 7.1 Security jail (wraps everything, including plugins)

- **Path discipline:** every path from any tool argument — or any plugin capability call — is canonicalized (symlinks resolved) and prefix-checked against the workspace root(s). Failures: `PATH_OUTSIDE_WORKSPACE`.
- **Protected paths:** `.git/**` (write), all config layers (§7.2), `.workbench/**` are read-only to the write engine, the sandbox, and every plugin at every trust tier — the server will not edit its own configuration, keys, or history, and the `run` allowlist therefore can't be self-amended by an injected agent or a malicious plugin.
- **Key management:** a per-workspace secret (32 bytes) is generated at `init`, stored under `.workbench/keys/` with `0600` permissions, used for the MRTR confirm-token HMAC, rotatable on demand via CLI (`workbench key rotate`). In the v2 fleet profile this is the one item that graduates to a key service (Appendix B).
- **Secret redaction at a single choke point:** the envelope layer of the server process, on the response path. Because *every* output — core tools, plugin tools, `script` return values, `run` output — passes through envelope construction, no surface can route around redaction by construction. Common credential patterns plus a high-entropy heuristic; redactions are marked `[REDACTED:aws-key]` so the model knows something was there. Configurable off per-workspace by a human — unless org policy forces it on (§7.2).
- **Resource ceilings:** per-request wall-clock timeout (default 10s), result ceilings, sandbox fuel/memory caps (per guest, including plugins), bounded work queues (§7.7), index size cap with explicit `notes` when hit.
- **Acceptance gate:** a red-team suite of ≥25 escape/injection attempts (symlink loops, `..` encodings, git-hook writes, config self-edit, redaction bypass) **plus a plugin chapter** (malicious grammar, capability escalation, write-queue bypass from a plugin tool, registry-spoof and signature-strip attempts) **plus a policy chapter** (lower config layers attempting to loosen policy-tightened keys) runs in CI; any success blocks release.

### 7.2 Configuration (five layers, tighten-only security lattice)

Configuration resolves through five layers, lowest precedence first: **built-in defaults** → **org policy** (admin-distributed via MDM or a fixed system path; read-only to everyone at runtime) → **user config** (`~/.config/workbench/config.toml`) → **workspace** `workbench.toml` → **per-session ephemeral flags**.

For security-relevant keys the lattice is tighten-only downward: policy can force redaction on, forbid sideloaded plugins, pin the `run` allowlist to an approved set, cap telemetry consent, or add protected paths — and no lower layer can loosen what a higher layer tightened (`CONFIG_POLICY_DENIED` on attempted use). Non-security keys (budgets, formatting, ranking preferences) merge nearest-wins. The merged effective config is inspectable via `workbench doctor config`, showing each key's value *and which layer decided it*. The config schema carries `config_version` (§10.2); unknown keys warn, never abort.

### 7.3 Indexer (runs in the engine child)

**7.3.1 Tiers.** Tier 0 (cold): nothing but the FS — lexical search runs directly via the grep crate; `map` returns directory skeleton only. Tier 1 (warming→warm): tree-sitter parse of every source file → symbols + imports + FTS5 content index. Tier 2 (optional): resolver-plugin-upgraded references precision (§8.3).

**7.3.2 Build.** On first start, the indexer registers a Tasks-extension task (`index/build`) and parses in parallel worker threads (Rust side, one arena per worker), committing in batches through the child's single SQLite writer. Targets, CI-enforced (law #1): useful at Tier 0 within 5 seconds; 500k LOC to Tier 1 in <60s on a laptop.

**7.3.3 Freshness by construction.** Watcher events debounce (75ms) into incremental re-parses. Additionally — because watchers lie — every query result is verified against file mtime+size before return; mismatch triggers a synchronous re-parse of the affected file(s) (bounded; falls back to labeling the result `cold` for that file). **No stale line number ever leaves the server.** Every envelope carries the freshness tier.

**7.3.4 What's indexed.** Source (all bundled-grammar languages: TS/JS/TSX, Python, Rust, Go, Java, C/C++, C#, Ruby, PHP, HTML/CSS, JSON/YAML/TOML, Markdown, Bash — read-only recognition), any language served by an installed language-pack plugin (§8.3), plus plain-text/FTS5 for *everything* not binary, so config, docs, and lockfiles are lexically searchable (grep parity). Junk pruning: `.gitignore` + built-in vendor/generated detection, both reflected in ranking (§7.4), never silently excluded from `mode:lexical` when the query targets them by path.

Gitignore fidelity is lane-dependent *by contract* (v1.4; narrowed in v1.6). The native walk (`ignore` crate) implements full semantics: nested `.gitignore` files, negations, anchored patterns. The degraded TS walk — `fallbackScanWorkspace`, used when the wasm core cannot scan, by the stub core, and by the Tier-0 `list()` lane in `main.ts` — honors **nested `.gitignore` files with compiled globs** (bare names match the basename at any depth below their `.gitignore`; `dir/` suffixes; `*`/`?`/`**` globs; slash-containing patterns anchored to their `.gitignore`'s directory). Negation (`!`) lines remain skipped — the one deliberate limit (v1.6 narrowed the gap from root-only after a 1.9GB nested-ignored spike artifact crashed the engine child on unscoped search and bloated the map census; see chunk:read-walker-gitignore). The wasm-guest zero-preopen rationale stands: the TS walk's fidelity was raised instead of preopening the workspace root. The worst case of the remaining gap is over-inclusion of `!`-rescued files' *absence* — a negation-rescued file is over-excluded in the degraded lane, never a path-targeted result: explicit-path reads never consult the walk. A test pins nested-honored/negation-skipped semantics so an accidental fidelity change is visible.

### 7.4 Ranking pipeline (the product)

Applied to all search results before enveloping, in order: (1) **kind classification** — declaration / call / import / comment / string, from tree-sitter when warm, heuristic when cold; (2) **file-class weighting** — source > tests > vendor/generated; (3) **definition-first ordering** within groups; (4) **dedup** — identical lines across N files collapse to one entry + count; (5) **grouping by file**, files ordered by best-hit score + path relevance; (6) **trim to budget** with the truncation manifest.

The combining model is **first-party, always**. Weights live in one tunable table; every change must land with a before/after MRR number from the eval harness (§11.1). Ranking plugins contribute *features* into step (5)'s scorer, never final orderings, and run in shadow mode until promoted (§8.3) — extending the no-number-no-merge rule to the ecosystem instead of losing it there.

### 7.5 Write engine (single mutation choke point)

Pipeline for *every* mutation regardless of originating tool (edit, files, script commits, undo): acquire the workspace write lock (§7.7) → resolve targets → generate dry-run diff → (confirm gate per §9.4) → apply to a shadow copy → **parse-verify** every touched file via `EnginePort` (`PARSE_REJECTED` on new syntax errors — the whole batch rolls back atomically; an engine-child crash during verify rolls back identically) → auto-format touched regions (Biome/detected formatter) → re-verify → journal (§7.6) → atomic swap into place → envelope reports the applied diff + any *new* diagnostics.

Non-source files (no grammar) skip parse-verify but still get: JSON/YAML/TOML well-formedness checks, journal, and confirm gates.

### 7.6 Journal & undo (durable, versioned, per-workspace)

Append-only SQLite journal: `(op_id, ts, tool, args_hash, session_id, files[], before_blob_ids[], after_blob_ids[])`, blobs content-addressed and zstd-compressed, **per-workspace** size cap (default 256MB, oldest-blob eviction with a note). Ops are tagged with the client/session identity supplied via `_meta` — identity travels on the request, state lives on disk, consistent with §5.1.

`undo(n=1)` defaults to undoing *the calling session's* ops; `scope:"workspace"` undoes globally — defined now because two agents sharing a workspace must never silently revert each other's work. Conflicts (a later op touched the same file) are detected by content hash and surfaced as a three-way choice in the envelope rather than silently clobbered. Deletes via `files` are journal-backed moves — nothing is ever unlinked destructively.

The journal is **durable-class data** (§10.2): format-versioned, forward-migrated at open, quarantined intact (never deleted) if unreadable, fail-closed under downgrade (`UNSUPPORTED_VERSION`).

### 7.7 Concurrency, cancellation, backpressure

The **workspace is the concurrency domain**. All mutations from any entry point — including a second Workbench process or the CLI — serialize through the per-workspace write lock (advisory lock file + SQLite transaction); the journal is the serialization witness; content-hash anchors (`STALE_ANCHOR`, `CONFIRM_MISMATCH`) close the read-modify-write races the lock doesn't.

MCP cancellation notifications propagate through the IPC layer to the engine child, which checks cancellation tokens between parse units and inside search iteration — a cancelled request actually stops burning the engine. Overload gets a typed answer instead of a timeout: bounded queues per subsystem and `ENGINE_BUSY` (recovery `retry`, suggested backoff in the payload).

---

## 8. Extension architecture (the public plugin SDK)

v1 ships a full public SDK (decision D2). The architecture's job is to make that compatible with the five laws: a plugin must never add fixed cost for users who have none (law #1), must never weaken the jail (law #5), and must never destabilize the envelope or the ranking discipline.

### 8.1 Execution substrate

Plugins are WASM components executed by the same Wasmtime host that runs `script` — one sandbox engine, one fuel/memory policy, one escape suite. Plugins execute inside the **engine child's** process space (they parse untrusted bytes), sharing the §6 crash domain rather than adding one. A plugin declares required capabilities in its manifest (`plugin.toml`: fs-read scopes, index query access, resource limits); the host grants at install time and enforces at runtime through capability objects — default deny, `PLUGIN_CAP_DENIED` on violation, `PLUGIN_CRASHED` on trap or fuel exhaustion (the plugin is circuit-breakered unhealthy; core tools continue at full fidelity).

### 8.2 Interface contracts

Interfaces are WIT (component model), versioned independently, evolved additively within a major:

| Interface | Surface |
|---|---|
| `workbench:lang-pack` | grammar + tag queries + file-class heuristics for a language |
| `workbench:resolver` | `resolveReferences`, `rename`, `diagnostics` — the Tier-2 upgrade path |
| `workbench:runner` | structured-failure parsers for `run` output |
| `workbench:tool` | new namespaced MCP tools |
| `workbench:ranking` | feature contributions to the ranking scorer |

The WIT vocabulary is deliberately a re-export of the internal port vocabulary (§6.4): first-party and third-party code speak the same language, and the SDK cannot drift from reality because first-party features are built on it (§8.3).

### 8.3 The five surfaces

**Language packs.** Grammars compiled to WASM plus tag queries. The twelve bundled grammars remain **native** in the engine child — the hot path for the overwhelming majority of parses is untouched, so the v1.0 kill of "runtime WASM grammar packs" (2–5× parse cost in the hottest path) stands unreversed. WASM grammars serve only the long tail the core team will never bundle, where 2–5× on a rare language beats no support. A **promotion path** exists: a signed, audited pack may be blessed to native execution inside the engine child — no new isolation mechanism needed, because the child is already a crash domain.

**Resolvers.** The Tier-2 interface, public from day one. The bundled **LSP plugin is the first first-party consumer of the public SDK** — not a private special case. It boots lazily after Tier-1 indexing, speaks stdio to project-detected language servers (via a declared subprocess capability, the one capability reserved to blessed plugins), and is circuit-breakered: any crash or timeout marks it unhealthy and everything continues at tree-sitter fidelity. Building it on the public SDK is the forcing function for SDK quality: the core team feels every rough edge before any third party does.

**Runner adapters.** Pure functions from command output to the structured failure format of §9.6. Smallest, safest surface; clearest community demand (every test framework ever). Known runners (bun/jest/vitest/pytest/cargo/go) ship as bundled first-party adapters on this same interface.

**Tools.** Registered under a mandatory namespace — `<publisher>.<pkg>/<toolname>` — with the seven core tools plus `undo` reserved and immutable (§1.1). Plugin tools declare schemas through the SDK; the host wraps every plugin tool's output in the standard ResponseEnvelope — budget accounting, truncation manifest, and the §7.1 redaction pass included — so the agent-facing contract stays uniform no matter who wrote the tool. Plugin tool *writes* go through the queued-mutation capability, which lands in the write engine's choke point like everything else.

**Ranking signals.** The tightest leash (§7.4): features only, first-party combiner, registry admission requires submitted eval-harness numbers, and locally a newly installed signal runs in **shadow mode** — logged to opt-in telemetry, not applied — until the user promotes it.

### 8.4 Distribution and trust

A first-party **signed registry**: sigstore-style keyless signing bound to publisher identity, a published review bar for the blessed tier, admission gated on the conformance kit (§8.5) plus a reduced red-team run. **Sideload always works** — this is a developer tool — but sideloaded plugins are labeled untrusted in `map` and `doctor`, receive the tightest capability defaults, and can be forbidden by org policy. The registry client lives **in the CLI** (`workbench plugin install|update|verify`); the server and engine processes never open a network connection — the no-egress law survives the ecosystem intact.

### 8.5 Conformance (TCK)

The SDK ships a plugin conformance kit: interface conformance, capability-violation probes, fuel/memory abuse tests, and (for language packs) a fuzz-corpus run. Registry admission runs the TCK; the SDK's own examples and all first-party plugins must pass it unmodified — that requirement is a release gate (M5, §13).

---

## 9. Tool surface

Seven core tools plus `undo`, immutable; plugin tools appear beside them under namespaces (§8.3). All share the ResponseEnvelope; all take an optional `budget` (tokens, default 4000). Schemas below show the essential shape (full JSON Schema in the repo's `schemas/` directory, from which the MCP `inputSchema`/`outputSchema` are generated — one source of truth, versioned per §4.2).

### 9.1 `search` — replaces grep, glob, find

```jsonc
{
  "query": "parseConfig",            // text | structural pattern | symbol name
  "mode": "auto",                    // auto | lexical | structural | references | files
                                     // ("semantic" reserved — rejected in v1)
  "kind": ["function","call"],       // optional structural filter
  "path": "src/**",                  // glob scope
  "include": { "tests": false, "vendor": false },  // ranking demotes by default; this excludes
  "context": 2,                      // lines of context per hit
  "budget": 4000
}
```

`mode:auto` escalation: lexical always runs (Tier 0 capable); if the query names a symbol and the index is warm, structural classification enriches and re-ranks; `references` requires warm index (else `INDEX_UNAVAILABLE` with `recovery:"retry"` + task progress in `notes`). `mode:files` is glob/filename search in the same envelope (kills `find`). Results: grouped by file, kind-tagged per hit, definitions first.

### 9.2 `map` — replaces ls -R and orientation greps

```jsonc
{ "path": ".", "depth": 3, "detail": "outline",   // skeleton | outline | full
  "budget": 6000 }
```

Returns pruned directory tree; per-file symbol outlines (tree-sitter tag extraction, compressed); import-graph summary (top fan-in/fan-out modules); detected entry points, project commands (feeds `run`), languages, index health, and the installed-plugin inventory with trust tiers (§8.4). `ttlMs` keyed to index generation for client caching.

### 9.3 `read` — replaces cat, head, tail, sed -n

```jsonc
{ "file": "src/auth/service.ts",
  "symbol": "UserService.authenticate",   // XOR lines: {start,end}
  "surround": "enclosing",                // none | enclosing (parent symbol) | imports
  "budget": 4000 }
```

Symbol addressing resolves via the index (ambiguity → `AMBIGUOUS_SYMBOL` with candidates). Returns the unit of meaning with precise line anchors + content hash (used by `edit` as a staleness anchor).

### 9.4 `edit` — replaces sed, awk, patch

**Verbs.**

```jsonc
// symbol verb (Serena-class)
{ "op": "replace_body" | "insert_after" | "insert_before" | "rename",
  "symbol": "UserService.authenticate", "file": "...", "content": "..." }
// structural verb (ast-grep-class, repo-wide)
{ "op": "rewrite", "pattern": "console.log($ARGS)", "rewritePattern": "logger.debug($ARGS)",
  "path": "src/**", "lang": "ts" }
// anchored string verb (fallback, non-code files)
{ "op": "replace_text", "file": "...", "old": "...", "new": "...", "occurrence": 1 }
```

**Contract.** All verbs run the write-engine pipeline (§7.5): workspace lock, dry-run diff, confirm, atomic parse-verified apply, format, journal, new-diagnostics report.

**Confirm gate.** Single-file symbol/text edits: auto-apply by default (parse-verify is the guard), diff returned in the envelope. Multi-file ops (`rewrite`, `rename`) and any op touching >1 file or >200 changed lines: **MRTR `input_required`** with the aggregated diff; applies only on confirmation. `autoApply:true` may be set per-call by trusted harnesses; the config layers (§7.2) can force confirmation for everything — and org policy can make that mandatory.

### 9.5 `files` — replaces mv, cp, mkdir, touch, rm

`{ "op": "move" | "copy" | "create" | "delete" | "mkdir", "from": "...", "to": "..." }`. `move` is **import-aware**: importer rewrites are computed (index) and included in the dry-run diff — inherently multi-file, so always confirm-gated. `delete` is a journal-backed move to `.workbench/trash/`, undoable.

### 9.6 `run` — the declared-command escape hatch

`{ "command": "test", "args": ["--filter","auth"], "timeout": 300 }`. Commands come from the merged config's `[commands]` table, seeded at init from lockfile/manifest detection, amendable only by a human, and governable by org policy (§7.2) — a centrally pinned allowlist is enforceable across a team. Output is *parsed*: known runners get failure extraction (test name, file, assertion, trimmed trace) via bundled runner adapters; installed runner-adapter plugins extend coverage (§8.3); unknown commands get head+tail with a truncation manifest. Undeclared command → `COMMAND_NOT_DECLARED`, `recovery:"abort"`, message pointing the human at their config layer.

### 9.7 `script` — Code Mode for the long tail

`{ "source": "<typescript>", "dryRun": true }`. Executes in QuickJS-ng-on-WASM under the Wasmtime host (§8.1): fuel-metered (deterministic CPU cap), memory-capped (default 256MB), no ambient FS/network/clock-of-consequence. The guest sees one global, `workspace`:

```ts
declare const workspace: {
  search(q: SearchArgs): SearchResult;      // same engine as the tools
  read(a: ReadArgs): ReadResult;
  glob(pattern: string): string[];
  edit: { queue(op: EditOp): void };        // queued, not applied
  files: { queue(op: FileOp): void };
};
```

Reads execute live (through the jail); **writes are queued**, and the script's return value plus the aggregated dry-run diff of the queue come back through the standard confirm gate (MRTR). `dryRun:true` (default) never applies. `script` is exactly as safe as `edit` — same choke point, same journal — while replacing bash's legitimate job: ad-hoc batch manipulation, with types and no quoting hell.

### 9.8 `undo`

`{ "steps": 1, "scope": "session" }` — session-scoped by default, `"workspace"` for global; see §7.6.

---

## 10. Data design

### 10.1 Schemas

```sql
-- .workbench/index.db  (WAL on local FS; TRUNCATE on network FS)  [cache-class]
PRAGMA user_version = 1;
CREATE TABLE files(id INTEGER PRIMARY KEY, path TEXT UNIQUE, lang TEXT,
  class TEXT CHECK(class IN ('source','test','vendor','generated','config','doc')),
  mtime INTEGER, size INTEGER, hash BLOB, parsed_gen INTEGER);
CREATE TABLE symbols(id INTEGER PRIMARY KEY, file_id INT REFERENCES files(id),
  name TEXT, qualname TEXT, kind TEXT, start_line INT, end_line INT, sig TEXT);
CREATE INDEX symbols_name ON symbols(name);
CREATE TABLE edges(src_symbol INT, dst_symbol INT, kind TEXT,  -- imports|calls|extends|references
  confidence TEXT CHECK(confidence IN ('ts','resolver')));      -- provenance of the edge
CREATE VIRTUAL TABLE content_fts USING fts5(path, body, tokenize='trigram');
-- journal.db  [durable-class]: ops(op_id, ts, tool, args_hash, session_id, ...)
--                              blobs(hash PRIMARY KEY, zstd BLOB)
-- telemetry.db (opt-in)  [cache-class]: queries(ts, tool, args_hash, hit_count, followed_by)
```

Multi-hop reference queries use recursive CTEs over `edges`; the closure table is a measured fallback, not a default. Acceptance: references p95 <50ms at 1M LOC (else the no-graph-DB refusal is revisited *with data*).

### 10.2 Data lifecycle (versioning, migration, upgrade/downgrade)

Every database and on-disk format is stamped (`PRAGMA user_version` / format-version key) and belongs to one of two classes with different obligations:

**Cache-class** (index, telemetry): rebuildable by any binary version. A newer format found by an older binary is discarded and rebuilt, never misread; migrations across minor versions run when cheaper than rebuild. Rebuild is always legal — the index is a cache of the filesystem, which is the truth.

**Durable-class** (journal, config): user data. Format-versioned; migrations are forward-only and run at open; an unreadable journal is quarantined intact with a `doctor` event, never deleted; an older binary encountering a newer journal fails closed with `UNSUPPORTED_VERSION` rather than guessing. Config: unknown keys warn, never abort.

All database replacements are atomic (write-new → fsync → rename). The migration runner and the upgrade/downgrade suite (§12) are part of M1, not an afterthought.

---

## 11. Observability & operations

### 11.1 Evaluation plane (tuning the product)

- **Eval corpus:** opt-in telemetry records (query, results, what the agent did next) to local JSONL. Zero-hit searches and immediate re-queries are auto-flagged — they *are* the ranking backlog.
- **Offline retrieval eval:** MRR / Hit@1 / Hit@5 against a labeled slice of the corpus; runs in CI. **Rule: no ranking-weight change merges without a before/after number** — extended to the ecosystem via shadow mode and eval-gated registry admission for ranking plugins (§8.3).
- **Agent benchmark:** 10 recorded end-to-end tasks (find-and-fix, rename-across-repo, orient-in-unknown-repo…) replayed against a live agent; tracks tool calls, tokens, wall-clock, success.
- **`map` exposes index health** (coverage %, watcher lag, generation, plugin inventory) so degradation is visible to the agent, not just to logs.

### 11.2 Operations plane (supporting the product)

- **Local structured tracing:** OTel-shaped spans (request → engine IPC → SQLite → envelope) written to an on-disk ring buffer with bounded size — the flight recorder. Never leaves the machine on its own.
- **`workbench doctor`:** probes the environment (filesystem type, watcher health, native-vs-WASM engine selection, plugin inventory and trust tiers), reports index health and the layered-config resolution with per-key provenance (§7.2), lists recent engine crashes and quarantined files, and can emit a **redacted diagnostic bundle** — a tarball the user can inspect before choosing to attach it to a support ticket. The bundle contains structure and symptoms, never file contents.
- **Opt-in crash reporting:** panic/abort metadata and stack hashes only — no paths, no code, ever. Consent is stored in config, where org policy can bound it (§7.2). Upload happens from the CLI on explicit action or recorded standing consent; the server never phones home.

### 11.3 Acceptance

A support engineer must be able to resolve a seeded corrupt-index scenario using only a `doctor` bundle — this is a release gate (M6, §13), because an ops plane that hasn't been exercised is documentation, not architecture.

### 11.4 Release engineering & supply chain

Signed releases for every artifact (binaries and npm packages) with SLSA-style build provenance attestations; an SBOM per release; continuous fuzzing of the bundled grammar corpus in CI, feeding the §6 crash-domain design with real crash data before users do. Updates are pulled, never pushed: `workbench self-update` verifies signatures, honors staged-rollout metadata and an emergency version blocklist published in registry metadata — checked by the CLI, never by the server. Fixes land on the current and previous minor release.

---

## 12. Testing strategy

Unit (ranking pipeline golden files; envelope schema property tests, including the one-code-one-recovery invariant), integration (each tool against fixture repos in 6 languages), **statelessness test** (two server processes on one local-disk workspace, alternating requests, byte-identical envelopes), **red-team suite** (§7.1 — jail, plugin, and policy chapters; release-blocking), **freshness fuzzer** (concurrent mutator process + query loop; any stale line number fails), **chaos suite** (kill the engine child mid-parse, mid-verify, mid-index; assert degraded-not-dead and rollback correctness), **concurrency suite** (two clients, interleaved mutations, no interleaved corruption; session-scoped undo semantics), **migration suite** (upgrade and downgrade across format versions; journal quarantine-and-recover), sandbox escape suite (WASI caps, fuel exhaustion, queue-bypass attempts — `script` and plugin guests), **plugin TCK** (§8.5, self-hosted by all first-party plugins), MCP conformance (official 2026-07-28 suite, both transports; MRTR + token-fallback flows; version negotiation), perf gates in CI (5s cold-start usefulness; 60s/500k LOC Tier-1; <50ms reference p95; <1ms IPC p95).

---

## 13. Build plan (gates, not dates)

By stakeholder directive, this plan contains no time estimates. It is a dependency graph of milestones; a milestone is done when its exit gate is green, and the graph — not a calendar — is the plan. M0 is deliberately the heaviest: the process split and the port layer cannot be retrofitted, and everything downstream stands on them.

**M0 — Foundations.** SDK wiring on 2026-07-28; both v1 transports with network-FS detection; versioned envelope + `_meta` negotiation; full error taxonomy; the jail with key management; the port layer with the import-graph lint; the two-process split with framed IPC and supervision; `workbench-core` napi+wasm CI matrix.
*Gate:* MCP conformance green on both transports · statelessness test green · red-team jail chapter green · engine-kill chaos test shows degraded-not-dead · IPC p95 <1ms.

**M1 — Read path.** *(depends: M0)* `search` (lexical + files) + `map` + `read`; Tier-0/1 indexer in the engine child; watcher; Tasks integration; migration runner and version stamping.
*Gate:* 5s cold-start usefulness · 60s/500k-LOC Tier-1 · freshness fuzzer green · migration suite green.

**M2 — Write path.** *(depends: M1)* Write engine; all `edit` verbs; journal + `undo` with session scoping; workspace write lock; MRTR confirm + token fallback on per-workspace keys.
*Gate:* parse-verify suite · concurrency suite · journal quarantine-and-recover test · statelessness re-run.

**M3 — Escape hatches.** *(depends: M2)* `files` (import-aware move) + `run` with the layered-config-governed allowlist; five-layer config merge + policy lattice.
*Gate:* red-team re-run including the policy chapter.

**M4 — Sandbox + SDK core.** *(depends: M2; parallel with M3)* `script` sandbox; the Wasmtime component host; WIT interfaces for all five surfaces; capability manifests and enforcement; the TCK.
*Gate:* sandbox escape suite incl. plugin probes · TCK self-hosted by the SDK's own examples · 10-one-liner expressiveness check for `script`.

**M5 — SDK proven.** *(depends: M4)* The LSP resolver rebuilt *as a plugin on the public SDK*; one first-party WASM language pack; one runner adapter on the public interface; shadow-mode ranking plumbing; blessing/promotion path exercised end-to-end.
*Gate:* all first-party plugins pass the TCK unmodified — **this gate is non-negotiable**; it is the system's main defense against freezing a wrong SDK for years.

**M6 — Operations + ecosystem.** *(depends: M3, M5)* `doctor` with diagnostic bundles; tracing ring buffer; opt-in crash reporting; registry with signing and TCK-gated admission; `self-update` with provenance verification; companion routing skill + CI drift check; eval harness live; dogfood release.
*Gate:* seeded corrupt-index scenario resolved from a `doctor` bundle alone · first ranking change lands with its MRR number · signature-strip attack fails in CI.

M1+M2 remain the usable daily driver, exactly as v1.0 intended; the public-SDK label ships when M5's gate is green.

---

## 14. Risks

| Risk | Sev | Mitigation |
|---|---|---|
| SDK interface freeze — public WIT mistakes carried for years | High | WIT per-interface versioning with additive evolution; first-party code forced through the public SDK before any third party (M5 gate); deprecation policy in releases |
| MRTR client support uneven (spec is weeks old) | High | Token-based two-call fallback is a first-class path, tested equally; rides the same version-negotiation machinery |
| Bun↔napi-rs compat breaks on a target | Med (was High) | Blast radius confined to the restartable engine child; WASM fallback auto-selected; startup self-test |
| WASM parse cost for third-party languages | Med | Bundled grammars stay native (hot path unchanged); blessing/promotion path to native for audited packs |
| Registry operational burden — a signed registry is a service commitment, not a feature | Med | CLI-side design: server never depends on registry availability; sideload as the permanent pressure valve |
| Tree-sitter references (name-resolution-free) give false positives vs. resolver truth | Med | `confidence` provenance on edges; resolver plugins upgrade precision; envelope never overclaims |
| Redaction false positives corrupt agent reads | Med | Marked redactions + per-workspace off-switch (policy-overridable); entropy threshold tuned on fixture corpus |
| Two-process complexity: supervision, IPC, partial-failure states | Low-Med | Same-binary lockstep; in-process test adapter; M0 chaos gate |
| SQLite write contention on huge monorepos | Low-Med | Single writer inside one process (engine child) + WAL + batch commits; measured gate at 1M LOC |
| Agents ignore the routing skill and misuse tools | Med | `recovery` hints in every error; telemetry shows misuse patterns; skill iterated from data |

## 15. Open questions (decided by dogfood data, not debate)

1. Should single-file `edit` default to confirm rather than auto-apply? (Flip trigger: any parse-verified-but-semantically-wrong write incident in dogfooding. Org policy can already force it per §7.2.)
2. Multi-root workspaces (monorepo siblings) — post-v1 scope; the jail and config layers already permit multiple roots.
3. Does `run` need streaming output for long test suites, via a Tasks-extension task? (Likely yes; deferred to first real need.)
4. Which capability grants third-party plugins request most in practice — the data that decides whether the default-deny grant UX needs a standing-consent tier.

---

## Appendix A — Decision record

**Brainstorm session, 2026-08-20.** 28 candidates across seven lenses; 17 killed (61%); 11 commitments embedded above: agent-native output contract (§4.2), workspace jail (§7.1), no-embeddings refusal (§1.1), MRTR confirm (§9.4), journal+undo (§7.6), SQLite-not-graph-DB (§10), zero-config cold start (§7.3), measurement from day one (§11.1), stateless transport core (§5), single Rust core crate (§3), QuickJS/Wasmtime sandbox (§9.7).

Notable kills, with reasons: *drop `script` from v1* (deferring it reinvites bash exactly where bash is worst); *drop LSP entirely* (survives as a resolver plugin, §8.3); *runtime WASM grammar packs* — **annotated by v1.1, not reversed**: the kill targeted WASM in the hot path; bundled grammars remain native, and WASM grammars serve only the unbundled long tail with a promotion path; *MCP Apps diff-review UI* (MRTR already carries the human-approvable diff); *context_packet tool* (`map`+`search` with the envelope *is* the context packet); *auto-generated companion skill* (the skill's value is judgment; a CI drift-check covers the real risk).

**Stakeholder architecture session, 2026-08-20 (v1.1).** D1 single-replica HTTP in v1, fleet designed-not-built (→ §5.2, Appendix B). D2 full public plugin SDK in v1, all five surfaces (→ §8). D3 WASM capability sandbox for plugins (→ §8.1). D4 every contract versioned from day 1 (→ law #5, §4.2, §10.2). D5 engine child process for parsing/indexing (→ §6). D6 doctor + opt-in crash reports (→ §11.2). D7 signed registry + sideload, CLI-side (→ §8.4). D8 five-layer config with org policy (→ §7.2). Standing directive: no time estimates anywhere in planning (→ §13).

## Appendix B — v2 fleet profile (designed, not built)

The v2 multi-replica HTTP profile, recorded so no v1 decision forecloses it. Replicas never share a live SQLite file: each replica holds a local index hydrated from content-addressed snapshots published by a single indexing owner (or a dedicated index service — chosen by measurement, not preference). The workspace volume carries source only; `.workbench/` state is per-replica or service-owned. Confirm-token HMAC keys move from `.workbench/keys/` to a key service; the journal gains a replica-id dimension and a merge rule; `ENGINE_BUSY`/backpressure semantics extend to cross-replica admission control. Preconditions established in v1: zero in-memory session state (§5.1), identity-on-the-request (§7.6), cache/durable data classes (§10.2), and the port layer (§6.4) — the fleet adapter set implements the same ports.
