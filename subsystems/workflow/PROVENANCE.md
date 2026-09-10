# Workflow source provenance

Magistr owns this repository-local fork of
[think-and-ship](https://github.com/AlrikOlson/think-and-ship). The source adoption
is `arch-14a-workflow-source-fork`; typed use-case extraction is the separate
`arch-14b-workflow-use-cases` chunk.

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

Paths below are relative to the upstream repository and retain their layout
beneath this directory:

- `crates/think-and-ship/`: library, binary, build script, manifest, README,
  examples, unit and integration tests, bundled `skills/`, and `skill-clients/`.
- `contract/` and `shared/`: schemas and fixtures consumed by inherited tests.
- `docs/`: inherited architecture, protocol, setup, and compatibility references.
- `scripts/`: upstream source scripts; compiled Python cache files are excluded.
- `.githooks/commit-msg`: fixture required by `no_process_narration`; imported
  without activating the hook in Magistr.
- `LICENSE`, `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`,
  `CODE_OF_CONDUCT.md`, and `deny.toml`.

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
`scripts/release/__pycache__/` is excluded even though upstream tracked it.
Ignored local artifacts, credentials, build products, and workflow stores are
not source inputs.

The upstream root `Cargo.toml` and `Cargo.lock` are replaced by Magistr's root
workspace manifest and lockfile. The adopted crate is a workspace member;
`magistr-engine` and `magistr-cli` use direct repository-relative path
dependencies on it. Its inherited `clap` and `anyhow` declarations resolve
through Magistr's workspace dependencies. There is one release-coupled
think-and-ship implementation and one Cargo resolution for the workspace.

Preserving `crates/think-and-ship/` two levels beneath this directory keeps
the inherited manifest-relative fixture paths meaningful. The build script
still embeds the complete skill trees. Source adoption keeps package and
library names compatible with current consumers; it does not change native
store paths, schema, project identity, or persistence policy, and performs no
live-store migration.

## Test provenance and verification

The imported tests come from the revision above. They retain engine and
persistence regressions, MCP and HTTP transport tests, cross-family behavior,
schema and documentation checks, skill evaluations, and tracker/cloud tests.
The `contract/`, `shared/`, and `docs/` trees are included because these tests
read them through the preserved layout. External-service smoke tests retain
their upstream opt-in requirements; a normal workspace run does not certify
those live integrations.

The source-adoption chunk changes Rust tests in two files:

- [`tests/think_config.rs`](crates/think-and-ship/tests/think_config.rs): two
  current-directory identity tests re-execute in a disposable, unmarked
  project child process. This prevents Magistr's ancestor project marker
  from changing their fixture inputs, while preserving the original
  assertions and avoiding a process-wide working-directory race.
- [`src/cli/setup.rs`](crates/think-and-ship/src/cli/setup.rs): the inline
  external-configuration test asserts that JSON reserialization changes the
  original bytes. It no longer requires sorted keys, because the host's
  unified `serde_json` features enable `preserve_order`. The test still proves
  that the external configuration is found and never rewritten.

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
Inherited violations cannot grow; new files must satisfy the normal limits.
The nested source location grants no blanket vendor exemption. All other
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
