# Workbench source fork

This directory preserves the Workbench implementation used to establish
Magistr's native workspace subsystem. It is an internal source fork, not a new
deployment of the standalone Workbench server.

## Source identity and import boundary

- Source checkout: `/Users/alrik/Serious_Code/workbench-mcp`.
- Revision: `2a2430bbcd5dcbed00920f92c6f3e6928f387d95`.
- Git tree: `c264c99904d406721c2af2760cac2ceaec0aa4f9`.
- Import date: 2026-09-10.
- The source checkout was clean at inspection and was not modified.

[UPSTREAM_MANIFEST.json](UPSTREAM_MANIFEST.json) inventories all 525 tracked
upstream files, including each path, Git mode, Git blob, SHA-256, and byte count.
It includes 428 files under [reference/](reference/) and records the exclusion
reason for the remaining 97. This is an inventory of the pinned tree; added or
changed local files must be recorded separately as local deltas rather than
rewriting upstream hashes.

The reference includes the TypeScript implementation, native Rust source and
tests, protocol schemas, WIT packages and compatibility baseline, plugin
manifests, guest source and lockfiles, committed guest binaries, fixtures,
specifications, and canonical and Claude routing skills. Unrelated host and
workflow configuration and exploratory spikes are excluded. The manifest is
authoritative for individual paths. Historical source paths, instructions, and
commands inside the reference describe the standalone upstream application;
they do not configure Magistr or establish an additional workflow authority.

The external checkout is historical provenance only. Builds and tests of the
fork must use files in this repository and must not read it at runtime.

## Native boundary

[magistr-workspace](../../crates/magistr-workspace/) is the native Rust library.
[NATIVE_IMPORTS.json](NATIVE_IMPORTS.json) maps its imported source back to the
pinned reference. The native crate exposes the engine, indexer, bundled
languages, dynamic language registry, and sandbox host through Rust APIs.

The upstream `bindings.rs`, NAPI build script and dependencies, `cdylib` output,
and `no-bindings` test workaround are outside this native library. Their
original source remains in the standalone reference. This distinction does
not assert that Magistr's protocol handlers already use the native library;
handler integration and behavior migration are separate work.

The `sandbox` feature retains the Wasmtime 48 host. The `lang-pack` feature
retains tree-sitter's WASM grammar support and its separate Wasmtime 36
dependency. Both upstream default features are retained; eliminating either
runtime or changing language behavior is outside this import. Guest crates
remain standalone WASM packages with their own lockfiles and are not native
Magistr workspace members.

The native library resolves against Magistr's root `Cargo.lock`; the
standalone reference retains its original, separate lockfile. The import
does not promise byte-identical dependency graphs. The current resolutions
include these patch-version differences:

| Dependency | Preserved reference lock | Magistr native lock |
|---|---|---|
| tree-sitter | 0.26.12 | 0.26.13 |
| Wasmtime sandbox host | 48.0.0 | 48.0.2 |
| Wasmtime grammar backend | 36.0.14 | 36.0.15 |
| WAT parser | 1.257.1 | 1.257.1 |

The native lock deliberately retains the reference WAT parser version.
An initial resolution to 1.259.0 rejected five inherited component fixtures;
pinning 1.257.1 preserves those fixtures without changing the source behavior.

Native tests must verify the integrated graph; a passing reference build
alone does not verify these native resolutions.

## Committed binary artifacts

The following SHA-256 values identify the bytes copied from the pinned tree.
They establish import integrity, not independent reproduction of the original
build environment.

| Artifact under `reference/` | Bytes | SHA-256 |
|---|---:|---|
| `plugins/runner-go/guest.wasm` | 47,728 | `195a10acbbd606ceabfbd5e2ccf817fdb60807dd949e4d13fb302567cf10d1f2` |
| `plugins/lsp-resolver/guest.wasm` | 80,895 | `ce29999850545d058b8bcf2498881fed01ff65abe1dbf6d30e080c7f92b63298` |
| `plugins/ranking-visibility/guest.wasm` | 15,600 | `2920381936628dcc9c11dcab4b9a79ddd5a27719f84c9e62b3b909cff380af4a` |
| `plugins/lang-pack-gleam/guest.wasm` | 307,774 | `13174d3606225a4851ae8664666133cd26d69eb2ce0ddafc807c8f013ea514e2` |
| `plugins/lang-pack-gleam/grammar/gleam.wasm` | 277,693 | `9f8d580d19c415f80b391b8d9ff02621da46f7c24a583b90830052a630cd7f4f` |
| `vendor/qjs/qjs-wasi.wasm` | 1,566,956 | `d2939e98c808e8b9f4164cd0d7b0398cbc0121ddf52862bcd92157d923e461cc` |

The four first-party guest artifacts have adjacent Rust source, WIT inputs,
and Cargo lockfiles. The Gleam guest additionally embeds the grammar binary
and tag query under `plugins/lang-pack-gleam/`. Upstream does not supply a
build attestation or a complete toolchain-input inventory for these binaries.

The [QuickJS artifact README](reference/vendor/qjs/README.md) identifies the
official QuickJS-ng v0.16.2 `qjs-wasi.wasm` release asset and gives the same
SHA-256 measured above. Its upstream release is
[quickjs-ng/quickjs v0.16.2](https://github.com/quickjs-ng/quickjs/releases/tag/v0.16.2).
The QuickJS source and WASI build toolchain are not included in this import.

The [Gleam grammar README](reference/plugins/lang-pack-gleam/grammar/README.md)
identifies the `tree-sitter-gleam` 1.0.0 generation and tree-sitter-cli 0.26.12,
but gives no exact upstream source commit or WASI SDK version. Its original
recipe clones an unpinned default branch. A future reconstruction should pin
the [v1.0.0 source](https://github.com/gleam-lang/tree-sitter-gleam/tree/v1.0.0),
tree-sitter-cli 0.26.12, and the downloaded SDK, then record the resulting
hash. That recipe has not been executed here and is not claimed to reproduce
the committed bytes.

## Runner build-script repair

Upstream `plugins/runner-go/guest/Cargo.toml` refers to `build.sh` and
`build.ps1`, but neither script exists in the pinned tree. The local fork adds
both under `reference/plugins/runner-go/`. They build the preserved guest with
`--locked --release --target wasm32-wasip1` and run the preserved
`componentize` crate with `--locked`. An explicit guest target directory keeps
the input path correct when the caller sets `CARGO_TARGET_DIR`. Both scripts
run from the reference root so its Rust toolchain selection applies;
PowerShell checks native process exit codes explicitly.

The required Rust target is `wasm32-wasip1`. From the repository root:

```sh
bash subsystems/workspace/reference/plugins/runner-go/build.sh
```

On PowerShell:

```powershell
& ./subsystems/workspace/reference/plugins/runner-go/build.ps1
```

These commands replace the local runner `guest.wasm`; review its new hash
before committing a rebuild. The componentizer supplies no WASI adapter, as
the runner guest is intended to have zero WASI imports. This repair adds
missing build entry points and preserves the committed binary. The local
source deltas in [LOCAL_CHANGES.json](LOCAL_CHANGES.json) also record Rust 1.95
formatting and three equivalent Clippy cleanups in the guest sources.

## License and verification limits

[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) records the exact available
license evidence and links complete component license texts. Upstream's root
Cargo metadata declares Apache-2.0, but the pinned tree has no top-level
LICENSE or NOTICE, and the standalone guest manifests omit license metadata.
This fork records those omissions without inventing authorship, an upstream
notice, or a guest license declaration.

The coordinating iteration has recorded the following reference results on
the current macOS host:

| Check | Observed result |
|---|---|
| Locked reference NAPI addon build | Passed; the NAPI CLI forwards `--locked` to Cargo after `--` |
| Reference native addon smoke | Passed |
| Full reference Bun suite | 1,069 passed, 10 skipped, 0 failed; 66 files, final gate 39.95 seconds |
| Reference Rust and guest checks | Workspace format, Clippy and tests passed; all four WASI guests passed format and Clippy |
| Shared conformance | Real reference addon and ordinary native Rust consumers passed the shared engine and sandbox fixtures |
| Opt-in ADR018 multi-client diagnostic | 1 failed, 1 skipped; a second client on a disposable `/tmp` root exited 75; recorded as a nonrequired diagnostic |

The optional multi-client diagnostic is an unresolved reference limitation,
not evidence of successful multi-client migration. No production workspace
lock owner was killed and no competing production workspace server was
started for this diagnostic.

The reference WASM NAPI addon was not rebuilt. `rust-analyzer` was unavailable,
and non-host operating-system process-death cases were skipped. Some entries
in the ten-skip count are inverse skip-sentinel tests; the count must not be
interpreted as ten missing capabilities or as a skipped real rustfmt check.

The added runner shell script passed `bash -n` and a temporary fake-Cargo
smoke covering invocation from another directory, an ambient
`CARGO_TARGET_DIR`, locked build arguments, and stopping after guest build
failure. The reusable
[disposable build check](../../scripts/workspace-runner-build-check.py) then
compiled the real runner with Rust 1.95.0 and its `wasm32-wasip1` target and
validated the resulting component through the preserved componentizer. It
produced a 47,766-byte component with SHA-256
`0063de938f30996c78df5b1f1289f7a11a7af8e03f175be97c927611c032cc73`.
Its size and hash differ from the pinned guest; the build is functional,
but byte-for-byte reproduction is not established. The check
confirmed the checked-in guest was unchanged and deleted the temporary
outputs. PowerShell was unavailable, so that script has not been executed.

The native Rust 1.95.0 matrix on macOS passed a locked release build with
both upstream default features and tests with no default features, sandbox
only, and lang-pack only. The default-feature native suite passed all 87
tests. All 15 required Magistr gates passed and were recorded, including
source integrity, structure, screenshots and release checks. The final full
workspace run passed all 9,670 tests; 20 additional tests were skipped by
configuration. The reference and native checks establish the source-adoption
boundary described here; they do not establish the deferred workspace service.
No Windows or Linux execution, complete TypeScript-to-Rust behavior parity,
or production protocol integration is claimed by the source import alone.

## Native policy port (arch-14c1)

The native `config`, `jail`, `contracts`, `response`, `file_content`, and
`workspace_policy` modules port the pinned TypeScript configuration, path/key,
envelope and byte contracts. The reference files and their import hashes remain
unchanged. These modules use the root lockfile; the added direct dependencies
were already locked (Serde/JSON, TOML, regex, SHA-256, getrandom and tempfile).
No new package or version is introduced by this port.

`WorkspacePolicy` composes read-only configuration resolution with path and
command admission. Existing Magistr project/user settings and
`PermissionPolicyService` retain their separate authority. Configuration stays
at `workbench.toml`, the existing user and organization paths, and explicit
environment overrides. Keys remain at `.workbench/keys/workspace.key`.
Command detection only returns suggestions; it cannot update the human-owned
allowlist. Resolved configuration is a snapshot, not a live reload service.

The port deliberately corrects these reference gaps:

- Path traversal follows each symlink before processing a following `..`,
  adapting Magistr's existing ACP target resolver. Dangling symlinks whose
  destination escapes the root are refused even when that destination is absent.
- All loaded configuration paths and additive protected subtrees are registered
  by the composed policy. Literal paths and the reference examples' trailing
  `/**` subtree notation are supported. Other glob forms produce an explicit
  `CONFIG_POLICY_DENIED` instead of silently protecting nothing. Built-in
  metadata/configuration protection cannot be disabled by approval.
- Key installation uses collision-safe private temporary files and atomic
  no-clobber installation; rotation atomically replaces the key. Invalid
  existing keys are errors and are never silently regenerated. Reads are bounded
  to key size; creation uses POSIX 0700 directories and 0600 files. Existing
  directory/key modes are preserved; Windows relies on directory ACLs.
- Empty declared commands are refused rather than producing an empty argv.
  `env` split-string options are also refused because they can reinterpret one
  allowed argument as a shell command. The retained command guard is not a full
  wrapper CLI parser: option operands such as `env -u NAME` and `nice -n 5`
  require further admission hardening before the native execution port. No
  command is executed by this policy unit.

The error taxonomy fixes all 24 code/recovery pairs and carries their required
payloads as typed variants. Native response outcomes contain no transport JSON;
`response` alone serializes and redacts outgoing envelopes. Truncation requires
a manifest whenever rows are cut. Token accounting is metadata at this boundary;
query-specific selection and byte/token ceilings remain part of the later query
and execution ports. Pinned redaction preserves truncation hints/cursors, while
redacting result values, notes and error payloads.

Shared [policy fixtures](conformance/README.md) exercise both the unchanged
reference and ordinary Rust consumers. Companion tests also cover deliberate
native corrections. Current API research used the Rust
[path semantics](https://doc.rust-lang.org/std/path/),
[atomic file creation](https://doc.rust-lang.org/std/fs/struct.OpenOptions.html),
[tempfile persistence](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist),
[regex Unicode semantics](https://docs.rs/regex/1.12.3/regex/#unicode), and
[Serde custom serialization](https://serde.rs/impl-serialize.html) documentation.

Path admission does not provide atomic directory-handle confinement; transactions
must revalidate immediately before effects. This unit does not supply key-bound
confirmation grants, database migrations, parser-worker supervision, a workspace
owner, production tool dispatch or launcher cutover. Those remain separate
native roadmap chunks. The earlier source-adoption result counts above describe
that earlier iteration, not the policy port.

## Native worker port (arch-14c2)

The `worker` module ports reference `adapters/engine-ipc/{frames,supervisor,
engine-child}.ts` into typed Rust requests and private process framing.
Magistr's executable hosts the worker role before normal application startup.
Existing workspace consumers and the Workbench connection have not switched
owners. This role never opens a workspace lock or native workflow store.

The native boundary keeps a 32 MiB per-frame ceiling, explicit version and
request identity, startup self-test, 128-call default admission ceiling, absolute
call deadlines, readiness timeout and restart backoff. A cancelled or dropped
active request recycles its owned synchronous worker. Quarantine fingerprints
include source and options, correcting the reference's path-only strikes and
its attribution of a crash to every queued request. The bounded ledger is
in-memory; durable diagnostics belong to later workspace status integration.

Tree-sitter, dynamic grammars and Wasmtime execution stay in the worker.
Sandbox limits may only tighten the existing one-billion-fuel/256 MiB ceilings.
Curated LSP specifications and capability admission remain the existing host
policy. LSP reader queues now retain at most 16 frames of at most 4 MiB each,
headers stop at 8 KiB, and each shared pool allows at most 16 live sessions,
including private invocations. Teardown kills/waits the owned LSP process and
joins its reader. Unix worker groups terminate descendants on hard cancellation;
graceful shutdown closes the sandbox pool before the worker is reaped. These
operations never inspect or kill a workspace lock owner. Callers must await
`shutdown()` before stopping the async runtime; Drop is a best-effort guard,
not a substitute for awaited cleanup.

Tokio and tokio-util were already in the root lockfile; this port adds direct
edges and no package/version update. The Unix libc edge now includes macOS for
dedicated process-group cleanup. Existing Linux parent-death and Windows
job-object LSP protections remain. Current primary API research checked
[Tokio child cleanup](https://docs.rs/tokio/latest/tokio/process/struct.Child.html),
[process groups](https://docs.rs/tokio/latest/tokio/process/struct.Command.html#method.process_group),
[Tree-sitter parse cancellation](https://docs.rs/tree-sitter/latest/tree_sitter/struct.ParseOptions.html),
and [Wasmtime resource limits](https://docs.wasmtime.dev/api/wasmtime/struct.Store.html).
The installed versions remain those recorded above.

The native process tests use the actual Magistr executable for parser,
dynamic grammar and sandbox behavior, plus a compiled Rust fixture for
deterministic crashes, stalls and corrupt streams. They also check queued and
active cancellation, dropped callers, deadline/backoff behavior, quarantine,
shutdown and owned-child cleanup. The fixture is test-only and adds no
production fault-injection mode. Dynamic language registrations and LSP warm
state are intentionally lost on worker restart; index/extension orchestration
must restore admitted state in later chunks. This iteration's gate results are
recorded in native ship state; it does not claim Windows/Linux execution or
production workspace port parity.


## Native index port (arch-14c3)

`WorkspaceIndex` starts cache opening and scanning in the background and sends
bounded extraction inputs through the existing native worker. Source content is
limited to 1 MiB per file; binary, invalid UTF-8 and oversized content receive
metadata records without parser/FTS content. A scan admits at most 100,000 files
by default, with a configurable ceiling of one million, and reports incomplete
walks. Each file replacement atomically updates file metadata, symbols (including
detail/doc), imports, test regions and aligned FTS5 rowids. Cache queries are
limited to 1,000 rows and live validation to 8 MiB of file content per call;
name-prefix and literal-phrase lookup are candidate operations for the later
navigation port, not complete public tool parity.

The fixed `native-index.db` and its SQLite sidecars are separate from retained
Workbench `index.db`, durable journals and workflow state. An OS-held
`native-index.lock` admits one native cache opener and is released after SQLite
closes; it neither reads nor replaces the workspace owner's lock. Version,
schema and integrity mismatches rebuild only the derived cache. Operational
busy/permission errors preserve its bytes. Database, lock and sidecar links
cannot alias durable files. The composing application supplies an admitted
state directory outside the source root or below its excluded `.workbench`;
in-workspace alternatives are refused to prevent cache writes from triggering
endless refreshes.

Network detection retains the explicit override, UNC and Linux longest-mount
selection, with a named local-default limitation elsewhere. The same result
selects WAL versus TRUNCATE and native versus polling watchers. Native overflow
or failure requests a complete reconciliation. The joined polling worker hashes
content and modes, coalesces changes into one epoch, and reports its 100,000-entry
and 64 MiB scan ceilings. Status includes pending changes, lag and errors.
Queries never wait for a cache build or parser: a busy/opening cache returns
explicit cold/warming results. Candidate hashes, modes, metadata and inherited
ignore rules are checked live; stale or unvalidated anchors are omitted.
Watcher-off queries remain warming and can miss additions until refresh.
Native mode also reconciles periodically to recover missed events.

Freshness is optimistic observation. A file may change after validation; index
generation is cache progress, never a source revision or completion receipt.
Startup rechecks persisted entries, and worker failure preserves host/cold
query availability and retries extraction through supervision. This coordinator
uses bundled language selection; dynamic extension activation and replay policy
remain c8. Await index shutdown before worker shutdown/runtime exit to settle
scans and join the poller. No production CLI/MCP consumer is switched over here.

The native cache and real Magistr executable tests cover initial/incremental
indexing, FTS replacement/deletion, corruption/incompatible-cache recovery,
exclusive cache ownership, watcher rename/delete/mode changes, ignore changes,
unchanged size/timestamp edits, bounded queries and owned worker crash/restart.
Current primary references are [SQLite WAL](https://www.sqlite.org/wal.html),
[FTS5](https://www.sqlite.org/fts5.html), [notify](https://docs.rs/notify/8.2.0/notify/)
and [Rust file locking](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock).
Verification is on the macOS host; Linux/Windows execution and reference WASM
addon rebuilding are not claimed. Full required gate results belong to the
native ship checks and iteration receipt.

## Native query port (arch-14c4)

`query::WorkspaceQueries` exposes typed read/search/map requests and outcomes
over the c1 admission policy, c2 supervised engine and c3 derived index.
It does not open a workspace owner, register production tools or dispatch
internal JSON tool calls. The shared bundled-language lookup serves scanning
and live queries; the direct globset dependency was already in the lockfile.

Reads preserve original-byte SHA-256 hashes, inclusive line/symbol extents,
32-line segments, enclosing/import surroundings and explicit ambiguous/missing
symbol errors. The leading import scan retains its lexical meaning and
200-line bound. Cold whole/range reads require no parser; symbol reads require
a live-validated, complete cached outline. Binary/invalid UTF-8 input is
`PATTERN_INVALID`, and mismatched cached content is `INDEX_UNAVAILABLE`;
neither can become lossy text or an apparently current source anchor.

Search preserves the seven retained ranking goldens, stable tie order,
definition/test/vendor weights, kind filters, context and grouped results.
Lexical and structural matching execute in the actual native parser worker.
References combine validated index candidates with live name occurrences;
heuristic confidence and provenance remain explicit. Missing/stale index rows
are omitted and marked warming, not treated as proof that references are absent.
Semantic mode is explicitly unsupported. Rust macro matching sees source
syntax, not macro expansion; multi-node patterns are refused by the engine.

Maps combine an admitted live tree with validated outlines, import summaries
and bounded Cargo workspace/crate/entry-point rows. The retained Cargo golden
is unchanged. Plugin inventory is explicitly unavailable until c8 supplies its
adapter. Native index health notes propagate cache/journal status; independent
build timestamps are omitted and cache generation never certifies source state.

Work is bounded before formatting the envelope: 1 MiB per live file,
100,000 walk entries, 10,000 search files, 64 MiB search source, 20,000 total
hits and 200 reference hits per file. Query patterns are at most 8,192 bytes
and contain no NUL; context is at most 20 lines. Budgeted row cuts produce
truncation manifests; work ceilings and oversized source/patterns receive
explicit errors or per-file omission notes. Whole-row accounting uses serialized UTF-16
units divided by four, rounded up, matching the retained wire contract.
Search retains the reference's pre-context budget accounting; requested context
can enlarge the final payload within its separate line/file ceilings.
Read/map stop before a row that does not fit; zero useful rows are an explicit
budget error rather than a fabricated continuation. Import graph validation
requires a complete inventory of at most 512 files in one cache generation;
otherwise graph/count summaries are omitted with a note. Outline enrichment
is capped at 512 files; Cargo scans at 256 manifests with 24 output rows,
eight entry points and 16 features per crate.

Live bytes are optimistic observations with before/open/after metadata checks,
fresh hashes and one retry. Unix opens traverse directory descriptors with
`O_NOFOLLOW` on every component and nonblocking final-file admission, preventing
a raced ancestor symlink from redirecting reads or a FIFO from blocking them.
The same opener serves index observations so cached query enrichment and
parser admission cannot bypass this source boundary. Required matrix testing
also exposed close-only cache-lock retention by duplicated descriptors. A
private owner guard explicitly unlocks after SQLite drops, preserving the
live-owner refusal while allowing immediate reopen even if a duplicated
handle survives. A deterministic duplicate-handle regression covers this
lifetime independently of concurrent process-spawn timing, consistent with
[Rust lock lifetime](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)
and [Apple flock semantics](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/flock.2.html).
Non-Unix admission retains path revalidation; atomic handle confinement there
is not established. Neither lane claims immutable snapshots or check-input
certification; that remains arch-14e.

The [query fixtures](conformance/README.md) compare complete envelopes against
the pinned real addon and index in independent temporary roots. Exact additive
native health/plugin diagnostics are asserted separately; all other notes,
hashes, freshness, budgets, ordering and cursors are compared. Native unit and
real worker tests cover cold/warm behavior, stale timestamp-preserving edits,
Rust macros/generics/attributes, malformed source, bounds and recovery.
Current primary research checked [ast-grep pattern parsing](https://ast-grep.github.io/advanced/pattern-parse.html),
[pattern syntax](https://ast-grep.github.io/guide/pattern-syntax.html) and
[globset options](https://docs.rs/globset/latest/globset/struct.GlobBuilder.html).
Verification is on this macOS host; Linux/Windows execution, extension
composition and production protocol cutover are not claimed by this chunk.

## Native durable journal (arch-14c5)

The native `journal` module reads the retained v1/v2 `.workbench/journal.db`
format through ordinary Rust APIs. It preserves operation sequence and timestamp,
file ordinal, session identity, undo relationships, and SHA-256 addresses of
uncompressed bytes. Existing zstd blobs remain unchanged during reads, migration
and deduplication. The v1-to-v2 migration changes only `user_version`; binary
snapshots and text, including BOMs, use the same lossless `FileContent` type.

Append records, blobs, undo links and retention changes share one SQLite
transaction. Read-only readers share the same query implementation, accept v1/v2
without migration and expose ascending history, newest-first bounded history,
keyset bounds and eligible operations filtered by session or named sequence.
Metadata eligibility and byte availability are separate: evicted history remains
visible with unavailable content, never an invented empty string. The default
retention cap counts 256 MiB of compressed blobs, evicting oldest reference
sequences with hash tie-breaking and protecting the newly appended operation.
If its protected bytes alone exceed the cap, a separate note reports the
overage. Operation metadata is retained; this cap is not a total database-size
limit.

Native query hydration refuses more than 1,000 operations or file records,
1 MiB per text field, or 64 MiB of aggregate decoded content and metadata.
Compressed blobs and zstd decoder windows are also bounded to 64 MiB;
SQLite row allocation is capped at 65 MiB before Rust copies row values. The
raw ascending list refuses an oversized result; bounded recent queries provide
sequence pagination. Hash/codec/size corruption is an error rather than eviction.
These explicit bounds and strict schema-object validation are intentional
hardening of the reference's unbounded reads, not silent truncation.

The separate `undo` module produces pure restoration plans. It simulates newest
operations first, preserves journal file order, skips a whole operation with
missing required blobs, and supports surface/keep-current/force conflict policies.
Surface conflicts propose neither writes nor consumed operation IDs. Empty and
absent files remain distinct, including the planned existence and hash anchors.
Ambiguous duplicate paths within one operation are explicitly refused by planning;
the stored history is still readable. Applying a plan must re-admit and revalidate
paths and observations, disable formatting for exact-byte restoration, and record
undo relationships only after success. This chunk does not apply source writes,
claim parse admission, certify an applied receipt or provide rendered tool rows.

Unreadable durable databases use quarantine with visible lifecycle notes and
preserved sidecars. Unknown versions fail closed before writable PRAGMAs. Derived
`native-index.db` recovery remains a separate implementation; conformance rebuilds
a corrupt cache beside an actual reference-produced journal and verifies the
journal bytes and records remain unchanged. Operational failures are errors,
not permission to discard history. Missing or empty main files with surviving
sidecars are refused intact. If replacement creation fails after quarantine,
the error identifies the preserved recovery location.

The [journal corpus](conformance/README.md) embeds actual pinned Bun-produced
SQLite databases. Native tests compare hydrated byte vectors and raw table
records, including compressed blob bytes, before and after migration. A
cross-runtime check appends native zstd content and reads it with the pinned
reference. Fixture verification uses disposable databases and no workspace server.

Current research checked [SQLite WAL lifecycle and reader snapshots](https://www.sqlite.org/wal.html),
[application-owned schema versions](https://www.sqlite.org/pragma.html#pragma_user_version)
and [bounded zstd streaming decoding](https://docs.rs/zstd/0.13.3/zstd/stream/read/struct.Decoder.html).
The pinned native SQLite is 3.48.0, before the published WAL-reset race fix.
The required topology is one admitted writer with true read-only readers and no
independent checkpoint connection; that excludes the documented race between
writable connections. Native backlog `workspace-sqlite-wal-upgrade` records the
dependency update before broader owner/checkpoint integration. Production owner
admission and launcher/protocol cutover remain later chunks.

## Native verified mutations (arch-14c6)

`WorkspaceTransactions` is the shared native byte-mutation pipeline beneath
the forthcoming edit/files/script/plugin/undo use cases. The composing caller
must hold exclusive local workspace admission for preparation and application,
and pass the admitted `WorkspacePolicy`, supervised parser, sole journal writer,
workspace key and explicit session identity. This chunk adds no second lock owner,
daemon, production handler cutover or source certification.

Preparation admits every target, distinguishes missing files from existing empty
files, checks expected SHA-256 hashes and builds a complete, immutable preview.
The preview carries before/after bytes, parser diagnostics and formatter results;
binary bytes are never decoded lossily. Alternate baselines are live, hash-anchored
source paths, not caller-supplied assertions of already-broken syntax. Native
worker parsing and JSON/TOML/YAML well-formedness reject newly broken input.
Already-broken baselines may retain diagnostics without being labeled valid.
Binary changes and deletion skip parsers/formatters; undo requires formatting
disabled and restores exact recorded bytes when parse admission succeeds. As in
the reference, undo refuses a restoration that introduces broken syntax into a
currently valid file, leaving the original operations eligible. Unsupported text is explicitly
distinguished from verified source.

Rustfmt uses the workspace toolchain, Cargo edition and directory configuration.
Biome receives stdin and the target-relative filename. Bounded subprocess I/O and
deadlines turn missing tools, nonzero exits and invalid formatter output into
visible degradation, preserving the admitted unformatted candidate. Formatted
bytes pass verification again. No formatted result is applied before it appears
in the preview.

Native confirmation tokens use a new `mgw1` domain and HMAC-SHA256 with the
owner-managed private key. Length-prefixed fields bind session, operation,
**exact serialized request bytes**, and the ordered finalized plan, including
file existence, contents, permissions, formatting and baseline observations.
Semantically equal but differently serialized requests are not interchangeable.
Verification uses the cryptographic library's constant-time tag verification.
Retained `wb1` tokens are deliberately invalidated; users must preview again
after migration. Tokens survive restart with the same key and fail after key
rotation. They neither admit a session nor provide one-time replay prevention.

Application rechecks paths, byte and permission anchors, then records durable
intent before any per-file atomic publication. Existing permissions are preserved.
Failure triggers reverse compensation and live rereads, including the failed
target. The result distinguishes applied, rolled-back, unchanged, not-applied and
indeterminate files, and includes errors when compensation or durable outcome
recording fails. Multi-file atomicity is not claimed. Parent directories must
already exist; c7 owns directory/trash/move semantics above these file changes.

The first native intent atomically migrates journal v2 to **native v3**, adding
`native_write_intents` and `native_intent_undo` without rewriting retained rows
or compressed blobs. Pending, rolled-back and indeterminate intents stay visible
in native history but are excluded from undo candidates. Undo links are consumed
only when an applied outcome is durably recorded. A process interruption leaves
pending intent for later owner reconciliation. Pending and indeterminate intent
blobs are protected from later retention eviction, with explicit cap-overage notes;
history also retains their deferred undo targets without marking those targets
consumed. Legacy append/read remains v1/v2
compatible until the first native intent. Older reference runtimes intentionally
refuse v3; changing its header back to v2 also fails closed. This avoids exposing
unapplied intent as undoable history through an older reader.

A journal sequence is always an intent/history reference. Even an applied outcome
records a past observation, not an immutable source revision or `SourceSnapshot`.
The later owner and source-certification chunks retain those responsibilities.

Acceptance is exercised by alongside confirmation, journal, formatter and
transaction regressions plus `magistr-cli/tests/workspace_transactions.rs`,
which uses the actual Magistr worker executable and rustfmt. Failure injection
covers forged/stale confirmation, parser unavailability, formatter errors,
journal failure, partial swaps, failed compensation and outcome-recording failure.
The native v3 tests cover atomic migration, recovery/read-only views, deferred
undo links and downgrade refusal. The retained comparator remains a separate
required gate; it is not evidence of native tool parity.

Dependency review adds `hmac 0.12.1` (compatible with existing SHA-256/digest),
`saphyr-parser 0.0.12` and its `arraydeque 0.5.1` dependency. Saphyr requires
`thiserror 2.0.20`, so both `thiserror` and `thiserror-impl` advance from 2.0.18
to 2.0.20; the latter now uses the already-present `syn 3.0.4`. The
[thiserror patch release](https://github.com/dtolnay/thiserror/releases/tag/2.0.20)
addresses a generated-code Clippy warning. These exact versions were reviewed
and tested in the native dependency graph. The HMAC crate forbids unsafe code;
the parser and thiserror sources contain no unsafe code. Arraydeque contains
unsafe storage internals, although
this integration selects Saphyr's string-input path rather than its buffered-input
adapter. This is a focused dependency review, not a complete security audit.
The YAML parser streams events rather than constructing
or expanding an alias graph, and runs inside the supervised worker. Research
checked [HMAC verification](https://docs.rs/hmac/0.12.1/hmac/),
[Saphyr's parser API](https://docs.rs/saphyr-parser/0.0.12/saphyr_parser/struct.Parser.html),
[the parser maintainer's migration guidance](https://github.com/Ethiraric/yaml-rust2/issues/26),
[temporary-file publication](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html#method.persist)
and [Rust filesystem race guidance](https://doc.rust-lang.org/std/fs/).



## Native editing, file operations and applied undo (arch-14c7)

`commands::WorkspaceCommands` is the native use case above the c6 pipeline:
`edit` (replace_body, insert_after, insert_before, rename, rewrite,
replace_text), `files` (move, copy, create, delete, mkdir) and `undo`. Every
verb resolves its intent into exact `TransactionChange` anchors (missing or
SHA-256, plus the source baseline for move/copy) and calls `prepare`; nothing
in `commands` writes, renames or unlinks a workspace file. The composing host
supplies the admitted policy, supervised worker, sole journal writer, workspace
key, optional derived index and an explicit session identity per service. No
tool handler, workspace owner, owner-bound session fencing or production cutover
is added here.

The prepared plan's `mgw1` token is the confirmation token. Rewrite, rename,
move and copy-over-a-differing-target always gate; any op touching more than
one changed file or more than 200 changed lines gates; `dryRun` returns every
row unapplied after the full verification/formatting pipeline and `autoApply`
is the trusted-harness bypass, exactly as retained. The intent bytes the token
binds are the serialized call without `confirm`, `dryRun`, `autoApply` and
`budget`, so the identical intent re-invoked with a token reproduces the same
sealed request. Because the native seal binds session, exact intent and the
finalized plan together, a presented token that does not reproduce the fresh
plan answers `CONFIRM_MISMATCH` whether the files moved, the arguments changed
or the key differs; the retained implementation split the first case out as
`STALE_ANCHOR` from its separate content hash. `STALE_ANCHOR` with the fresh
hash is still answered for an `expectedHash` mismatch, an absent `replace_text`
anchor or an out-of-range occurrence, and the pipeline re-checks every anchor
before publication. Retained `wb1` tokens are refused. A verb that resolves to
no change answers the nothing-to-change envelope before any plan is prepared,
and a presented token for such a plan is a mismatch.

Symbol verbs require the validated derived outline whose content hash equals
the bytes being edited; anything less is `INDEX_UNAVAILABLE` with progress
notes. `rename` finds its files through the native references search (a warm
index, definition rows plus import-linked and bare occurrences) rather than
raw index edges; the note carries the confidence counts and a truncated scan is
reported instead of claiming completeness. `rewrite` runs the worker's
ast-grep rewrite over the admitted live walk within the glob/lang scope, skips
files with no bundled grammar and says so, and is bounded to 10,000 candidate
files and 64 MiB of source. Files opened by a verb are read through the same
no-follow opener as queries and are bounded to the pipeline's 4 MiB.

Deletion is journal-backed and physical: the swap renames the admitted file
into `.workbench/trash/<random>-<name>` with pinned directory descriptors and
never unlinks; the journal remains the restoration source. Missing parents of
a written file are created on the admitted path before the pinned swap, so
`create`, `mkdir` (a journaled `.gitkeep`) and moves into new directories work;
a rolled-back creation leaves the empty directory behind, which the receipt
does not hide. Import rewriting ports the four specifier families verbatim
without look-around regexes (boundary checks are explicit), and Rust moves port
the module-tree derivation, declaration confirmation, contiguous path rewrite,
brace-group reporting and manifest target updates as a pure planner. Because
the pipeline formats Rust with the workspace's rustfmt, a declaration inserted
into an existing parent may be reordered by `reorder_modules`; the retained
rig used a passthrough formatter and never showed this.

`undo` is never confirm-gated. Candidates come from the journal's session or
named-sequence eligibility; live bytes are read for every touched path (absent
is distinct from empty); the c5 plan surfaces conflicts as three-way hash rows
with no writes and no consumed operations, `keepCurrent` keeps the conflicted
file while rewinding the clean ones, and `force` restores the journaled bytes.
When nothing differs from disk the choice is still consumed by a bookkeeping
intent with no files. Restorations run with formatting disabled and exact
anchors; a refused restoration (a stale race, or journaled bytes that would
break a now-valid file) leaves the operations undoable, and a failed
application reports each file's observed effect. `action: "list"` is not
ported: the journal's native `recent` query already serves that read.

Acceptance is exercised by the alongside unit suites (worker-free text and
binary lanes, planner and import fixtures) and
`magistr-cli/tests/workspace_editing.rs`, which uses the actual Magistr
parser worker, derived index, rustfmt and `cargo check` over the retained
six-language verb and move fixtures and the Rust fixture crate, plus stale
previews, rename ambiguity, mixed create/delete/write batches, a read-only
directory partial failure with compensation and rolled-back intent, and undo
after external edits. Verification is on this macOS host. The `similar` crate
(already locked at 2.7.0) renders unified diffs; research checked
[its unified-diff API](https://docs.rs/similar/latest/similar/udiff/index.html)
and the [regex crate's deliberate lack of look-around](https://github.com/rust-lang/regex/discussions/910).

## Native extensions and script host (arch-14c8)

`extensions` is the native plugin coordination layer above the retained
Wasmtime lanes. A `plugin.toml` is validated by `PluginManifest::parse` over the
locked `toml` crate with the retained rules: fail-closed identity, tools and
component path, reserved core names refused, unknown tables warned, derived
`publisher.pkg/tool` wire names that always contain a slash. The
`.workbench/plugins.json` inventory and every manifest are read per call
through the workspace jail; a source other than exactly `registry` is a
sideload and a tier is honored only on registry entries, so a sideloaded record
cannot label itself trusted. Effective capabilities are declared ∩ granted with
the same-major grant rule, and every `workbench:subprocess` stamp is reserved to
tier `blessed` — a grant a human was talked into giving anything lower is
ignored with a note, so the guest is refused before instantiation. The breaker
is `.workbench/plugin-health.json`, keyed by plugin label and component SHA-256;
a breach short-circuits later invokes without touching the worker until the
bytes change. `PluginHost` answers `PLUGIN_CAP_DENIED` (no breach),
`PLUGIN_CRASHED` (breach, `plugin` payload) and `ENGINE_UNAVAILABLE` (dead lane,
no breach) and host-wraps JSON results; core queries never consult any of it.

Every guest executes through `ExtensionLane`. The production lane is
`WorkerExtensionLane` over the c2 supervised worker, so untrusted bytes never
load into the application owner's process; component bytes travel with each
request under the existing 32 MiB frame ceiling and quarantine keys. The worker
gains `RankingContribute` and `RunnerParse` requests over the retained ranking
and runner lanes. Fixed while landing the script host: the worker executed
requests inline on its Tokio runtime thread, and `wasmtime-wasi` blocks on an
inner runtime there, which Tokio refuses — every WASI request killed the child.
Requests now run on a blocking thread; a panic while executing still ends the
worker, so crash and quarantine handling is unchanged. The WASI lane keeps the
last four compiled modules by guest digest, because the script interpreter is
the same 1.5 MiB module on every round and JIT compilation dominated a round.

`tck::run_plugin_tck` ports the four chapters: interface (manifest, component,
every tool answers the canonical payload with JSON), capabilities
(declared-sufficiency and least-privilege under zero grants), resources
(starvation fuel and a tiny cap per tool, plus the kit-owned spin and grow
fixtures that must die typed through the same lane) and the lang-pack fuzz
corpus for a `classify-file` tool. The corpus is the retained seeds plus
xorshift32 mutations, deterministic run to run; it mutates chars rather than
UTF-16 units because Rust strings cannot hold the lone surrogates the reference
corpus produces, so the native corpus is stable but not byte-identical to the
reference's. The kit reads the plugin directory with plain filesystem calls,
outside any jail, by design.

The `script` verb on `WorkspaceCommands` retains the bootstrap program and
nonce-framed sentinel protocol verbatim: deterministic re-entry serves host call
k from the injected journal, the shim reports divergence as a typed guest
failure, and only exit-0 rounds are parsed. Host calls resolve to the typed c4
queries (`search`, `read` behind the read jail, `glob` over the admitted walk
with the same globset dialect) and the guest sees the unredacted envelope
shape; the script's return value is redacted when the script envelope is
sealed. The 32-call ceiling, `SANDBOX_FUEL_EXCEEDED`, `SANDBOX_MEMORY_EXCEEDED`
and `SANDBOX_CAP_DENIED` mappings, and the "interpreter failed is
`ENGINE_UNAVAILABLE`" rule are retained. The final queue resolves to
`TransactionChange` anchors — replace-all or one occurrence with `STALE_ANCHOR`
and the fresh hash, `create`, a journaled `.gitkeep` for `mkdir`, `delete` —
and runs through the c7 gate as always-gated with the intent bytes `{source,
dryRun}`. Two deliberate deviations: the write jail decides before any read of
a queued path, so a queued write to a protected or out-of-jail file answers
`SANDBOX_CAP_DENIED` before a token is minted (the reference minted the token
and refused at apply); and as in c7 a presented token whose plan no longer
reproduces answers `CONFIRM_MISMATCH` even when the anchor text still matches.
The interpreter is the retained `vendor/qjs/qjs-wasi.wasm` loaded by the host
(`ScriptInterpreter` reports its SHA-256); guest bytes travel per round as a
JSON byte array, which the module cache offsets but the frame encoding does not
compress. QuickJS number formatting can differ from serde's for a few edge
values (`1e21`, negative zero), which the shim would report as nondeterministic
replay rather than serve.

Lanes: `LangPackBoot` discovers `lang-pack-` packs, reads the carrier through
the worker and registers the language there; the tier selects the lane
(blessed promotes to native, everything else runs the grammar in the WASM
store), a failing pack breaches and stays lexical-only, and boot is idempotent
per language. Registration lives in the worker, so a worker restart forgets it
and boot must re-run; host-side language selection for the index and live
queries still uses the bundled table (backlog). `RankingSignals` keeps the
persisted promotion store `.workbench/ranking-promotions.json` bound to the
component hash, the 5 MiB `.workbench/ranking-shadow.jsonl` with oldest-half
eviction, a 50,000-entry feature cache per signal and the post-response scorer;
`after_search` spawns on the current runtime and never blocks the search
response, promoted features apply only through the synchronous
`PluginFeatureLookup` seam folded into the first-party combiner (weight table
`ranking-visibility/exported` = 0.5, unlisted names weigh zero), and guest
feature names are namespaced and sanitized at the boundary. `PluginResolver`
consults the blessed, subprocess-granted resolver only with detected specs and
a warm index, breaches on a crash, budget expiry or a malformed/out-of-jail
location with a 60 s retry window, treats cap-denied as configuration and a
dead lane as degradation, and its edges merge per edge into references search
(`resolver-lsp`/high; nothing unconfirmed is relabeled). `LanguageServerSpec`
detection ports the TypeScript and Cargo probes with injectable lookups.
`RunnerAdapter` runs `can-parse` then `parse-output`; `None` is the guest's
honest "no structure", a dead lane, trap or unclaimed command is a typed error
the run tool degrades from. The run tool's bundled fallback floor and declared
command execution remain c9; served tool rows, map plugin inventory and
production composition remain c10.

Acceptance is exercised by the alongside suites over scripted lanes and
worker-free `.txt` harnesses, and by `magistr-cli/tests/workspace_extensions.rs`
and `workspace_script.rs` through the actual Magistr worker: the TCK over the
SDK example plugins and every bundled plugin unmodified, real WAT components for
capability denial, the durable breaker and core continuity, the blessed-only
subprocess grant over the real resolver guest, the gleam pack registering on the
wasm and native lanes with identical extraction, the ranking signal scoring
through search with promotion and demotion, the resolver lane over the compiled
LSP fixture merged into references, the runner goldens, and the script tool
over the pinned QuickJS-ng guest (the ten one-liners, fuel/memory/capability
kills, redaction at the seal, guest failure as data and the confirm flow).
Verification is on this macOS host; no dependency or version changed.

## Native execution and diagnostics (arch-14c9)

`crates/magistr-workspace/src/execution/` ports `src/tools/run.ts`,
`run-parse.ts` and `run-cargo.ts`; `crates/magistr-workspace/src/diagnostics/`
ports `src/tools/journal-view.ts`, `src/doctor/{probes,report,bundle,tar}.ts`,
`src/config/doctor.ts` (already native as `ResolvedConfig::doctor`),
`src/protocol/tracing.ts`, `src/ports/tracing.ts` and the `workbench_status`
answer from `src/protocol/server.ts`. The reference is unchanged.

Execution: `WorkspaceRun` is composition over the c1 `CommandAdmission` — the
allowlist, the no-shell guard and the munging redirect decide before any
executor is consulted and nothing here widens them. The executor is a seam
(`RunExecutor`, boxed future plus `CancellationToken`) whose production
implementation `ProcessExecutor` spawns one literal argv in the workspace root
with stdin closed, both streams piped, `kill_on_drop` and its own process
group; it captures each stream under a ceiling (three quarters head, one
quarter rolling tail, the dropped byte count reported and a marker line that
no parser matches), waits under a wall-clock deadline or the cancel token, and
on either kills the owned group by its own pid (never a caller-provided pid)
before reaping. A drain deadline covers a grandchild that holds the pipes after
the parent exits. Native safety deltas versus the reference's Bun executor:
bounded output (the reference buffered everything), a typed `FILE_NOT_FOUND`
for a missing program and `ENGINE_UNAVAILABLE` for other spawn failures (the
reference threw), signals reported as data with no exit code, and cancellation
as a first-class terminal outcome. Parsing is pure and byte-for-byte pinned:
the seven bundled runner parsers, the panic-block/backtrace helpers and the
rustc/clippy extractor reproduce every retained cargo golden by JSON equality,
the JS regexes are ported with ASCII word boundaries, and the retained
semantics that a claimant without structure ends the search (the honest
head+tail cut, never a weaker parser's guess) is kept. The registries are open
trait lists (`RunnerParser`, `DiagnosticExtractor`, `PluginRunnerLane`) with
bundled defaults in the retained trust order; the plugin lane is the c8
`RunnerAdapter` consulted first, a dead lane degrading with a note. Budget
fitting, the diagnostics manifest (`diagnostic:N`) and the head+tail line
allocation (budget × 4 / 80, one quarter to stderr) are the reference's; the
manifest rides `ResultSet::with_manifest`, added for a cut made inside a row.

Diagnostics: `WorkspaceStatus` answers root, state dir, the detected journal
mode, the envelope versions and the worker/index snapshots the composition
holds, degradations in notes. `JournalInspection` renders the retained
journal-view rows (one row per touched file, thin rows for fileless ops,
`evicted`, `binary`, the 20,000-unit diff cap) over the read-only
`JournalReader`: no lock, nothing rewritten, a missing journal is an empty
listing and a newer journal is `UNSUPPORTED_VERSION` with the file untouched.
`TraceRecorder` keeps the reference's structural privacy (typed attributes
only, no free-form attach), OTel id widths, derived recovery classes, the
8 MiB cap keeping the newest half, torn tails tolerated and never repaired; it
adds the credential redactor on every line and a dedicated writer thread with
a bounded flush handshake so the served path never awaits trace IO. There is
no async-local active span: the composition passes spans explicitly.
`DoctorRun` composes the six retained sections read-only: the index probe
opens `native-index.db` (the native cache, not the retained `index.db`) with
`SQLITE_OPEN_READ_ONLY | NOFOLLOW` after canonicalizing the state dir and never
touches `native-index.lock`; the watcher line says status is in memory;
quarantine reads the retained `quarantine.jsonl` tolerant of torn lines and
says the native worker ledger is per session; plugins read the inventory and
breaker files under the sandbox feature and say so when the host is not
compiled in. `DoctorBundle` is the retained members under the retained
`workbench-doctor/` prefix, every member through `redact_bundle_text` (root
spellings, foreign absolute paths) and then the credential redactor, packed by
the in-tree ustar writer and `flate2` gzip (the one new direct dependency,
already in the lock). Doctor CLI verbs and exit codes (0/1/64/65/66), the
lock-free `journal` CLI subcommand and served tool rows remain c10.

Evidence: 50 execution tests (real-process executor: exit codes, missing
program, timeout kill, cancel kill of grandchildren, pipe-holding grandchild,
noisy output bounded, signals; scripted-executor port of `run-tool.test.ts`;
`run-cargo.test.ts` goldens and assertions; plugin-lane fakes) and 22
diagnostics tests (status notes, journal rows beside a live writer with a
state-dir byte snapshot, newer-journal refusal, trace contract/cap/torn
tail/unwritable/redaction, doctor seeded stores, corrupt and newer index,
read-only beside a live `IndexCache` holding its lock, no state creation,
bundle ustar conformance through an independent reader, sentinel privacy,
support-engineer derivation), plus `tests/live_cargo_run.rs` running the real
toolchain over a fixture workspace through the production executor. Limits:
verification on this macOS host; on Windows the group kill degrades to
`kill_on_drop`; the plugin runner lane over a real worker guest is covered by
the c8 suites, not re-run here; the live cargo test skips honestly without
cargo on PATH.
