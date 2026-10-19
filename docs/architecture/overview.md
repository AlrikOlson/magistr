# Architecture

Magistr is unreleased. The [pre-release policy](../../AGENTS.md#before-public-release) prohibits backward
compatibility in every design decision until the owner declares official public
release. Existing aliases, older formats and reference parity describe current
or historical implementation; none is a requirement to retain that behavior.

Magistr separates project configuration, workflow state, agent transport, and
presentation. The root Cargo manifest groups the public Rust crates into one workspace.
The desktop has a separate workspace and lockfile for its OS WebView dependencies.
The [contract baseline](contracts.md) maps current owners and entry paths,
records the refactor's intended policies, and names executable behavior scenarios.
The [system contracts](system-contracts.md) page is the maintained glossary,
the dependency, runtime and deployment views, the end-to-end traces and the
exercised entry, build and platform matrix.

| Component | Responsibility |
| --- | --- |
| `magistr-cli` | Commands, project setup, runtime integration, concrete workflow/workspace interface clients, and MCP service startup |
| `magistr-domain` | Deterministic identities, run rules, policy, and verification values |
| `magistr-application` | Project-scoped use cases, typed outcomes, and narrow adapter ports |
| `magistr-workflow-contracts` | Canonical workflow records, typed mutation frames, snapshots and freshness values, independent of stores and runtimes |
| `magistr-core` | Configuration, presets, and shared project/interface types |
| `magistr-engine` | Sessions, workflow policies, costs, gates, native workflow integration, and the one producer of typed workflow snapshots and events |
| `magistr-workspace` | Native configuration/path/key policy, response and byte contracts, parsing/search/rewrite, index extraction, language registry and sandbox primitives, bounded native worker supervision, persistent derived index with watchers and typed source queries, durable journal/undo planning, confirmed verified byte transactions, native commands and served tool parity, generation-fenced single-owner admission with explicit sessions, Unix client attachment through the shared owner endpoint; the bridge, launcher and worker lifetime live in `magistr-cli` |
| `magistr-process` | Owned child reaping, Unix process groups and hard-exit registry, bounded command capture and termination |
| `magistr-acp` | Agent Client Protocol transport and configured local effect adapter |
| `magistr-acp-agent` | Coding-runtime and chat-API provider adapters |
| `magistr-mcp` | In-process MCP tool definitions and request handling |
| `magistr-cockpit` | Semantic session/workflow messages, controls and effect ports |
| `magistr-cockpit-runtime` | Shared message channels, conversation/planning coordination and session relays over application handles and effect ports, arrival measurements and bounded background jobs for hosts and surfaces |
| `magistr-desktop` | Tauri Rust host, semantic HTML/CSS, typed session commands and bounded acknowledged channels; private child roles and settled shutdown; separate Cargo workspace |
| `magistr-theme` | Shared color and theme vocabulary |

The [domain boundary](domain.md) enforces effect-free dependencies. Existing
engine re-exports are implementation details that may be removed with their callers.
The [application contracts](application.md) define project-scoped use cases and
effect ports. The [host effect adapter](effects.md) routes ACP filesystem and
terminal requests through configured capabilities; further host migration remains
separate.
The [permission service](permissions.md) supplies one decision implementation for
received agent requests and host effects across cockpit and noninteractive sessions.
[Durable operations](durability.md) record what a run dispatches before each
effect and reconcile interrupted attempts against their authorities on recovery.
The [SQLite stores](stores.md) inventory names every database file, its
journal mode, its writers, checkpointers and readers, and the bundled runtime
floor that keeps each topology safe.

## Workflow and tools

The CLI's MCP service embeds the repository-local
[workflow source](../../subsystems/workflow/PROVENANCE.md). Its roadmap,
reasoning, execution, and signal tools share native project state. The
workspace tools are served by the same binary's native workspace bridge
(`magistr mcp --dir`), every client of a workspace reaching its
one admitted owner. The
[project authority](project-authority.md)
controls cooperating MCP owners; a second process must not open an independent
cached writer for the same project. The [client inventory](clients.md) names
every supported surface's capability and admission path, and states what
is coherent across the project's owner, working-copy executions, attached
clients and readers.

The accepted [first-party subsystem decision](owned-subsystems.md) makes
workflow and workspace services Magistr-owned code. It defines
workflow and workspace ownership, source certification and operation contracts. The accepted target is one Rust-native Magistr product. Workflow
source and the adopted `magistr-workspace` engine belong to the root Cargo
workspace. Typed
reasoning, ship, roadmap, and signal use cases now own native behavior beneath
MCP and local owner adapters; command execution and tracker probes are explicit
async effects. The complete Workspace reference source, contracts and tests are
retained with pinned provenance under `subsystems/workspace/reference`; Bun and
NAPI are reference-only dependencies. The
[source-to-native feature inventory](../../subsystems/workspace/FEATURE_INVENTORY.md)
maps the delivered native policy, worker, index, source-query, journal,
transaction, extension, command and served-tool boundaries. The private
`magistr --workspace-worker` role runs parser, grammar and sandbox operations
under `WorkerSupervisor`; it has no workspace lock or independent service
lifecycle. Admission fences competing owners, stale sessions and
previous-generation confirmations. The [workspace owner](workspace-owner.md)
serves multiple Unix MCP clients over one admitted service, keyed by canonical
root independently of workflow roles: every invocation is a bridge that stamps
a durable client identity and reconnects (attaching or admitting) on its next
request when the owner goes away, and workers end with a killed owner. Source
snapshots bind verification receipts to their observed inputs.

[Native workflow configuration](configuration.md) is captured before admission;
reader, writer and attachment paths use its typed settings and identity inputs.
Project setup enters through the CLI's detection module, which calls the core
stack detector with the CLI's plugin results. Setup writes
`.magistr/project.json` and agent-specific files from the CLI crate's templates
and the core crate's presets. The project file supplies commands, custom gates,
testing conventions, and native-roadmap settings. Generated files carry
ownership rules so refresh and removal can preserve user changes; setup,
repair and owned removal each compute one
[administrative change plan](configuration.md#administrative-change-plan)
(expected state, ownership, apply/skip/refuse intent per path) and execute
exactly that plan through one administrative service that revalidates each
target at its write and answers a per-change receipt, so the dry-run preview
is what is written and a partial outcome is reported as one.

## Sessions and interfaces

[Session configuration](sessions.md) chooses an agent provider. ACP carries
session updates and permission requests between the runtime adapter and Magistr.
The CLI host's session supervisor owns turn admission, permission handling,
terminal observations, and provider residency for both headless and cockpit
sessions. The cockpit uses an application session handle. Providers differ in
model discovery, resumption,
tool use, and cancellation; chat APIs do not supply the same coding-session
behavior as external runtimes.

The [terminal](terminal.md) renders the cockpit's `Model` as a cell tree and
sends `Msg` values to its update function. Effectful work returns messages to
the shared state instead of mutating it from renderer code. What the model
shows of the store arrives as typed [projections](projections.md): one
producer, a whole snapshot and a sequenced live feed, decoded only at the
transport edge.

The [peer protocol](peers.md) is a tested experiment for two-party plan agreement
and worker admission. It is available only to unit tests or with the explicit
`experimental-peer` engine feature; production hosts do not enable it. Runtime
authentication and governed dispatch are not implemented.

The optional [Claude Code adapter](claude-mod.md) presents native work through
connected MCP tools; native services retain every workflow and source authority.

## Applications and resources

`apps/website` contains the documentation website. Fixture descriptions, font
provenance, and licenses remain beside
the files they document.

The [desktop foundation decision](desktop-foundation.md), recorded 12 October 2026,
selects a Tauri Rust host and OS WebView from native performance, editing and
VoiceOver evidence. It names the existing workspace/session/history owner repairs
and the atomic production renderer cutover. Its bounded proof lives in
`benchmarks/desktop-foundation`; the component rows above describe the currently
delivered application.

[Desktop development](desktop.md) is maintained in `crates/magistr-desktop`.
The Tauri host uses the shared admitted application services. Process and native-window checks with controlled providers verify its
composition; the complete real-provider interaction matrix remains separate work.
