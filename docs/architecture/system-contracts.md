# System contracts

This is the maintained cross-cutting view of the delivered architecture: what
the objects are and who owns them, how the crates, processes and installed
files relate, how each production journey travels, and which builds and entry
paths were actually exercised. Mechanism lives in the per-subsystem guides;
this page names the owner, links the guide and states the evidence. When a
guide and this page disagree, the guide's source anchors decide and this page
is corrected.

Two tests keep it honest: every top-level `magistr` command must have a row in
the [entry matrix](#entry-matrix), and every relative link here must resolve
(`crates/magistr-cli/src/main.tests.rs`, the two `system_contract` tests). The
[pre-release policy](../../AGENTS.md#before-public-release) applies: nothing
here is a compatibility promise.

Recorded 19 September 2026 against `main` after `ab5dba7`
(`audit-260912-36-system-contracts`). The
[contract baseline](contracts.md) remains the historical record of what the
refactor started from.

## Glossary

One row per object. *Identity* is the value that names an instance; the full
identity table with types and change rules is the
[identity map](application.md#identity-map).

| Object | What it is | Owner (the only writer) | Identity | Guide |
| --- | --- | --- | --- | --- |
| Project | One native workflow store and its configuration | `ProjectAuthority` admission per `WriterRole` | `project_id` | [project authority](project-authority.md) |
| Writer role | `Host` (cockpit, loops, desktop) or `Service` (the agent's MCP server); the two coexist, a second of either is refused | the OS lock the role's lease holds | `OwnerGeneration` (feed `epoch`) | [roles](project-authority.md#roles) |
| Native writer | `NativeEngines`: every roadmap, ship, think and signal mutation, by calling the library's own tool handler | constructed only by `ProjectAuthority::open`/`serve` | the role's generation | [one write path](project-authority.md#one-write-path) |
| Reader | `NativeReader`: lease-free, cannot mutate | anyone | none | [capabilities](project-authority.md#capabilities) |
| Owner endpoint / attached client | A Unix socket an admitted owner serves; `OwnerClient` acts through the owner instead of opening a second writer | the admitted owner's invocation | protocol version, peer credential, exact build identity, project, role, generation | [attachment](project-authority.md#attachment) |
| Chunk, signal, reasoning step, objective, task | Native workflow records | the four family engines behind the native writer | record ids; `run_id`, `task_id`, `task_instance` | [clients](clients.md#workflow-consistency-model) |
| Run and attempt | One loop execution and one retry of one command in it | `IterationLoop` with the domain `RunState` and operation ledger, in `.magistr/loop-state.json` | run id; `AttemptId` | [durability](durability.md) |
| Operation record | Intent written before an effect and settled after it | the loop checkpoint | command plus attempt number | [records and outcomes](durability.md#records-and-outcomes) |
| Session and turn | One provider conversation and one request in it | the CLI host's `SessionSupervisor` | provider session id (not persisted by Magistr) | [sessions](sessions.md#shared-session-supervision) |
| Permission decision | Allow, Deny or Ask for one request | the shared permission service over project policy | request correlation within a session | [permissions](permissions.md) |
| Workspace | One checkout's source tools, index, journal and workers | the admitted `WorkspaceService`, one per canonical root | `WorkspaceIdentity { root, generation }` | [workspace owner](workspace-owner.md) |
| Source route | A session's only way to write source: routed through the workspace `write` verb, or refused | the host that admitted the workspace | the workspace generation | [effects](effects.md#routing-writes-through-the-workspace-pipeline) |
| Source snapshot | A content-addressed observation of selected bytes | none; lock-free capture | `SourceSnapshot::revision()` | [owned subsystems](owned-subsystems.md#evidence-requires-source-identity) |
| Mutation receipt | What one applied journal intent changed | the workspace journal | `MutationReceipt::revision` | [application](application.md#identity-map) |
| Check receipt | One admitted, run and settled gate on one task | `AdmittedCheck` through the native writer | receipt id; `CheckIdentities` | [receipts](project-authority.md#receipts-and-the-evidence-capability) |
| Workspace worker | Parser, grammar and sandbox child of one owner; no lock, no service | `WorkerSupervisor` in the owner | the parent's lifetime | [native worker boundary](owned-subsystems.md#native-worker-boundary) |
| Projection | `NativeSnapshot` and `NativeEvent`: what a surface shows of the store; never an authority | the one producer, `magistr_engine::native` | `EventCursor { epoch, sequence }`, `SnapshotCursor` | [projections](projections.md) |
| Cockpit model | Shared interface state updated by `Msg` | `magistr-cockpit` `update` | none | [terminal](terminal.md#state-and-rendering) |
| Administrative change plan | The per-path expected state setup, repair and removal compute and then execute | the administrative service | per-change receipt | [configuration](configuration.md#administrative-change-plan) |
| Project inputs | Environment, user, provider and machine inputs captured once per entry | `ProjectInputs` at each composition root | none | [project inputs](configuration.md#project-inputs) |

## Dependency view

Compile-time direction only, normal (non-dev) dependencies. The `domain_boundary`
and `application_boundary` suites, the `workflow-source`, `workspace-source`,
`structure` and `machete` gates enforce it; see
[domain enforcement](domain.md#enforcement-and-verification).

| Crate | First-party crates it depends on |
| --- | --- |
| `magistr-domain`, `magistr-process`, `magistr-theme`, `magistr-mcp`, `magistr-acp-agent` | none |
| `magistr-application` | domain |
| `magistr-workflow-contracts` | domain |
| `magistr-workflow` (first-party think-and-ship source) | workflow-contracts, process |
| `magistr-engine` | domain, workflow, workflow-contracts |
| `magistr-workspace` | process |
| `magistr-core` | domain, engine, process, workspace |
| `magistr-acp` | application, process |
| `magistr-cockpit` | application, domain, theme, workflow-contracts |
| `magistr-cockpit-runtime` | application, cockpit |
| `magistr-tui` | application, cockpit, cockpit-runtime, engine, theme |
| `magistr-cli` (the host library and the `magistr` binary) | acp, acp-agent, application, cockpit, cockpit-runtime, core, engine, mcp, process, workflow, workspace; `magistr-tui` only with the `terminal` feature |
| `magistr-gui2` (own workspace) | cockpit, cockpit-runtime, application, domain, theme, workflow-contracts; the pinned `qs-gpu`, `qs-platform`, `qs-text` |
| `magistr-desktop` (own workspace) | `magistr-cli` with `default-features = false`, gui2, cockpit, workspace |

Regenerate the root rows with `cargo metadata --format-version 1 --no-deps
--locked` (normal dependencies whose name starts with `magistr`); the two
desktop rows come from their own manifests.

The [desktop foundation decision](desktop-foundation.md), added 12 October 2026,
selects a Tauri Rust host and OS WebView using a separate bounded native fixture.
These dependency/runtime rows continue to describe the delivered components.
The target reuses their admitted application, session, workflow and source ports;
the decision identifies the owner repairs required before the renderer cutover.

Rules the view carries: `magistr-domain` has no effectful dependency;
`magistr-gui2` never depends on `magistr-tui`; the desktop launcher reaches the
host library with the `terminal` feature off; `experimental-peer` is never
enabled by a production resolution; there is no N-API, Node or Bun edge in the
production graph. Crate responsibilities are in the [overview](overview.md).

## Runtime view

Processes and the capability each one holds while a session runs. Solid lines
are parent-to-child spawns; dotted lines are socket attachments.

```text
host process: magistr run | next | next-loop | magistr-desktop
  ├─ Host lease ─ NativeEngines ─ OwnerEndpoint (typed commands)  ◄┄┄ second `magistr run`
  ├─ WorkspaceService (one per root) ─ OwnerEndpoint (mcp)        ◄┄┄ every workspace bridge
  │     └─ magistr --workspace-worker            (parser, grammar, Wasmtime sandbox)
  ├─ SessionSupervisor ─ ACP ─► magistr acp-agent --provider …    (provider adapter)
  │                                  └─► provider runtime (claude, codex, chat API, ollama)
  │                                         ├─► magistr mcp-serve            Service lease, own NativeEngines
  │                                         │      ◄┄┄ a second `mcp-serve` proxies its stdio here
  │                                         └─► magistr mcp-launch workbench-mcp   bridge ┄┄► the host's WorkspaceService
  └─ surface: magistr-tui runtime or magistr-gui2 window, fed by Msg and the projection feed

observers (no lease, beside any owner): bare `magistr`, `roadmap show|next|status|export`,
  `guard`, `workspace journal|snapshot|doctor`
```

Two workflow writers may be live at once, one per role, on one store; they are
made coherent by the per-family store stamp, not by a shared cache
([consistency model](clients.md#workflow-consistency-model)). One workspace
owner is live per canonical root. Interrupts reach every owner through the
process-wide shutdown ([interrupts](sessions.md#interrupts-and-shutdown)).

## Deployment view

What is installed and what it writes. There is no daemon, no remote endpoint
and no separately deployed think-and-ship or Workbench product.

| Artifact | Built by | Contains | Platforms |
| --- | --- | --- | --- |
| `magistr` | `scripts/package-release.sh` (host archive plus checksum), verified by the `public release readiness` workflow | every CLI command, both MCP endpoints, the ACP provider adapter and the private worker role in one executable | Linux and macOS in CI; Windows is not built |
| `magistr-desktop` | `cargo build --manifest-path crates/magistr-desktop/Cargo.toml --locked`; no packaging exists | the desktop window plus the same child roles | macOS exercised; see [launch evidence](desktop.md#launch-evidence-and-platforms) |
| Documentation website | `apps/website` (`npm ci && npm test`) | static pages | any static host |

| State | Location | Writer |
| --- | --- | --- |
| Native workflow store | `.think-and-ship/` family files in the project | the admitted native writer of either role |
| Loop checkpoint and loop-owner lock | `.magistr/loop-state.json`, `.magistr/loop-owner.lock` | the loop; `resolve-dispatch` under the lock |
| Project configuration and generated agent files | `.magistr/project.json`, `.claude/`, `.codex/`, `.mcp.json` | the administrative service |
| Workspace index, journal, key, owner lock | `.workbench/` | the admitted `WorkspaceService` |
| Owner sockets | the runtime directory, keyed by the lease | each admitted owner |
| Cost, session and permission records | the user's data and state directories | the CLI host |

Every SQLite file, its journal mode and its writers are in [stores](stores.md).
External requirements are the provider CLIs or API keys the chosen provider
needs ([sessions](sessions.md#configuration-and-defaults)); Git; nothing else
at run time.

## Traces

Each trace names the production path in order and the evidence that executes
it. Test paths are under `crates/magistr-cli` unless another crate is named.

### Setup

`magistr init` → CLI detection with plugin results → core stack detector →
one [administrative change plan](configuration.md#administrative-change-plan)
→ the administrative service revalidates each target at its write and answers
a per-change receipt. `--dry-run` prints the same plan that would be applied;
`init ci`, startup repair and `codex-remove` use the same plan and service.
Interactive setup additionally starts an `AgentSession` whose source writes go
through an admitted workspace or are refused. Evidence: `main.tests.rs`
`full_init_pipeline_detect_to_scaffold`, the scaffold plan companion suites,
`tests/effect_capabilities.rs`.

### Resume

`--resume <id>` or `--continue` → session configuration resolves the provider
and model before any turn → the transport selects ACP Load only for an
explicit id and only when the provider advertises loading → the provider
replays history. A provider that cannot restore refuses; Magistr does not
start over silently (the chat API adapters, Ollama included, refuse loading;
Codex and Claude restore).
In the desktop a failed connection returns the request to the editor with the
reason. Guides: [capability guarantees](sessions.md#capability-guarantees),
[connection recovery](desktop.md#connection-recovery). Evidence: ACP-agent
integrations `agent` and `codex_provider`, `tests/cockpit_session_contract.rs`,
`crates/magistr-desktop/tests/native_launch.py`.

### Permission

Provider request → ACP → `SessionSupervisor` → shared permission service
(policy Deny wins, remembered answers keep their scope) → Allow or Deny is
answered without a surface; Ask reaches the cockpit strip or desktop control,
and a noninteractive host refuses → the supervisor returns the chosen option;
cancellation or teardown answers pending asks Cancelled. Host effects (file
read, terminal) pass the same service before they run. Guide:
[permissions](permissions.md). Evidence: `tests/shared_permissions.rs`,
`tests/cockpit_permissions.rs`, `tests/session_harness.rs`, supervisor tests
`headless_and_cockpit_attachments_share_policy_decisions_and_terminal_outcomes`.

### Native source mutation

Two entrances, one pipeline. An agent's `edit`/`files`/`script`/`undo` tool
call → `magistr mcp-launch workbench-mcp` bridge → attaches to the root's
admitted `WorkspaceService` → verified write pipeline (dry-run diff, parse
verification, format, journal intent, atomic swap) → `MutationReceipt`; a
settled write always answers a receipt, compacted when the response budget is
small. An ACP `fs/write_text_file` → permission decision → the host's
`SourceRoute` → the same `write` verb; a refused route withholds the write
capability at initialize. There is no direct file write in the ACP host
(`native-write-path` rule 12). Guides: [effects](effects.md),
[workspace owner](workspace-owner.md#mcp-sessions-and-writes). Evidence:
`tests/source_pipeline_routing.rs`, `tests/workspace_served.rs`,
`tests/workspace_attachment.rs`.

### Checks

`magistr_gate` or `ship_check` (agent) and the loop driver's gate set (host) →
`AdmittedCheck::admit` names the task and captures a lock-free
`SourceSnapshot` → the command runs once through `OwnedProcess` (a pty for
named gates, pipes for direct workflow checks) in the observed project →
settlement records exit code, identities and a durable receipt on that task,
or `Stale` when the admitted run or task was superseded. Source changes retain
the execution result with changed freshness and refuse completion. Re-running
a gate captures the current source and allocates a new attempt; direct
workflow checks also resolve relative reports in that admitted workspace.
The loop's set is
`WorkspaceEvidence::required_gates` for the project root. `magistr validate`
runs the same commands and records nothing. Guides:
[receipts](project-authority.md#receipts-and-the-evidence-capability),
[check admission](application.md#the-check-admission-and-source-capability-interface).
Evidence: `tests/application_composition.rs`, `tests/mcp_serve_integration.rs`,
loop driver tests `every_gate_the_loop_runs_reaches_the_trace_as_a_verified_check`.

### Completion

`roadmap_complete_chunk` (agent), the cockpit or desktop Complete action, or
the loop's judged completion → the native writer's handler → completion is
[governed by receipts](project-authority.md#completion-is-governed-by-receipts):
every required gate must hold a current receipt for the present source and
gate configuration, and the reference names the task those receipts are on.
A later source, gate-configuration or acceptance edit refuses completion until
the gates re-run. In the loop, red outer gates reopen a self-completed chunk.
Evidence: `tests/application_composition.rs`, `tests/next_iteration_integration.rs`,
loop driver tests `a_judged_completion_names_the_task_its_gates_were_recorded_on`.

### Multi-client attach

A second `magistr run` is refused the Host lease → `OwnerClient::attach_project`
→ handshake (version, credential, exact build, project, role) → every cockpit
verb is carried out by the running host's writer under its generation. A
second `magistr mcp-serve` proxies its stdio to the owner's session. Every
workspace bridge attaches to the root's one owner, or admits when the root is
free, and reconnects lazily when the owner goes away. An unavailable endpoint
fails closed; read-only is the explicit observer path. Guides:
[attachment](project-authority.md#attachment), [the bridge](workspace-owner.md#the-bridge).
Evidence: `tests/mcp_project_owner.rs`, `tests/workspace_attachment.rs`,
`tests/workflow_consistency.rs`, workflow client tests
`a_second_cockpit_attaches_to_the_running_host_and_acts_through_it`.

### UI events

Native writer mutation → the one producer emits a sequenced `NativeEvent` on
the owner's feed → decoded at the transport edge → `FeedCursor` delivery →
cockpit `Msg` → `update` → `Model` → the terminal cell tree or the desktop
pixel layout. A gap, an owner change or a detached surface reseeds from a
whole `NativeSnapshot`; freshness is restored only by a seed or a confirmed
unchanged stamp. Session updates travel supervisor → application
`SessionHandle` → bounded relay → `Msg`. User input returns as commands
through the host, never as renderer-side mutation. Guides:
[projections](projections.md#delivery-and-freshness),
[terminal data flow](terminal.md#data-flow), [desktop ownership](desktop.md#ownership-and-rendering).

### Cancellation

Cancel-turn, terminate-session and detach-viewer are separate. First Ctrl-C
(or Stop) with a turn in flight requests cancellation and the session stays
available; the supervisor keeps the turn active until the adapter reports a
terminal outcome or the stream is lost, and what that outcome proves differs
by adapter (Codex waits for the runtime's terminal event; Claude and the chat
APIs give no remote acknowledgment — see
[capability guarantees](sessions.md#capability-guarantees)). A process SIGINT requests the process-wide shutdown every owner
observes; the second is the emergency exit. A cancelled or disconnected result
never certifies work: the loop records the observed outcome and gates still
decide. Guide: [interrupts and shutdown](sessions.md#interrupts-and-shutdown).
Evidence: `tests/cockpit_session_contract.rs`, ACP-agent integration `agent`,
`cmd_run_headless` tests.

### Recovery

Loop start → `open_or_recover` reads the atomic checkpoint → every in-flight
record becomes `Indeterminate` and is reconciled against its authority (native
store, durable receipt, runtime liveness with start identity, Git baseline) →
what stays open ends the loop `Unreconciled` without launching or latching a
terminal reason → `magistr workspace doctor` explains each record and
`magistr resolve-dispatch` records an operator's decision under the loop-owner
lock. A settled terminal run is archived on the next start. Workspace side: a
killed owner takes its worker with it, bridges reconnect to the next owner,
and the journal recovers unresolved intents. Guide:
[recovery and reconciliation](durability.md#recovery-and-reconciliation).
Evidence: core `iteration_operations.tests` (real child process),
`tests/workspace_attachment.rs`, `tests/workspace_cli_verbs.rs`,
`tests/mcp_project_owner.rs::simultaneous_stdio_start_has_one_owner_and_kill_allows_fresh_restart`.

## Entry matrix

Every top-level command of the shipped binary, the desktop executable, and the
capability it holds. *Workflow* and *workspace* use the glossary's words;
"none" means the command touches neither store. Admission paths and per-row
tests for the first group are in the [client inventory](clients.md#client-inventory).

| Entry | Workflow | Workspace | Session |
| --- | --- | --- | --- |
| `magistr` (bare), `magistr roadmap` show, next, status, export | reader | none | none |
| `magistr roadmap generate` | none here; the agent's `mcp-serve` writes | admitted for the run | `AgentSession` |
| `magistr run` (cockpit) | Host writer, or attached client when held | admitted, or refused when held elsewhere | `CockpitSession` through `InteractiveHost` |
| `magistr run` plain and `--headless` | none here; the agent's `mcp-serve` writes | admitted for the run | `AgentSession` |
| `magistr next` | reader for the summary; cockpit form holds the Host writer | admitted for the run | supervisor per attempt |
| `magistr next-loop` | Host writer; the loop-owner lock | admitted per loop | supervisor per attempt; judge with no tools |
| `magistr resolve-dispatch` | none; rewrites the loop checkpoint under the loop-owner lock | none | none |
| `magistr mcp-serve` | Service writer, or stdio proxy to the owner when held | lock-free evidence only | none |
| `magistr mcp-launch` `magistr` or `workbench-mcp` | as `mcp-serve`, or none | the owner when free, else an attached bridge | none |
| `magistr mcp-proxy` (hidden) | none; filters a foreign MCP server's stdout | none | none |
| `magistr workspace` serve | none | the owner when free, else an attached bridge | none |
| `magistr workspace` journal, snapshot, doctor | none (doctor reads the checkpoint as it lies) | lock-free readers; doctor adds one bounded probe | none |
| `magistr workspace` init, key, migrate | none | file administration; no service opened | none |
| `magistr acp-agent` (hidden) | none; its tools arrive through `mcp-serve` and the bridge | none | the provider side of one session |
| `magistr codex-launch`, `magistr codex-remove` | none in the wrapper | none | the external runtime; removal is an administrative plan |
| `magistr guard` | reader | none | none |
| `magistr validate` | none; runs gates and records nothing | none | none |
| `magistr init`, `magistr reset`, `magistr migrate`, `magistr import`, `magistr export`, `magistr preset`, `magistr plugins` | none | none | interactive `init` starts an `AgentSession` with an admitted workspace |
| `magistr detect`, `magistr lint`, `magistr doctor`, `magistr config`, `magistr archetypes`, `magistr policy`, `magistr team` | none | none | none |
| `magistr models` | none | none | a zero-turn provider probe |
| `magistr learn`, `magistr memory`, `magistr plans`, `magistr sessions`, `magistr costs`, `magistr permissions` | none; machine-local and project-local records | none | none |
| `magistr login`, `magistr logout` | none; identity cache | none | none |
| `magistr completions`, `magistr man-pages` | none | none | none |
| `magistr --workspace-worker` (private role, not a command) | none | none; child of one owner | none |
| `magistr-desktop` | as `magistr run` (cockpit): the same `InteractiveHost` | as `magistr run` (cockpit) | Conversation and Roadmap sessions through the shared supervisor |

Not entries: the `magistr-tui` cockpit example and the `magistr-gui2` fixture
viewer show sample data and hold no capability; the engine `peer` module is an
experiment behind `experimental-peer` ([peers](peers.md)).

## Build, feature and platform matrix

| Lane | Command | Exercised 19 September 2026 on macOS 26.5.2, `aarch64-apple-darwin`, Rust 1.95.0 |
| --- | --- | --- |
| Root workspace, default features | `cargo check --workspace --all-targets`, `cargo nextest run --workspace`, Clippy with `-D warnings` | required gates `check`, `test`, `lint` |
| `magistr-cli` host library without `terminal` | `cargo check -p magistr-cli --lib --no-default-features --locked` | matrix run below |
| `magistr-engine` with `experimental-peer` | `cargo nextest run -p magistr-engine --features experimental-peer --locked` | matrix run below; never part of a production resolution |
| `magistr-workspace` `--no-default-features`, `--features sandbox`, `--features lang-pack` | `./scripts/workspace-native-gate.sh` | required gate `workspace-native` |
| Desktop renderer | `cargo check --manifest-path crates/magistr-gui2/Cargo.toml --locked --all-targets` | matrix run below |
| Desktop launcher | `cargo test --manifest-path crates/magistr-desktop/Cargo.toml --locked` | matrix run below |
| Release binary | `./scripts/workspace-release-smoke.sh`, `./scripts/release-latency-gate.sh` | required gates |
| Host archive | `./scripts/package-release.sh` and checksum verification | matrix run below |
| Visual contracts | `./scripts/screenshot-gate.sh`, `./scripts/legacy-gui-ratchet.sh` | required gates |
| Website | `npm ci && npm test` in `apps/website` | CI job `quality`; not a project gate |

### Unavailable environments

| Environment | Status | Consequence |
| --- | --- | --- |
| Linux | not available on this machine; built, tested and packaged by the `public release readiness` workflow on `ubuntu-latest` | local results below say nothing about Linux; CI is the evidence |
| Windows | no runner, no build, no package | process-tree settlement, key-file protection and the parent-death guard are unverified there (`audit-260912-42-windows-process-tree`, blocked on an external runner) |
| `wasm32-wasip1` fallback lane of the workspace engine | not built | the `--no-default-features` lane is the nearest exercised build |
| Live providers (Claude, Codex, chat APIs with credentials) | opt-in only; not part of this verification | controlled in-process and child-process providers carry the session contracts; live runs are recorded where they happened ([live Codex acceptance](desktop.md#live-codex-acceptance)) |
| Desktop native driver (`native_launch.py`) | needs a graphical session and Accessibility permission | run only when stated in the record below |
| Optional `codex-subagent` gate | needs a Codex login and quota | not required; last recorded pass 12 September 2026 |

### Wasmtime lines and guest assets

| Line | Version in `Cargo.lock` | Reached through | Feature | Guests it runs |
| --- | --- | --- | --- | --- |
| Extension host | `wasmtime` and `wasmtime-wasi` 48.0.2 | `magistr-workspace` `sandbox` | `sandbox` | plugin `guest.wasm` components and the QuickJS script guest |
| Dynamic grammars | `wasmtime` 36.0.15 | `tree-sitter` 0.26 `wasm` feature (`wasmtime-c-api-impl`) | `lang-pack` | language-pack grammar modules such as `gleam.wasm` |

Both majors compile as distinct crates; `cargo tree -i wasmtime@36.0.15` and
`cargo tree -i wasmtime@48.0.2` regenerate the table. The tracked guest assets
are all under `subsystems/workspace/reference/`: four plugin guests
(`lang-pack-gleam`, `lsp-resolver`, `ranking-visibility`, `runner-go`), the
Gleam grammar and `vendor/qjs/qjs-wasi.wasm`. Both lines run only inside
`magistr --workspace-worker`. The full dependency table is the
[workspace support matrix](../../subsystems/workspace/SUPPORT_MATRIX.md).

### Native and reference provenance

| Source | Status | Record | Gate |
| --- | --- | --- | --- |
| Workflow (think-and-ship fork) | first-party, in the root workspace as `magistr-workflow` | [workflow provenance](../../subsystems/workflow/PROVENANCE.md) | `workflow-source` |
| Workspace engine (Workbench fork) | first-party, `magistr-workspace` | [workspace provenance](../../subsystems/workspace/PROVENANCE.md), [feature inventory](../../subsystems/workspace/FEATURE_INVENTORY.md) | `workspace-source`, `workspace-native` |
| Workbench reference tree (Bun, NAPI) | reference only; never in the production graph | `subsystems/workspace/reference`, `UPSTREAM_MANIFEST.json`, `LOCAL_CHANGES.json` | `workspace-reference` |

## Verification record

Filled from the recorded checks of `audit-260912-36-system-contracts`; the
native ship record is the evidence and this table is its summary.

| Check | Command | Result on 19 September 2026 |
| --- | --- | --- |
| Host library without the terminal | `cargo check -p magistr-cli --lib --no-default-features --locked` | passed |
| Peer experiment lane | `cargo nextest run -p magistr-engine --features experimental-peer --locked` | 544 passed |
| Desktop renderer | `cargo check --manifest-path crates/magistr-gui2/Cargo.toml --locked --all-targets` | passed |
| Desktop launcher | `cargo test --manifest-path crates/magistr-desktop/Cargo.toml --locked` | 8 passed; what they cover is described in [launch evidence](desktop.md#launch-evidence-and-platforms) |
| Combined-process recovery | `cargo nextest run --locked -p magistr-cli -p magistr-core --no-tests fail -E 'binary(mcp_project_owner) \| binary(workspace_attachment) \| binary(workspace_cli_verbs) \| binary(application_composition) \| binary(source_pipeline_routing) \| binary(workflow_consistency) \| test(iteration_operations)'` | 54 passed: real `magistr` processes, a real Host writer with a real `WorkspaceService` and worker binary, killed owners and workers, real provider child processes in crash injection |
| Host archive | `./scripts/package-release.sh`, then `shasum -a 256 -c` on the archive | `magistr-0.1.0-aarch64-apple-darwin.tar.gz` built and verified |
| Required gates | `magistr_gate(all)`: `check`, `test`, `lint` and the thirteen required custom gates of `.magistr/project.json` | recorded on the verification task; this page is committed only with all of them passing |

Not run in this verification, by name: Linux and Windows (above), the
`wasm32-wasip1` lane, every live provider, the desktop native Accessibility
drivers (`native_launch.py`, `native_live.py`), the website build, and the
optional `codex-subagent` gate. The release and visual contracts are the
`workspace-release-smoke`, `release-latency`, `screenshots` and `legacy-gui`
gates; the visual gates compare rendered pixels and cell output against
intended goldens, which is an appearance check and not proof of behavior.
The behavioral invariants are carried by the suites the traces name, which run
real processes and real stores rather than fake ports, except where a row
above says a controlled provider stands in for a live one.

### Discoveries corrected by this review

- The contract baseline still said no `gui2` crate existed on `main` and no
  desktop writer needed routing; the desktop launcher composes the same
  `InteractiveHost` as cockpit `run`. Corrected there and in the
  [client inventory](clients.md#client-inventory), which had no desktop row.
- The workspace support matrix listed `rusqlite` 0.33.0; the lockfile holds
  0.40.2 since `audit-260912-22-sqlite-version`. Corrected.
- A hand-drawn dependency diagram drafted for this page was wrong on four
  edges (`magistr-core` depends on the engine, process and workspace crates;
  `magistr-acp` on application and process; the CLI on the workflow source
  directly). The [dependency view](#dependency-view) is therefore the measured
  `cargo metadata` table, not a drawing.

## Change walkthrough

The acceptance for this page asks for a human walkthrough of one provider,
one policy and one cross-surface action change. **No human has performed it.**
The question was put to the project owner during this review and was not
answered, so nothing below is a human result, a timing or a comprehension
score. What follows is the script a person should follow, and the reading set
an agent measured for each change so their result has something to be compared
with. `audit-260912-36-system-contracts` stays open in the roadmap, blocked on
a person, until that result is recorded here.

Measurement method: production files (companion `*.tests.rs` and test support
excluded) under `crates/*/src` whose text matches the named symbols, by
`grep -rlE`. It is a name match, not name resolution; false positives are
called out rather than removed from the count.

| Change | Start here | Symbols matched | Files matched | What the match shows |
| --- | --- | --- | --- | --- |
| Provider: add an adapter capability and show it to a surface | [capability guarantees](sessions.md#capability-guarantees), then `crates/magistr-acp-agent/src/provider_capabilities.rs` | `ProviderCapabilities`, `"capabilities"` | 13 | 9 are the change: the definition, the provider trait, four adapters, the Codex handshake, `serve.rs` and the crate root. 4 are unrelated uses of the word (workspace extension manifests, the MCP client, plugin lifecycle). No cockpit or desktop file matches: capability data still stops at the wire, as the baseline's change-locality note observed |
| Policy: add a permission outcome | [permissions](permissions.md#owners-and-precedence), then `crates/magistr-domain/src/policy/permission.rs` | `PermissionOutcome` | 15 | two different types share the name: the domain policy outcome (domain, engine re-export, CLI session policy) and the application's reply outcome (application, ACP handlers, three provider adapters, the TUI harness agent). A reader has to tell them apart before counting; no surface renderer matches, since Ask is presented from the request, not the outcome |
| Cross-surface action: add a cockpit verb that also works attached and in the desktop | [owner commands](clients.md#owner-commands), then `crates/magistr-cockpit/src/engine/port.rs` | `EngineCmd::`, `OwnerCommand::` | 9 | cockpit command construction and `update`, `apply` and `describe`, the attached port, and the engine's owner dispatch. The in-process port, the `EnginePort` trait and `NativeEngines::apply` do not match these two patterns and are also part of the change; neither renderer matches, because both send the shared cockpit command |

For each change the person records: start and end time, every file opened in
order, the first point at which they were unsure where to go next, and whether
the companion tests they expected to extend existed. Report the raw notes;
do not convert them to a score.
