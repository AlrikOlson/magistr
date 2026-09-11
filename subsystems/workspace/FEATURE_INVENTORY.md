# Workspace source-to-native inventory

Baseline: Workbench `2a2430bbcd5dcbed00920f92c6f3e6928f387d95`.
[`UPSTREAM_MANIFEST.json`](UPSTREAM_MANIFEST.json) accounts for all 525 Git files:
428 reference inputs retained and 97 exclusions with reasons. File bytes and
modes are checked against that pinned tree; [`LOCAL_CHANGES.json`](LOCAL_CHANGES.json)
names reviewed reference repairs. [`NATIVE_IMPORTS.json`](NATIVE_IMPORTS.json)
maps native files back to their source. Reference source is a parity oracle,
not a second production product or evidence of native service completion.

`arch-14c-workbench-source-fork` adopts the engine into the root workspace as
[`magistr-workspace`](../../crates/magistr-workspace/src/lib.rs), an ordinary Rust
library with no NAPI dependencies, addon loader, `build.rs`, or `no-bindings`
test escape hatch. Parsing, lexical/structural search, rewriting, extraction,
language registration, and the Wasmtime host are native source now. Configuration,
path/command admission, keys and response/byte contracts are native through c1.
The c2 worker supervisor now isolates parser, dynamic grammar and sandbox/LSP
execution in Magistr's private executable role, with bounded frames, admission,
deadlines, cancellation, backoff, quarantine and owned process cleanup.
The c3 index now persists native file/symbol/import/test-region and FTS records,
refreshes through bounded native/poll watchers, and revalidates live candidate
bytes, modes and ignore selection. The c4 typed query service now composes
admitted read/search/map operations, deterministic ranking and budget envelopes.
The c5 journal now preserves retained v1/v2 SQLite history and exact-byte blobs,
with bounded native reads, session/operation selection and pure undo plans.
The c6 pipeline now applies confirmed, verified byte transactions, and the c7
commands resolve edit/files verbs and applied undo through it. The c8
extensions layer now validates manifests, records and grants, breakers plugins
on disk, runs the conformance kit, hosts the QuickJS script tool and coordinates
the lang-pack, ranking, resolver and runner lanes through the worker. The c9
execution layer now runs declared commands through an owned-process executor
and parses runner and compiler output behind open registries, and the c9
diagnostics answer status, journal rows, redacted traces and doctor
reports/bundles read-only.
The c10 `WorkspaceService` now composes every capability behind one
admitted owner per process and the rmcp adapter `magistr workspace serve`
serves the complete retained tool surface, compared with the reference
server in twin disposable roots. The operational CLI verbs, the Bun-free
release smoke, the supported matrix and the measured budgets remain
`arch-14c10b-workspace-port-acceptance`; production owner routing and
launcher cutover remain `arch-14d-workspace-owner`.
Listed tests are retained evidence targets; listing a test does
not claim that a particular platform or execution lane passed.

## Public tools

There are eight schema-defined tools plus schema-less `workbench_status`.
Names, inputs, and the status registration come from
[`schemas/tools`](reference/schemas/tools) and
[`src/protocol/server.ts`](reference/src/protocol/server.ts). The native
[`WorkspaceService`](../../crates/magistr-workspace/src/service/workspace_service.rs)
composes every capability and the rmcp adapter in
[`crates/magistr-cli/src/workspace_mcp`](../../crates/magistr-cli/src/workspace_mcp/mod.rs)
registers the complete surface under `magistr workspace serve`; production
consumers still reach the workspace through the legacy server until owner
cutover.

| Tool | Contract and behavior to preserve | Reference implementation and tests | Native completion boundary |
| --- | --- | --- | --- |
| `search` | Auto/lexical, structural, references, files; regex and glob validation; test/vendor ranking, kind/context filters, grouped deduped budgeted results and truncation manifest; reference confidence provenance; semantic is reserved and rejected | `src/tools/search.ts`, `ranking.ts`; `test/search-tool.test.ts`, `ranking.test.ts`, `ranking-shadow.test.ts`, `rust-structural.test.ts`, `freshness-fuzz.test.ts` | Typed c4 native queries, live admission/freshness, native engine matching and retained ranking goldens delivered; c8 extension enrichment; served by c10 with the request's cancellation token reaching the worker and the declaration compared with the reference server |
| `map` | Pruned tree, skeleton/outline/full, language census, index health and generation TTL, trusted/sideloaded plugin inventory, import graph, bounded Cargo workspace/crate/entry-point rows | `src/tools/map.ts`, `map-cargo.ts`; `test/map.test.ts`, `support/golden/map/cargo-workspace.expected.json` | Typed c4 tree, outlines, native index/import and bounded Cargo summaries delivered; c10 serves the installed-plugin inventory rows with the retained fail-closed trust labels and the untrusted note (compared with the reference server) beside the index row |
| `read` | Whole file, inclusive 1-based line ranges, symbol and enclosing/import surrounds; fresh hashes, exact symbol extents, segmented budgets/cursors; ambiguous/missing symbol and binary handling | `src/tools/read.ts`, `src/envelope/file-content.ts`; `test/read.test.ts`, `binary-files.test.ts`, `freshness-fuzz.test.ts` | Typed c4 admitted reads, exact native symbol/surrounds, byte hashes and segmented envelopes delivered |
| `edit` | Symbol body/insert/rename, structural rewrite, anchored replace-text; stale expectedHash rejection; aggregate dry-run, confirmation, parsing, formatting, journal and atomic application | `src/tools/edit.ts`, `write-engine.ts`; `test/edit-verbs.test.ts`, `write-engine.test.ts`, `mrtr-confirm.test.ts`, `rust-structural.test.ts` | Native c6 confirmed verified transactions and c7 `WorkspaceCommands::edit` (symbol verbs over the validated outline, references-search rename, worker rewrite, anchored replace-text, token gating) delivered; served by c10 with the retained admission messages, MRTR elicitation and the token fallback |
| `files` | Create/mkdir/copy/move/delete; binary preservation; import-aware JS/TS/Python/Java/C/C++ moves and Rust module/Cargo target moves; protected paths; recoverable trash; no accidental overwrite | `src/tools/files.ts`, `rust-move.ts`; `test/files-tool.test.ts`, `rust-move.test.ts`, `binary-files.test.ts`, `binary-mcp.test.ts` | Native c7 `WorkspaceCommands::files` delivered over the c6 pipeline: import-aware moves for the four families, Rust module/Cargo-path moves, trash-backed deletion, parent creation, touch/mkdir/copy semantics; served by c10 with the retained admission, MRTR elicitation and the token fallback |
| `run` | One declared argv command, guarded extra args, no shell or text/file munging; bounded time and output; runner failures versus compiler diagnostics; Cargo/Clippy/nextest and other bundled runners; plugin runner extensions | `src/tools/run.ts`, `run-parse.ts`, `run-cargo.ts`; `test/run-tool.test.ts`, `run-cargo.test.ts`, `runner-plugin.test.ts`, Cargo golden fixtures | Native c9 `WorkspaceRun` over the c1 `CommandAdmission` (never widened) and the owned-group `ProcessExecutor` (bounded head+tail capture, timeout/cancel/drain-deadline kills, concrete outcomes); seven bundled runner parsers and the cargo diagnostics extractor equal every retained golden; plugin runner lanes precede the bundled floor through open `RunnerParser`/`DiagnosticExtractor`/`PluginRunnerLane` registries; served by c10 with the request's cancellation token and installed `runner-` plugin lanes |
| `script` | QuickJS guest with live jailed search/read/glob; queued edits and file ops only; dry-run default; deterministic re-execution/confirmation; fuel, memory and host-call caps; value plus applied/dry-run diffs | `src/tools/script.ts`, `src/ports/sandbox.ts`; `test/script-tool.test.ts`, `sandbox-host.test.ts`, `sandbox-escape.test.ts` | Native c8 `WorkspaceCommands::script` over the worker WASI lane: retained shim/sentinel protocol, deterministic re-entry over typed c4 search/read/glob, 32-call ceiling, typed sandbox kills, queued edit/create/mkdir/delete resolved through the c7 gate (always gated, write jail decides first); served by c10 over the embedded interpreter with MRTR elicitation and the token fallback |
| `undo` | Read-only recent journal pagination and named operation selection; rewind whole batches by session by default; explicit workspace scope; surface/keepCurrent/force conflicts; eviction notes and binary restoration | `src/tools/undo.ts`, `journal-view.ts`; `test/undo.test.ts`, `journal-db.test.ts`, `binary-files.test.ts`, `journal-cli.test.ts` | Native c5 durable queries and pure plans, c7 applied session/operation-scoped undo with conflict/kept/evicted rows, force/keep-current, bookkeeping consumption and exact binary restoration delivered; c10 serves `action: list` with the retained probe, op-boundary budget cut and `beforeSeq` cursor (compared with the reference server); owner identity fencing later in arch-14d |
| `workbench_status` | Schema-less service/index/engine health and degradation; useful during index warmup or child restart, without inventing readiness | `src/protocol/server.ts`, `workspace.ts`; `test/workspace-runtime.test.ts`, `engine-supervisor.test.ts`, `mcp-conformance.test.ts` | Native worker/index health c2/c3; c10 serves the retained deterministic row (root, journal mode, SQLite's applied mode verbatim, network detection, envelope versions) with degradations in notes, byte-identical across fresh owners |

Paths in the tables are relative to [`reference/`](reference).

## Contracts, authority, persistence, and composition

| Capability | Source and executable evidence | What is native now; remaining boundary |
| --- | --- | --- |
| Envelope version negotiation, deterministic row ordering, token accounting and truncation, additive compatibility, content hashes, diff and secret redaction | `src/envelope/*`, `schemas/envelope.schema.json`, `schemas/baselines/envelope.v1.schema.json`, `src/generated/schemas.ts`; `test/envelope-*.test.ts`, `diff.test.ts`, `redaction.test.ts`, `generated-schemas.test.ts`, `statelessness.test.ts` | Native c1 response adapter plus c4 query rows/budgets; shared responses/content fixtures and real independent-root query envelope comparison; c10 serves the envelope as every tool's outputSchema with structuredContent and text carrying the same sealed JSON, the schema byte-identical to the reference (`schema_parity`) |
| Error code → recovery and required payload; eight tool input schemas and schema generation | `schemas/error-taxonomy.json`, `schemas/tools/*`, `src/envelope/errors.ts`, `scripts/generate-schemas.ts`; `test/error-taxonomy.test.ts`, `generated-schemas.test.ts`, `skill-drift.test.ts` | Native 24-code typed taxonomy and required payloads in c1; c10 serves the eight input schemas byte-identical to the reference tree and the retained InvalidParams admission messages |
| Five-layer config merge, key provenance, command allowlist, policy ceilings and detection | `src/config/*`, `workbench.toml`; `test/redteam-policy.test.ts`, `redteam-served.test.ts`, `doctor.test.ts` | Native five-layer config, provenance, command admission and suggestion-only detection in c1; composed policy protects all loaded layers |
| Canonical jail, nonexistent in-root path handling, symlink/TOCTOU checks, protected configuration and metadata, secret-key creation | `src/jail/*`, `src/protocol/write-access.ts`; `test/redteam-jail.test.ts`, `jail-keys.test.ts`, `write-engine.test.ts` | Native c1 admission/private key lifecycle with malicious-path regressions; c6 adds application-time revalidation and key-bound confirmation grants |
| Versioned port interfaces: engine 2, index 9, journal 4, watcher 1, sandbox 1, formatter 1, resolver 2 | `src/ports/*`, especially `versions.ts`; `test/import-graph.test.ts`, `inprocess-adapter.test.ts`, `wit-contracts.test.ts` | Rust primitives adopted; application-facing contracts and effect ownership c1–c10 must preserve these behaviors without internal JSON/tool dispatch |
| Tree-sitter parse/verify, grep regex search, ast-grep structural match/rewrite, ignored/vendor classification, bundled language registry | `crates/workbench-core/src/engine.rs`, `langs.rs`; inline source tests, `test/rust-structural.test.ts`, `inprocess-adapter.test.ts` | Adopted in `crates/magistr-workspace/src/{engine,langs}.rs`; service worker boundary c2 |
| Symbols, imports, references, Rust trait/impl ambiguity, docs, test ranges and confidence provenance | `crates/workbench-core/src/indexer.rs`; inline tests, `test/indexer.test.ts`, `resolver-plugin.test.ts` | Extraction adopted in native `indexer.rs`; query/storage/freshness c3/c4 |
| Engine process IPC frames, self-test admission, bounded queue, deadlines, restart/backoff, repeated-input quarantine, crash isolation | `src/adapters/engine-ipc/*`, `src/protocol/tasks.ts`; `test/ipc-frames.test.ts`, `engine-child.test.ts`, `engine-supervisor.test.ts`, `tasks.test.ts` | Native `worker` module and private Magistr executable role delivered in c2; real process acceptance covers framing, readiness, crash/restart, quarantine, cancellation, saturation and reaping; c10 serves the status row and the `index/build` task |
| Index database, symbol/import/reference tables and FTS deletion, build progress, freshness from mtime/size then hash, external edits, optional fswatch and rebuild | `src/adapters/engine-ipc/indexer.ts`, `ipc-index.ts`, `src/adapters/fswatch/*`; `test/indexer.test.ts`, `freshness-fuzz.test.ts`, `watcher-wiring.test.ts`, `fswatch.test.ts` | c3; index remains rebuildable derived data, never a source revision |
| Durable/cache SQLite lifecycle, schema versions, forward migration, newer-format refusal, corruption quarantine and sidecars | `src/data/versioned-db.ts`, `src/protocol/workspace.ts`; `test/migrations.test.ts`, `workspace-runtime.test.ts` | Native c3 cache rebuild and c5 durable journal migration/quarantine are separate; journal history and sidecars are preserved, unknown versions refused |
| Journal batch metadata (tool, args hash, session, undoOf), per-file CAS before/after blobs, zstd storage, 256 MiB eviction with notes, lossless binary snapshots | `src/adapters/journal-db.ts`, `src/ports/journal.ts`, `src/envelope/file-content.ts`; `test/journal-db.test.ts`, `binary-files.test.ts`, `undo.test.ts` | Native c5 exact-byte storage, append transactions, read-only history and undo selection delivered. A journal sequence does not certify successful filesystem application |
| Write lock → diff → confirmation → parse → format → journal → atomic swap; rollback, stale anchors, aggregated HMAC-confirmation binding, binary and modes | `src/tools/write-engine.ts`, `edit.ts`, `src/protocol/write-access.ts`; `test/write-engine.test.ts`, `write-lock.test.ts`, `mrtr-confirm.test.ts`, `binary-mcp.test.ts` | Native c6 immutable byte previews, exact request/session-bound confirmations, supervised parse/format, per-file atomic swaps and observed compensation outcomes delivered. Native v3 journal intents defer undo consumption until applied; c7 verbs resolve intent into these transactions |
| Rustfmt/Biome and passthrough formatter adapters; formatting failure must not masquerade as verification | `src/adapters/rustfmt/*`, `biome/*`, `passthrough-formatter.ts`; `test/rustfmt-formatter.test.ts`, `biome-formatter.test.ts` | Native c6 bounded Rustfmt/Biome routing, toolchain/edition/config discovery, revalidation and explicit degradation delivered; c9 owns declared run-command execution |
| Wasmtime core/component sandbox, memory/fuel limits, capability-granted hostcalls, plugin subprocess pool and parent-death cleanup | `crates/workbench-core/src/sandbox.rs`, `wit/{tool,subprocess}.wit`; inline tests, `test/sandbox-host.test.ts`, `sandbox-escape.test.ts`, `server-pool.test.ts` | Native sandbox executes through c2 worker requests; bounded LSP reader/session cleanup and owned worker group shutdown are tested. Plugin policy and extension orchestration remain c8 |
| Plugin manifest validation, trust/capability policy, namespaced tools, circuit breaker and plugin TCK | `src/tools/plugins.ts`, `tck.ts`, `src/protocol/*-plugin.ts`, `wit/*`, `plugins/*/plugin.toml`; `test/plugin-manifests.test.ts`, `plugin-tck.test.ts`, `m5-gate.test.ts` | Native c8 `extensions`: manifest validation, per-call inventory with fail-closed tiers, declared ∩ granted with the blessed-only subprocess rule, disk breaker keyed by component hash, `PluginHost` list/run with derived wire names, four-chapter `run_plugin_tck`; every guest through the supervised worker |
| Dynamic WASM language packs, query validation, native Gleam promotion and indexed hot path | `crates/workbench-core/src/lang_registry.rs`, `wit/lang-pack.wit`, `plugins/lang-pack-gleam/*`; `crates/workbench-core/tests/lang_pack_hot_path.rs`, `test/lang-pack.test.ts`, `lang-pack-promotion.test.ts` | Registry adopted in native `lang_registry.rs`, native hot-path/WIT tests retained; c8 `LangPackBoot` discovers, breakers and registers packs through the worker with the tier selecting the lane (blessed → native); host-side extension selection for index/queries stays bundled (backlog) |
| Resolver confidence and LSP process isolation, ranking shadow failure isolation, runner-go failure extraction | `wit/{resolver,ranking,runner,subprocess}.wit`, `plugins/{lsp-resolver,ranking-visibility,runner-go}/*`; `test/resolver-plugin.test.ts`, `ranking-shadow.test.ts`, `runner-plugin.test.ts`, `server-pool.test.ts` | Native c8 `PluginResolver` (blessed + granted, warm-index gate, 60 s breach expiry, budget, jail-admitted edges merged into references search), `RankingSignals` (promotion store, shadow log, feature cache, promoted features through the first-party combiner off the response path) and `RunnerAdapter` over new worker ranking/runner requests; the run tool's fallback floor is c9; component guests, sources, locks and fixtures retained |
| Operational CLI setup/doctor/journal/TCK, telemetry ring buffer, bounded redacted diagnostic bundles, status and task polling | `src/main.ts`, `src/doctor/*`, `src/protocol/{tracing,tasks,serve,server}.ts`; `test/doctor.test.ts`, `doctor-bundle-gate.test.ts`, `tracing.test.ts`, `journal-cli.test.ts`, `tasks.test.ts`, `mcp-conformance.test.ts` | Native c9 `WorkspaceStatus`, `JournalInspection` (read-only reader, no lock, no rewrite), `TraceRecorder` (typed spans, derived recovery, credential-redacted JSONL, capped, fire-and-forget) and `DoctorRun`/`DoctorBundle` (six probes, read-only, ustar+gzip through one redaction choke point) delivered; c10 serves the `index/build` task over tasks/get and the retained tasks/list plus one trace span per call; CLI verbs and exit codes remain arch-14c10b |
| One owner, session admission, confirm/undo identity, reconnect, shutdown, stale-generation fencing | `src/adapters/workspace-lock.ts`, `specs/018-multi-client-daemon/*`; `test/multi-client.test.ts`, `write-lock.test.ts` | Deferred to `arch-14d-workspace-owner`; audited multi-client tests are opt-in red reproductions. Preserve this gap, replace age-based live-owner stealing, never kill a lock owner |
| Source identity/selection manifest, external writes, ignored/untracked/binary inputs, symlinks/modes, observation versus immutable snapshots | Existing freshness/write source above; accepted contract in `docs/architecture/owned-subsystems.md` | Deferred to `arch-14e-source-snapshots`; neither index generation nor journal sequence proves check input identity |

## Degradation and error paths

| Lane or failure | Required behavior and retained evidence | Native boundary |
| --- | --- | --- |
| Cold/disabled index | Tier-0 lexical/file/tree work continues; symbol/reference-only requests return `INDEX_UNAVAILABLE` with progress, not invented results. `test/search-tool.test.ts`, `map.test.ts`, `tasks.test.ts` | c3/c4 |
| Native addon unavailable in reference | Loader tries WASI fallback and self-tests parse/search/rewrite before readiness; fallback lacks native-only extension host. `src/adapters/core/loader.ts`, `test/core-loader.test.ts`, `engine-supervisor.test.ts` | Reference-only comparator; native product has no addon lane |
| Reference WASM build | `build:core:wasm` must use `--no-default-features`: Wasmtime cannot compile into this fallback target. Native feature defaults retain Wasmtime 36 for Tree-sitter and 48 for extensions | Source gate enforces matrix; cross-platform artifact execution remains separate evidence |
| Parser crash/overload | Retry/backoff, bounded queue, quarantine repeat crash input until changed, preserve surviving core capability. `test/engine-supervisor.test.ts`, `engine-child.test.ts`, `ipc-frames.test.ts` | c2 |
| Watcher absent or delayed | Query-time freshness still checks actual files; derived caches may be rebuilt. `test/freshness-fuzz.test.ts`, `watcher-wiring.test.ts` | c3 |
| Plugin trap/fuel exhaustion/capability violation | Limit guest resources, break the failing plugin, leave core tools usable; do not silently widen capabilities. `test/sandbox-escape.test.ts`, `plugin-tck.test.ts`, `ranking-shadow.test.ts` | Native c8: typed `PLUGIN_CAP_DENIED` without breach, `PLUGIN_CRASHED` with a durable breach until the bytes change, lane loss as `ENGINE_UNAVAILABLE`; core queries never consult plugin state; guests only in the c2 worker |
| Unknown/new/corrupt storage | Cache rebuild, durable forward-only migration or untouched refusal, corrupt durable quarantine with sidecars. `test/migrations.test.ts` | c1/c3/c5 |
| Write parse/format/staleness/confirmation failure | Whole-batch rejection and reviewable error payload, no partial success claim; re-derive against fresh content. `test/write-engine.test.ts`, `mrtr-confirm.test.ts` | c6/c7 |
| Journal eviction/conflicting external edits | Surface missing undo blobs and current/expected/restore conflicts; never imply overwritten or evicted content can be recovered. `test/journal-db.test.ts`, `undo.test.ts` | c5/c7 |

The complete retained taxonomy has 24 codes. Native parity must preserve each
code's recovery and payload rather than flatten failures into strings:

| Recovery | Codes and required extra payload |
| --- | --- |
| `abort` | `PATH_OUTSIDE_WORKSPACE`, `COMMAND_NOT_DECLARED`, `RUN_REDIRECTED`, `UNSUPPORTED_VERSION`, `PLUGIN_CRASHED` (`plugin`), `CONFIG_POLICY_DENIED` |
| `narrow` | `AMBIGUOUS_SYMBOL` (`candidates`), `SYMBOL_NOT_FOUND`, `FILE_NOT_FOUND`, `PARSE_REJECTED` (`parserMessage`, `rejectedDiff`), `PATTERN_INVALID`, `BUDGET_TOO_SMALL`, `SANDBOX_FUEL_EXCEEDED`, `SANDBOX_MEMORY_EXCEEDED` |
| `retry` | `STALE_ANCHOR` (`contentHash`), `INDEX_UNAVAILABLE`, `ENGINE_UNAVAILABLE`, `ENGINE_BUSY` (`backoffMs`) |
| `confirm` | `CONFIRM_REQUIRED` (`confirmToken`), `CONFIRM_MISMATCH` |
| `escalate` | `PROTECTED_PATH`, `SANDBOX_CAP_DENIED`, `INPUT_QUARANTINED`, `PLUGIN_CAP_DENIED` |

## Native delivery sequence and verification

| Native chunk | Completion responsibility |
| --- | --- |
| `arch-14c1-workspace-policy` | Native configuration, composed path/command admission, protected policy, key lifecycle, envelopes and typed error/byte contracts; shared fixture parity |
| `arch-14c2-workspace-workers` | Parser/extension process supervision, bounded IPC, crash and quarantine behavior |
| `arch-14c3-workspace-index` | Native persistent derived index, bounded refresh/watchers, candidate queries and build health; public navigation semantics remain c4 |
| `arch-14c4-workspace-queries` | Delivered typed search/map/read over admitted live source; documented native limits and independent-root reference fixtures |
| `arch-14c5-workspace-journal` | Delivered: binary durable journal, schema recovery, retention, read-only history and undo restoration plans |
| `arch-14c6-workspace-transactions` | Verified atomic writes, formatter integration and confirmation |
| `arch-14c7-workspace-editing` | Delivered: edit/files verbs and applied undo through the shared transaction pipeline; served by c10, owner fencing remains arch-14d |
| `arch-14c8-workspace-extensions` | Delivered: plugin manifests/records/grants/breaker, plugin host, TCK, script verb and the lang-pack/ranking/resolver/runner lanes over the worker; served rows, map inventory and composition delivered by c10 |
| `arch-14c9-workspace-execution` | Delivered: owned-group declared-command execution with the retained parsers and goldens behind open registries, plus status/journal/trace/doctor diagnostics; served rows and composition delivered by c10, CLI verbs remain arch-14c10b |
| `arch-14c10-workspace-port-parity` | Delivered: the typed `WorkspaceService` composition root with OS-held single-owner admission and the thin rmcp adapter `magistr workspace serve`; served conformance in disposable workspaces and against the reference server. Production owner routing remains arch-14d |
| `arch-14c10b-workspace-port-acceptance` | Pending: the operational CLI verbs with the retained exit codes, the Bun-free release-artifact smoke, the supported Rust/platform/feature/dependency matrix and the measured budgets; closes this inventory |

`arch-14d-workspace-owner` and `arch-14e-source-snapshots` follow their native
prerequisites. This table maps implementation scope; native roadmap state owns
readiness and completion. There is no implicit CLI/MCP runtime cutover in c.

Root formatting, check, Clippy and nextest include `magistr-workspace`.
[`workspace-source-gate.py`](../../scripts/workspace-source-gate.py) verifies
the original tree, retained hashes/modes, explicit local deltas, ordinary local
Rust library, both Wasmtime majors, and absence of NAPI in its production graph.
[`workspace-structure-gate.py`](../../scripts/workspace-structure-gate.py) measures
every native and retained reference Rust file using the shared lexical measure:
public items, inline test modules, long-function count/excess and longest
function. Each file is bounded by the reviewed numeric baseline; new files
receive normal limits. Compiler output is excluded, source is not blanket
allowlisted. Export denial remains covered by the workflow source gate.

Reference Bun contracts, schema generation/type checks, native default-feature
Rust tests and the reference `workbench-core/no-bindings` test lane are separate
checks. A green no-bindings test run is not proof of a shipping native service.
Bundled guest/WIT/grammar inputs, reference test fixtures, lockfiles and build
scripts remain pinned. Full verification outcomes belong to recorded ship
checks and the iteration receipt; this inventory does not claim unrun lanes.
