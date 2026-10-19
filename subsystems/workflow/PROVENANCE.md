# Workflow source provenance

The [project pre-release policy](../../AGENTS.md#before-public-release) governs
this material and every imported document, schema, fixture and test beneath
this directory. They record source history and current behavior; they impose no
backward-compatibility, old-format migration or API preservation requirement.
When the intended Magistr design changes, update current contracts and tests
together. Preserve attribution and accurate provenance without treating the
imported behavior as frozen.

Magistr owns this repository-local fork of
[think-and-ship](https://github.com/AlrikOlson/think-and-ship). The source adoption
is `arch-14a-workflow-source-fork`; typed use-case extraction is the separate
`arch-14b-workflow-use-cases` chunk; `arch-14f4-rehome-naming` moved the crate
to [`crates/magistr-workflow`](../../crates/magistr-workflow/Cargo.toml) under
Magistr's name and deleted the standalone surfaces (see
[Rehome and retired surfaces](#rehome-and-retired-surfaces)). This directory
keeps the provenance record and the imported inputs the crate's tests and
gates still read.

## Origin and license

| Field | Value |
| --- | --- |
| Upstream repository | `https://github.com/AlrikOlson/think-and-ship` |
| Audited and imported revision | `8c10c1fcfb479e90b33c59502eabd92586cf0761` |
| Upstream package | `think-and-ship` 0.6.0 |
| Source recheck | 10 September 2026; checkout clean at the audited revision |
| License | [MIT](LICENSE), copyright 2026 Alrik Olson |

The original checkout is a research and import input. Building or running
Magistr does not require it. This fork is ordinary tracked source, without a
Git submodule, sibling-checkout dependency, or temporary Cargo override.
Future upstream changes require a reviewed import with a new revision record;
Magistr's commits record local modifications from this baseline.

## Included source

The import retained the following upstream paths, relative to the upstream
repository. Where they live now:

- `crates/think-and-ship/`: library, manifest, unit and integration tests and
  the bundled `skills/` were imported into [`crates/magistr-workflow`](../../crates/magistr-workflow/Cargo.toml)
  (package `magistr-workflow`, library `magistr_workflow`). The binary, build
  script, README, examples and `skill-clients/` were deleted by
  `arch-14f4-rehome-naming`. The unused inherited skills and their tests were
  subsequently removed when Magistr's CLI became the sole owner of agent
  templates; current skills are verified by the CLI scaffold and workflow tests.
- `contract/` and `shared/`: schemas and fixtures consumed by inherited tests;
  still here, read through `../../subsystems/workflow/…` from the crate.
- `docs/`: inherited architecture, protocol, setup and compatibility
  references; still here. `CLI.md` and the setup prose describe the retired
  standalone binary and are kept as the import's reference material.
- `.githooks/commit-msg`: fixture required by `no_process_narration`; imported
  without activating the hook in Magistr.
- `LICENSE` and `README.md`.

The initial source copy was byte-identical to the corresponding tracked files
at the revision above. Build integration and subsequent source changes belong
to Magistr's history; this statement describes the import baseline, not a
promise that the owned fork remains unchanged.

## Excluded source and replaced build metadata

The viewer application (`crates/think-and-ship-viewer/`) and npm binary
distribution (`npm/`) are excluded. They are separate product and distribution
surfaces, not dependencies of the adopted Rust service. Upstream repository
automation and metadata are also excluded: `.github/`,
`.impeccable/`, `.dockerignore`, `.gitattributes`, `.gitignore`,
`.think-and-ship/project.json`, `justfile`, and `release-plz.toml`.
The upstream packaging files that the import first retained (`CHANGELOG.md`,
`CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `deny.toml`,
`docs/deploy/`, `docs/RELEASING.md`, `docs/DEPLOYMENT.md`, `scripts/`) were
deleted by `arch-14f4-rehome-naming`: they described releasing a product
Magistr does not ship.
Ignored local artifacts, credentials, build products, and workflow stores are
not source inputs.

The upstream root `Cargo.toml` and `Cargo.lock` are replaced by Magistr's root
workspace manifest and lockfile. The adopted crate is a workspace member;
`magistr-engine` and `magistr-cli` use direct repository-relative path
dependencies on it. Its inherited `anyhow` declaration resolves through
Magistr's workspace dependencies. There is one release-coupled workflow
implementation and one Cargo resolution for the workspace; every first-party
crate carries the same version.

## Rehome and retired surfaces

`arch-14f4-rehome-naming` (12 September 2026) moved the crate from
`subsystems/workflow/crates/think-and-ship` to `crates/magistr-workflow`,
following the workspace precedent (engine under `crates/`, provenance under
`subsystems/`), and renamed it:

| Before | After |
| --- | --- |
| package `think-and-ship` 0.6.0 | package `magistr-workflow` 0.1.0, `publish = false` (the upstream package and version are recorded above) |
| library `think_and_ship` | library `magistr_workflow` |
| `magistr_engine::tas` (`TasConfig`) | `magistr_engine::native` (`NativeConfig`), the native workflow store beside the engine's own `workflow` phase engine |
| crates.io metadata (`readme`, `repository`, `homepage`, `keywords`, `categories`) | dropped; this record carries the origin |

Deleted as retired standalone surfaces (decided by `arch-14f-native-application`):
the `think-and-ship` binary (`src/main.rs`, `[[bin]]`) and its CLI
(`src/cli/`: serve, init, project, skills, roadmap, prune/adopt, doctor,
status, trace, otel, corpus, sync, calls, telemetry, tracker, repair,
connect/token/disconnect), the `build.rs` skill embedding and
`skill-clients/`, `examples/cloud_census.rs`, the crate README, the CLI-only
`corpus` and `hygiene` modules, `otel_receipt` and the OTel retention note
(both read only by `otel send`), and the tests that executed or parsed the
binary (`docs_command_reachability`, `http_transport`,
`project_identity_only`, `roadmap_blocker_command_seam`,
`roadmap_next_command_seam`, `roadmap_status_command_seam`, the two spawned
tests in `roadmap_export_tool_seam`). The library plumbing the served
`tracker_*` tools reached through the CLI — configuration and credential
resolution and the table-taking port builder — now lives in
`tracker::host`, and `tracker_command_seam` drives the served setup path
through it.

Unchanged, and proven by the retained suites: tool names and the served
tool surface, `THINK_AND_SHIP_*` environment variables, the `think-and-ship`
data directory and `.think-and-ship/project.json` identity, persistence
schemas and versions, the cloud envelope contract, OTel `service.name`,
scope name and `think_and_ship.*` attribute keys, the repo-sync tool name and
the keychain service name. Two things did change with the module path:
`tracing` targets follow the crate (`magistr_workflow::…`, and the OTLP log
fence with them), and `tracker_setup` no longer accepts a push interval at
all — the cadence that read it belonged to the standalone server, and
`audit-260912-37-integration-promises` retired that cadence, the cloud
realtime subscriber, the tracker doorbell, the sweep schedule and the push
receipt store from the fork rather than carry a schedule Magistr never
composes. A supplied `push_secs` is refused by name. Automatic cadences are
deferred product work, recorded in `docs/architecture/owned-subsystems.md`.

## Test provenance and verification

The imported tests come from the revision above. They retain engine and
persistence regressions, MCP transport tests, cross-family behavior,
schema and documentation checks, skill evaluations, and tracker/cloud tests.
The `contract/`, `shared/`, and `docs/` trees are included because these tests
read them through the preserved layout. External-service smoke tests retain
their upstream opt-in requirements; a normal workspace run does not certify
those live integrations.

The source-adoption chunk changes Rust tests in two files:

- [`tests/think_config.rs`](../../crates/magistr-workflow/tests/think_config.rs): two
  current-directory identity tests re-execute in a disposable, unmarked
  project child process. This prevents Magistr's ancestor project marker
  from changing their fixture inputs, while preserving the original
  assertions and avoiding a process-wide working-directory race.
- `src/cli/setup.rs` (since deleted with the CLI): the inline
  external-configuration test asserted that JSON reserialization changes the
  original bytes without requiring sorted keys, because the host's unified
  `serde_json` features enable `preserve_order`.

At the source-adoption commit `25b7227`, imported production Rust was unchanged.
That baseline suite recorded 1,737 passed and 14 skipped with both test
adjustments applied. The subsequent use-case extraction changes production
source as described below. Host verification
also exposed a goal-extension transcript ordering bug, repaired at the transport,
session-supervisor, and loop-driver boundaries, and a blocking stdio read that
prevented an attached MCP client from exiting after owner loss. The proxy now
uses cancellable stdio, and its process tests preserve client input while checking
owner loss. These host repairs are separate from the imported workflow behavior.
Acceptance requires fresh recorded workspace checks for the integrated implementation.

Magistr's configured workspace formatting, check, Clippy, and nextest commands
include both Magistr crates and the adopted crate. The required
[`workflow-source` gate](../../scripts/workflow-source-gate.sh) verifies locked
dependency resolution, retained source inputs, and the disabled roadmap-export
policy. It also runs the imported structure gate against
[`STRUCTURE_BASELINE.json`](STRUCTURE_BASELINE.json), an explicit per-file
baseline for inherited public-item, inline-test, and long-function counts.
Inherited violations cannot grow; new files must satisfy the normal limits;
the baseline's keys follow the crate to `crates/magistr-workflow/`.
Living under `crates/` grants no blanket vendor exemption, and the
`native-write-path` gate's composition rule treats the crate as the thing
being composed rather than as a composer. All other
configured required gates remain part of acceptance.
Recorded checks in native ship state identify the actual implementation
worktree and results; earlier architecture-only checks do not certify this
import.

## Migration boundary

`arch-14b-workflow-use-cases` adds native application modules for reasoning,
ship, roadmap, and signals. Validation, reciprocal-family operations, tracker
setup, and ordinary completion policy now sit beneath CLI/MCP adapters.
Synchronous commands return typed values; command execution, provider probes,
and approval waits use explicit async capabilities. Native callers no longer
serialize MCP arguments, decode tool results, or poll handlers with a noop waker.
Existing tool names and versioned owner replies remain transport contracts.

Ship state gains optional execution identity metadata in its existing native
store. Legacy stores remain readable; new objective runs and task instances
receive fresh identities. Check admission captures project/run/task instance
before execution, and settlement compares the persisted identity under the
existing file lock before appending. A stale outcome is returned intact rather
than attached to a reused task name. This is a correlation interface, not durable
CheckReceipt storage or exactly-once settlement; inherited general mutations
retain their merge semantics and ordinary finalization warnings.

New regressions cover native/MCP success and refusal parity, reciprocal-family
behavior, tracker probe concurrency and consent, legacy identity, task reuse,
peer rollover, and stale check settlement. Full configured gates are rerun on
the actual integrated main checkout. Workspace source adoption, ownership, and
`SourceSnapshot` production remain later chunks.

The uncommitted receipt draft remains an unverified recovery checkpoint:
native `think:714`, architecture closeout `think:715`, and the exact recovery
bundle identified in [owned subsystems](../../docs/architecture/owned-subsystems.md#delivery-and-preserved-work).
The owner requested main-only development; completed commits and the active
extraction moved to `main`, and this run's isolated worktree was retired.
The archived draft is neither imported nor certified by these checks. Receipt
consumption remains deferred to blocked `arch-14-verification-receipts`.
