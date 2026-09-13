# Architecture contract baseline

This is the behavior baseline for the architecture refactor, recorded on
9 September 2026. It describes main at
`3723c626bceb71679019029fb9f13feae92a583b` and desktop development at
`gui2@f0d6370ac32877160e6d40852b8051736e4ca6ec`.
The [architecture report](https://moistfridge.com/magistr-architecture-reconsidered)
motivates consolidating ownership. The observations below come from these source
snapshots and executable scenarios. **The pinned observations are historical;
implemented changes are identified separately below.**

The subsequent [domain extraction and run consolidation](domain.md) establish
shared pure values and implement the run-policy decisions in `arch-03-run-semantics`.
The [implementation record](#implemented-run-consolidation) identifies the
changed behavior without replacing the original observations. Scoped identities
remain available to application adapters; existing session wire IDs stay compatible.

## Objects and owners

| Object | Authority or owner at the pinned snapshot | Intended boundary |
| --- | --- | --- |
| Project | `NativeConfig` and native identity select the project/store; `MagistrService` owns cooperating MCP admission. Configuration resolution also lives in CLI/core. | One admitted project application handle supplies mutations to every supported host. Workbench retains its separate service ownership. |
| Chunk, signal, reasoning, execution objective/task | The four native think-and-ship family engines behind `NativeEngines`; scope strings and cockpit selections do not create isolated execution cycles. | Keep native workflow canonical behind narrow commands and queries. |
| Run/attempt | CLI/core `IterationLoop` owns production iteration limits, Git progress and recovery; `LoopDriver` coordinates cockpit execution. Engine `WorkflowEngine` separately models phase, stop counters and checkpoints. | One deterministic run policy; runtime coordinates effects and persists checkpoints. |
| Session/turn | Headless/plain `AgentSession` and cockpit `agent::Session` are distinct lifecycle owners over the same `connect_transport` seam. Providers own opaque conversation IDs. | A shared supervisor owns live tasks, provider attempts and terminal outcomes; the interface observes and sends commands. |
| Decision | `CockpitPolicy` evaluates protected paths, profile, charter and governance; cockpit Session correlates requests and answers. Headless has a separate option-selection worker. | Shared policy decides Allow/Deny/Ask; hosts resolve Ask according to an explicit interaction mode. |
| Verification | MCP gate service and CLI `LoopDriver` execute checks; native Ship records them. Native roadmap completion and phase admission are separate operations. | Completion validates applicable required evidence and references the actual verification task. |
| Projection | `NativeSnapshot` and `NativeEvent` from `magistr_engine::native` (the one producer), agent translation and cockpit `Model` describe stored/live facts; `NativeFrame` is decoded at the transport edge only. They are not another workflow authority. | Typed, freshness-aware views; schema version, owner epoch, aggregate revision, event sequence and snapshot cursor have distinct meanings — see [projections](projections.md). |
| Presentation | TUI owns terminal protocols; cockpit currently includes shared state, cell view/layout and effect adapters. gui2 owns native pixel composition and geometry. | Share semantic state/actions; keep each surface's layout, input and renderer local. |

Current sources: [native facade](../../crates/magistr-engine/src/native/native_engines.rs),
[service admission](../../crates/magistr-cli/src/mcp_tools/service.rs),
[iteration loop](../../crates/magistr-core/src/workflow/iteration.rs),
[engine workflow](../../crates/magistr-engine/src/workflow/workflow_engine.rs),
[session configuration](sessions.md), and [terminal contract](terminal.md).

## Native writer inventory

The inventory follows production callers of native constructors, mutation
wrappers and command consumers. Public APIs without a production caller are
listed separately so capability is not mistaken for integration.

| Entry | Path and mutations | Admission/limitations |
| --- | --- | --- |
| MCP, including tools called by external coding agents | `MagistrService::{for_project,new}` → `ProjectAuthority::admit(Service)` → `publish` → `serve` → embedded `UnifiedService` **and** the `NativeEngines` over the same family services; native roadmap/ship/think/signal verbs on the wire, `magistr_*` gate recording (`ship_record_gate` on the named active task after a pty run) and phase actions (`ship_record_action`) through the writer. `run_stdio_server` serves the owner endpoint (`serve_owner_endpoint`) beside the stdio session. | Service lease acquired before opening engines; clones share it. Native verbs dispatch directly, not through the phase guard. The transport never calls its own wire. |
| Attached `magistr mcp-serve` | Refused the Service lease (`WouldBlock`) → `OwnerClient::attach_mcp` → `proxy_to_owner` copies stdio to and from the owner's rmcp session on the socket. | No lease, no engine: every tool runs in the owner's process. Ends with an error when the owner hangs up; fails closed, naming both refusals, when no endpoint answers. |
| Interactive cockpit `run` | `cmd_run_cockpit` → `EngineClient::admit_or_attach` (`ProjectAuthority::admit(Host)`, or `OwnerClient::attach_project` when the host is held) → `EngineClient::port` (`NativeEnginePort` or `AttachedEnginePort`) → `engine/apply`; the host serves its endpoint (`serve_owner`) for the run. Pin/checkpoint reasoning; start/complete chunks; resolve proposals; clear blockers; promote/ignore/snooze signals — each the library's tool handler, so a promote creates and cross-links the backlog chunk. | Admitted host writer beside the agent's service, or an attached client acting through the running host's writer; fails closed (`EngineError::Occupied`) when the host is held and no endpoint answers. A human's completion passes no ship reference. |
| Cockpit `next` and `next-loop` | `admit_native_host` → `IterationLoop` → `run_cockpit_loop` serves the owner endpoint (`serve_owner_endpoint`) and attaches `EngineClient` to the loop's writer → `LoopDriver`; installs the native port for each session, records gates, starts/completes judged chunks, sets human blockers, reopens self-completed chunks after failed required checks. | One admitted host writer per process; a second loop on a held host is refused (execution cycles are serialized), while a `magistr run` beside it attaches. Gates are recorded on the task named active before the set runs, and a judged completion carries `task:<that task>`. The outer set is the completion contracts themselves — `WorkspaceEvidence::required_gates` for the project root: per-archetype specs in their directories, shared commands once, required custom gates — run under the contracts' names and commands; a set that cannot be resolved fails the iteration. |
| Headless/plain workflow commands | `admit_native_host` and core loop handles supply roadmap reads/prompts, and serve the owner endpoint for the loop; their agent's MCP service performs native workflow writes. | Host lease held for the loop. No outer `LoopDriver` judge/reopen/contract-gate guarantee. Loop JSON and cost/session records are separate persistence. |
| Bare/roadmap cockpit, launcher, `roadmap show|next|status|export`, guard | `EngineClient::observe` / `open_native_reader` subscribe, reseed and read a `NativeReader`. | Observers: no lease, no writer, so they run beside any admitted host. Rendered controls alone do not establish a working writer. |
| Library escape hatches | None. `NativeEngines` is constructed only by `ProjectAuthority::open`; `with_*` callbacks and public constructors were removed by `arch-10-project-authority`, `open_engines` is crate-private, and the `native-write-path` gate pins that engines and services are composed only in `magistr-engine/src/native`, that only the writer and the reader take an engine lock, and that the transport never calls its own wire. | The historical row named public `NativeEngines::{open,open_for_project,with_*}` with no lease; readers are the only unadmitted opening and cannot mutate. Desktop: no `gui2` crate exists on main, so there is no desktop writer to route. |

Writer anchors: [native port](../../crates/magistr-cockpit/src/engine/native_port.rs),
[apply](../../crates/magistr-cockpit/src/engine/apply.rs),
[run wiring](../../crates/magistr-cli/src/orchestration/commands/run/cmd_run_cockpit.rs),
[loop driver](../../crates/magistr-cli/src/orchestration/loop_driver/driver.rs),
[bare cockpit](../../crates/magistr-cli/src/orchestration/commands/cockpit.rs).

Setup/migration/reset commands can modify project files or remove local state;
roadmap export writes a projection. They are filesystem administration, not an
additional native workflow authority. They must not run concurrently with an
admitted writer during consolidation. Roadmap exports remain disabled for this
repository. Standalone think-and-ship, older binaries and arbitrary filesystem
writers do not participate in the [cooperative lease](project-authority.md);
the Service and Host roles exclude only a second writer of their own role.

Native persistence merges additions on disk but does not refresh another
handle's cached reads. The existing two-writer and reload scenarios demonstrate
this limit. They do not prove serializable application commands or atomic
cross-family writes. Detached cockpit reseeding reloads roadmap/ship/signal on
a changed store stamp, and a snapshot the feed asked for over an unchanged
stamp is confirmed rather than re-read; socket attachment or a recent timestamp
is not proof of a coherent global revision — the projection's freshness is
restored only by a seed or by that confirmation.

## Session entry inventory and behavior

| Entry | Lifecycle and permission route |
| --- | --- |
| `run --headless`, headless `next` / `next-loop` | `AgentSession::start` → session supervisor → shared transport; supervisor owns permission handling and callers await termination before exit/next attempt. |
| Plain `run` / workflow drains | `AgentSession::start` uses the same supervisor and resolved permission policy; drains reject missing terminal evidence. |
| Cockpit `run`, TUI init, home resume | `CockpitSession::start` → shared supervisor → application `SessionHandle` → cockpit relay; the supervisor owns pending Ask replies. Home resume supplies an explicit session ID and no opening prompt. |
| Cockpit `next` / `next-loop` | `Connector::live` → shared supervisor and application handle for each attempt; ordinary turns and delegated goals enter explicit owner operations. The outer driver owns workflow settlement. |
| Interactive setup / roadmap generation | `cmd_init_interactive` and `cmd_roadmap` start AgentSession for configuration/planning prompts; these are distinct from roadmap implementation. |
| Workflow judge | `SessionJudge::ask` resolves setup through its `Connector` (explicit model override or the shared session/provider default, governance applied) and hands the connected transport to the shared `SessionSupervisor` under `NoTools` (every permission ask answered with the reject option, noninteractive `Refuse`); it owns no event or permission loop. The turn's text is read as a verdict only when `JudgeEvidence` admits it: `terminate_session` settled and the owner's last outcome is a confirmed `EndTurn` with no error. Cancelled, a limit or refusal, a failed or ended stream, a missing terminal response and a cleanup failure all answer *not yet* with the reason. |
| Model discovery | `cmd_models` → `probe_all_providers` → `probe_runtime` uses a direct zero-turn ACP New/close probe, outside AgentConfig/shared transport setup. |
| Provider protocol host | `acp-agent` serves the provider adapter selected by transport startup; it is the other side of a session, not a second native workflow owner. |
| Direct Codex launcher | CLI `codex-launch` invokes the external runtime wrapper; it is separate from the ACP Codex provider and its session lifecycle. |

Public `AgentPool` worker methods also start AgentSession and its shared session
supervisor, but callers are confined to the multi supervisor/pattern subsystem;
no production CLI/app construction of that subsystem was found. The public
`LocalEngineClient::new` facade has only test callers. Neither is an additional
integrated session path.

[AgentSession](../../crates/magistr-cli/src/agent/session.rs),
[transport setup](../../crates/magistr-cli/src/agent/connect_transport.rs),
[plain drain](../../crates/magistr-cli/src/orchestration/commands/run/mod.rs),
[cockpit Session](../../crates/magistr-cockpit/src/agent/session.rs), and
[direct Codex launcher](../reference/codex.md) define these paths.

| Concern | Historical differences pinned by the baseline |
| --- | --- |
| Permission | Headless chooses AllowOnce if present, otherwise the first option (even RejectOnce), otherwise Cancelled. It does not evaluate project policy. Cockpit policy denial precedes remembered Always answers; undecided requests reach the strip. |
| Defaults and explicit model | AgentSession applies user defaults and backend model-selection failures prevent inference. Direct cockpit/connector setup does not copy provider model defaults; cockpit model selection drops an error and can continue with the runtime default. |
| Provider resume | Transport selects Load only for an explicit `resume` ID. `continue_conversation` is stored but has no transport-selection consumer. Codex replays real history; the API provider starts with empty history on resume while the ACP server advertises loading. |
| Cancel turn | Headless backend ends its message reader on cancellation; cockpit Session remains available for subsequent commands. With a turn in flight, first Ctrl-C cancels and second quits in the TUI. Codex waits for the matching terminal acknowledgment before reuse. |
| Cancellation as loop outcome | Headless Cancelled maps to exit 0 and is not among the headless loop's error/budget break cases. Cockpit driver treats any `TurnEnded(_)` as ended, then runs gates/settlement. EOF without a result is headless Ok. These are not proof of successful work. |
| No progress | Core production loop stops at 2 consecutive iterations without a commit; engine workflow stops at 3. Core resets after observing a changed Git HEAD; engine resets when `record_committed()` is called. |
| Recovery | Core loop persistence omits its separate no-commit counter and recovery resets it to zero. Engine checkpoint restoration retains the count. Cockpit next-loop recovers an existing loop; headless/plain loops start fresh. The checkpoint is written with `fs::write` and records nothing about a provider launch, a gate receipt or a native mutation in flight. |
| Completion | Native roadmap completion permits completion without a check or ship reference. Phase done needs a recorded check count, including failures. Native finalize can return shipped with a failed-required-check warning. The skill's complete gate/acceptance protocol is stronger. |

The no-progress and recovery differences above are resolved by the implemented
run consolidation below. The cancellation row remains provenance for the old
entry paths; headless and cockpit loop adapters now record their observed terminal
outcomes through the shared run state. `arch-08-session-supervisor` then moves
AgentSession paths to one supervisor and removes plain-drain success on EOF.
Cancellation request, terminal response, detachment, and residency are separate
facts; see [session supervision](sessions.md#headless-session-supervision).
Cockpit supervision remains separate until arch-09. The permission differences
are resolved by the implemented permission consolidation below.

The API loading limitation is source evidence in
[API provider](../../crates/magistr-acp-agent/src/api/provider.rs) and
[ACP serve](../../crates/magistr-acp-agent/src/serve.rs), not a claim that a new
fresh-process API restoration experiment passed. Model/default, plain-drain and
loop-cancellation comparisons are source traces; the executable suites below
cover their listed boundaries rather than every live provider combination.
No integrated plain-command permission/signal scenario was run: the shared ACP
timeout and TUI harness do not prove that complete path. At the pinned snapshot
plain CLI SIGINT used `utils/signal_handler.rs` to exit with 130 rather than
requesting session cancellation; since `audit-260912-11-bounded-session` the
first SIGINT requests the process-wide shutdown every owner observes and the
second is the emergency exit — see
[interrupts and shutdown](sessions.md#interrupts-and-shutdown).

## Integrated and experimental surfaces

Main integrates terminal and headless session paths. Peer/fleet authority types
and tests are implemented domain capabilities, but production authenticated
dispatch and durable remote ownership are not wired; see [peers](peers.md).

At the pinned gui2 snapshot, the [desktop guide][gui-guide] and
[window startup][gui-window] select busy/mock/catalogue fixtures. The reusable
[Native interface][gui-native] exposes an inbox and a one-time command receiver,
but the executable never takes that receiver. Its [route][gui-route] forwards
nonlocal commands and ignores a failed send. There are no production CLI gui2
references or desktop transport/session launch calls in that snapshot.

Desktop fixtures construct the same Model/`NativeSnapshot`/`NativeEvent` shapes
as live clients, through the cockpit harness fixtures. Those values are sample data, not an additional project/run/session
authority. The shared cockpit crate and CLI cockpit run wiring are identical
between the audited snapshots. Native pixel composition is separate from the
terminal cell tree. This baseline does not build gui2, require its private
framework, change legacy GUI code, or claim live desktop verification.

## Decisions for consolidation

These choices resolve the conflicting historical semantics before code moves.
They are the default contract for subsequent chunks; changing one requires an
explicit decision update and acceptance tests.

1. **Run modes:** distinguish conversation, one-chunk execution, and bounded
   chunk-loop execution. Conversation has no Git no-progress rule. One-chunk
   execution ends after its selected attempt. A chunk loop shares one policy
   across terminal/headless surfaces; UI presentation and permission interaction
   are separate dimensions, not hidden run modes.
2. **No progress and recovery:** adopt the production threshold of two
   consecutive no-commit iterations for chunk loops. Reset on observed commit,
   not a provider restart, recovery or viewer change. Persist the count, budget
   observations and iteration limit with the run. Old checkpoints lacking the
   count require explicit recovery acknowledgment or a new run; do not silently
   manufacture zero. Implemented in `arch-03-run-semantics`; the engine's
   three-count behavior remains a historical observation, not an active policy.
3. **Permissions and compatibility:** use one project policy for every surface.
   Policy Deny always wins. Interactive Ask is presented; noninteractive Ask
   cancels/refuses unless an explicitly selected compatibility policy permits it.
   Preserve the current headless behavior during migration behind a clearly
   named legacy automatic-approval setting, with documented selection and tests;
   do not silently remove it or make it the new shared default. Legacy first-option
   selection is characterized, not a policy rule to generalize. Configuration
   spelling and rollout belong to the shared permission implementation.
4. **Execution cycles:** allow only one native objective/execution cycle per
   project/store until the native library provides true run isolation. Independent
   provider conversations may coexist but cannot replace an active objective.
   A scope string or second session ID is not run isolation. All supported native
   writers must acquire the same application capability; keep effects outside
   the short mutation critical section.
5. **Cancellation and resume:** distinguish cancel-turn, terminate-session and
   detach-viewer. A disconnect or cancelled result never certifies completed
   work. Await terminal provider outcome or record uncertainty before reuse.
   Resume means restored provider conversation with proven capability; an API
   with no durable history must refuse that promise and offer a new conversation.
   Resolve explicit model/default settings before starting any turn.
6. **Completion:** successful commit, recorded check, verified process result,
   passing required checks, and accepted chunk are different facts. Require all
   configured standard and required custom gates plus acceptance-specific evidence
   and the verification task reference before completion/finalization. Treat native
   warnings as evidence to inspect. Keep native storage canonical; no database
   replacement or claim of atomic cross-family receipts is made here.

Current primary constraints checked on 9 September 2026:
[ACP session setup](https://agentclientprotocol.com/protocol/v1/session-setup)
requires declared loading support and history replay;
[Tokio shutdown](https://tokio.rs/tokio/topics/shutdown) separates notifying
tasks from waiting for their termination. These support decisions 4–5;
they do not choose Magistr's no-progress threshold or compatibility policy.
[SQLite isolation](https://sqlite.org/isolation.html) describes serialized
database writes, not coherence of Magistr's current cached native engines.

## Implemented run consolidation

`arch-03-run-semantics` makes
[`RunState`](../../crates/magistr-domain/src/workflow/run_state.rs) the shared
owner of mode, attempt cursor, iteration ceiling, no-progress count, cumulative
budget and limit, settlement marker and terminal reason. The
[`mode`](../../crates/magistr-domain/src/workflow/run_mode.rs) and
[`stop reason`](../../crates/magistr-domain/src/workflow/stop_reason.rs) enums
provide explicit conversation, one-chunk and chunk-loop semantics. Chunk loops
stop after two consecutive no-commit attempts; repeating settlement for an
already observed attempt cannot increment the counter. A later observed Git
commit clears it. Conversation skips the Git guard, and one-chunk mode admits
only its selected attempt.

The [core loop](../../crates/magistr-core/src/workflow/iteration.rs) and
[engine workflow](../../crates/magistr-engine/src/workflow/workflow_engine.rs)
delegate these decisions to the domain. Core retains the legacy loop-state
iteration and maximum fields only as checked projections. Its status field uses
canonical `IterationStatus` while preserving the old `success` wire spelling
for `Committed`. Core and engine also share
[`WorkflowPhase`](../../crates/magistr-domain/src/workflow/workflow_phase.rs)
and [`ProjectContext`](../../crates/magistr-domain/src/project.rs). Phase labels
are explicit; notification punctuation belongs to the
[presentation adapter](../../crates/magistr-core/src/workflow_state.rs).
The core phase index and context/spec counts retain `usize` API compatibility;
the domain iteration projection retains its existing `u32` fields.

[Headless](../../crates/magistr-cli/src/orchestration/commands/next/cmd_next_loop_headless.rs),
[plain](../../crates/magistr-cli/src/orchestration/commands/next/cmd_next_loop.rs)
and [cockpit](../../crates/magistr-cli/src/orchestration/commands/next/cmd_next_loop_cockpit.rs)
next-loop entry points now share `open_or_recover`. Their loop checkpoints retain
the run's stopping evidence and terminal state. Recovery validates projections
and keeps the attempt's original Git baseline. It refuses legacy files missing
the no-progress count/run evidence instead of inventing zero. Starting a new run
requires deliberately preserving elsewhere or removing
`.magistr/loop-state.json`; the adapter leaves an invalid checkpoint intact.

Engine [session snapshot format version 2](../../crates/magistr-engine/src/session/session_snapshot.rs)
requires `RunState` and the complete `CostGovernor`. Restoration checks iteration,
no-progress and budget projections for agreement before
[`EngineHandle`](../../crates/magistr-engine/src/handle/engine_handle.rs) installs
the recovered state. Budget policy, accumulated spend, warning state and
attribution survive a process restart. Version 1 and missing required stopping
evidence are refused. [Peer journals](peers.md) still own reservations, requests,
unsettled worker attempts and receipts; these are separate from session recovery.

The provider/session lifecycle remains separate work. This consolidation does
not redesign atomic checkpoint writes or durable operations (`arch-08` and
`arch-13`), prove live-provider recovery, or integrate the desktop fixture paths.
The [domain guide](domain.md#recovery-boundaries) records the adapter boundaries.

## Implemented permission consolidation

`arch-05-shared-permissions` replaces the historical blanket headless worker
with the [shared permission service](permissions.md). Cockpit and AgentSession
construct the same project policy; next/loop and pool use explicit workflow
context. Noninteractive Ask defaults to refusal, and the named
`legacy-automatic-approval` compatibility setting can resolve only an Ask.
Policy Deny wins. The historical arbitrary first-option fallback is retired.

Remembered answers now belong solely to policy and retain their scope. Workflow
shortcuts cannot bypass remembered refusals or written constraints. Pending
cockpit decisions receive Cancelled on turn cancellation or session teardown;
invalid and stale responses cannot grant permission. The guide identifies policy
metadata, compatibility selection, executable tests and provider-effect limits.

## Implemented project authority

`arch-10-project-authority` moves writer admission out of the MCP transport
into the engine's [`ProjectAuthority`](project-authority.md). The MCP service
admits the `Service` role, publishes the feed and serves the family services
composed inside the engine crate; the cockpit, the loops and the headless
commands admit the `Host` role; the launcher, the bare cockpit, the guard and
the one-shot roadmap reads open a lease-free `NativeReader`. The two roles
coexist on one store and each refuses a second writer of its own role. The
cockpit loop attaches to the loop driver's writer instead of opening a second
cached one. `NativeEngines` is the write capability, constructible only from an
admitted authority, derefing to the reader; its `with_*` callbacks and public
constructors are gone. Admission remains cooperative: no exclusion of older
and direct writers is claimed.

## Implemented local owner clients

`arch-11-local-owner-client` adds the [attachment path](project-authority.md#attachment):
an admitted writer serves an `OwnerEndpoint` on a Unix socket derived from the
lease's key, and a surface whose role is held attaches through the synchronous
`OwnerClient` instead of opening a second cached writer. The handshake checks
the peer credential, protocol version, project and role; every command carries
the `OwnerGeneration` the admission minted in the lock file and a stale one is
refused. Hosts serve typed `OwnerCommand`s carried out by `NativeEngines::apply`
(a second `magistr run` acts through them via `AttachedEnginePort`); the MCP
service serves its rmcp session (a second `magistr mcp-serve` proxies its stdio
to it). The endpoint lives as long as the owner's invocation, a detach cancels
nothing, a deadline or hang-up spends the connection until an explicit
reconnect, and an unavailable endpoint fails closed — read-only is the explicit
observer path. Local only: no remote endpoint, daemon, upgrade path across
protocol versions, or shared writer routing.

## Implemented native command path

`arch-12-native-command-path` makes the library's own tool handler the
[one implementation](project-authority.md#one-write-path) of every supported
native mutation. `ProjectAuthority::open` composes the four family services
as the standalone binary does; `NativeEngines` carries each typed verb out by
calling the handler method directly and reading its structured answer as a
wire client would; `serve` hands the same service instances to the transport
as its `UnifiedService`, with the writer beside them. The observed drift is
closed by construction: promoting a signal from the cockpit now creates and
cross-links the backlog chunk exactly as the MCP verb does, and the verbs'
refusals are the same on every surface. `magistr_gate` runs on the pty
runner and records through `ship_record_gate` on the task named active before
the run; `magistr_set_phase` records its action through the writer; the
transport's own MCP self-calls are gone. The loop driver runs the gate set
completion verifies (`required_gates()` of the same evidence capability, at
the moment the set is due), records each on the named task under the
contract's name and command, and a judged completion carries `task:<that
task>`. Two direct
engine calls remain, documented and pinned: `record_check_full` (the
recording half of `ship_check`) and `propose_title` / `propose_status` (no
MCP tool). Not claimed: native run isolation (one objective and active task
per project, loops serialized), a production adapter for the application
crate's `CommandContext`, or any desktop writer on main.

## Implemented durable operations

`arch-13-durable-operations` records the loop's own operations — the attempt's
reservation, the provider dispatch, each gate receipt, each native mutation,
each published frame — in the loop checkpoint as `OperationRecord`s with
stable command/attempt identities and `Accepted`/`Running`/`Completed`/
`Failed`/`Cancelled`/`Indeterminate` outcomes, written before the effect and
settled after it. The checkpoint (run state, stop counters, budget, ledger,
run identity) and the cost ledger are replaced atomically. Recovery marks
in-flight records `Indeterminate` and reconciles them against the native
store, the runtime's liveness and the attempt's Git baseline; what stays open
refuses the next attempt without latching a terminal reason. Deduplication
returns only an established `Completed` outcome; a retry is the next attempt
number. See [durable operations](durability.md). Not claimed: exactly-once
external effects, a production adapter for the application `CommandContext`,
or any change to native storage.

## Executable scenarios

The original scenarios intentionally pinned behavior including gaps. The run
rows below now assert the implemented policy; other rows retain their scoped
baseline coverage. A changed contract requires updating its scenario explicitly.

| Scenario | Executable evidence |
| --- | --- |
| Shared core decisions, project context, noninteractive refusal/compatibility, scoped remembered answers | CLI integration `shared_permissions`; CLI `agent::session::tests`, noninteractive response and configuration tests |
| Shared mode policy, two no-commits stop, repeated settlement remains idempotent | [Domain run-state tests](../../crates/magistr-domain/src/workflow/run_state.tests.rs); engine `workflow::workflow_engine::tests::should_stop_on_repeated_no_commits` and `a_restored_no_commit_count_still_stops_the_loop` |
| Recovery retains stopping count, Git baseline, budget and terminal reason; unknown legacy state is refused | [Core iteration tests](../../crates/magistr-core/src/workflow/iteration.tests.rs), including `recovery_retains_no_commit_stop_before_another_attempt`, `repeated_post_check_settles_each_attempt_once_even_after_recovery` and `recovery_observes_commit_against_the_persisted_attempt_baseline` |
| Version 2 requires consistent run/governor evidence; restored budget, pause, abort and cancellation remain enforceable | [Session snapshot tests](../../crates/magistr-engine/src/session/session_snapshot.tests.rs) and [engine-handle recovery tests](../../crates/magistr-engine/src/handle/mod.tests.rs) |
| Phase/status compatibility and one project-context value across core/engine | [Core phase tests](../../crates/magistr-core/src/workflow_phase.tests.rs), [iteration status compatibility](../../crates/magistr-core/src/workflow/iteration.tests.rs) and `gathered_context_is_accepted_by_the_engine_without_conversion` in [context builder tests](../../crates/magistr-core/src/workflow/context_builder.tests.rs) |
| Shared objective/active task, independent projects, weak completion/phase/finalize evidence | CLI integration `architecture_contracts` (temporary native store and actual failing child command) |
| Policy Allow/Deny/Ask, protected edit/delete, remembered workflow refusal and precedence | CLI integration `cockpit_permissions`; domain permission-service and CLI session-policy companion tests |
| Permission round trips, no title-only bypass, invalid/stale options, pending cancellation and first Ctrl-C cancel then quit | CLI supervisor permission tests and CLI integration `session_harness` through the real TUI renderer |
| Cockpit FIFO admission with ordered transcript, cancellation kept active until the provider acknowledges, stream loss without a fabricated terminal, detach retaining the resident turn | CLI integration `cockpit_session_contract` over a real in-process ACP agent |
| Headless and cockpit attachments share policy decisions and normalized terminal outcomes; only Ask reaches the cockpit and a reply naming no offered option revokes it | `headless_and_cockpit_attachments_share_policy_decisions_and_terminal_outcomes` in [supervisor tests](../../crates/magistr-cli/src/agent/session_supervisor.tests.rs) |
| Rejected/missing explicit model prevents inference | CLI `agent::backend::tests` |
| Provider cancellation while waiting/queued, fresh turn after cancellation | ACP-agent integration `agent` |
| Codex terminal-acknowledged cancellation and restored thread/history/model | ACP-agent integration `codex_provider` |
| Headless cancelled/empty-stream outcomes and exit mapping | CLI `orchestration::commands::run::cmd_run_headless::tests` |
| Admitted owner lifetime/contention/aliases and occupied sockets; Service and Host roles coexist and each refuses a second of itself; readers and a real `magistr roadmap status` process answer beside a running owner without a lease | Engine `native::project_authority::tests`; cockpit `engine::client::tests`; CLI integrations `mcp_service_owner`, `mcp_project_owner` |
| Occupied ownership reached through the owner endpoint with no second lease; stale generation refused until reconnect; detach cancels nothing; command deadline on a silent owner; handshake refuses protocol/project/role/kind; owner shutdown leaves the client without a writer; a second cockpit acts through the running host and fails closed without an endpoint; a real second `magistr mcp-serve` attaches, writes through the owner and ends with it | Engine `native::owner_endpoint::tests`, `native::owner_generation::tests`, `native::owner_socket::tests`; cockpit `engine::client::tests`; CLI integration `mcp_project_owner` |
| Native iteration with real recorded gates and task/think completion references | CLI integration `next_iteration_integration` |
| Red outer gates reopen self-completion; green evidence retains it | CLI `orchestration::loop_driver::driver::tests` |
| Cached-writer merge/reload, detached projection reseed and the requested resynchronization that confirms an unchanged store | Engine `native::native_engines::tests`; cockpit `engine::reseed::tests`, `cockpit::projection::tests`, `cockpit::update::tests` (reconnect over an unchanged store, owner change) |
| One write path: in-process promotion creates and cross-links the backlog chunk as the verb does; the handlers' refusals are the writer's; a green `magistr_gate(all)` leaves one verified check per gate on the named task and opens `done`, running each command once; a judged loop completion names the task its gates were recorded on; no writer escape hatch outside `magistr-engine/src/native` and no transport self-call | Engine `native::native_engines::tests`, `native::handler_call::tests`; CLI integration `mcp_serve_integration`; CLI `orchestration::loop_driver::driver::tests`; the `native-write-path` gate |
| Durable operations: outcome transitions and admission rules; atomic checkpoint survives a torn temporary; crash injection around reservation, native mutation, receipt, publication and a real provider child process; an open dispatch ends the loop `Unreconciled` without a launch or a terminal reason; a judged iteration leaves every kind settled under one run | [Domain ledger tests](../../crates/magistr-domain/src/workflow/operation_ledger.tests.rs); [core crash-injection tests](../../crates/magistr-core/src/workflow/iteration_operations.tests.rs), `durable_write` and `process_alive` tests; CLI `orchestration::loop_driver::driver::tests` |

Run the named new and directly related suites without an empty-filter success:

```sh
cargo nextest run -p magistr-cli --test architecture_contracts
cargo nextest run -p magistr-cli -p magistr-core -E 'test(agent::session::tests::) | test(workflow::iteration::tests::)'
```

The full `.magistr/project.json` gate set is required as well. No live-provider
opt-ins or golden-update switches are part of baseline verification.

## Change-locality measurements

These are explicit source **reading sets**, not predictions that every file must
change. Count each listed file once; companion tests add verification cost.
Repeat the same use cases after a migration slice and explain any changed set.

| Representative change | Main reading set | Concepts crossed |
| --- | --- | --- |
| Add a permission outcome | 8 files: CLI `agent/session.rs`, `permissions/cockpit_policy.rs`, `orchestration/commands/run/cockpit_policy_for_project.rs`; cockpit `agent/session.rs`, `cockpit/model.rs`, `cockpit/update.rs`, `cockpit/view.rs`; ACP `permission/response.rs` | Host mode, project policy, request correlation, UI state/action/view, protocol response |
| Add a provider capability | 7 files: CLI `agent/connect_transport.rs`, `agent/backend.rs`; ACP-agent `provider.rs`, `serve.rs`; ACP `event.rs`; cockpit `agent/translate.rs`, `agent/event.rs` | Provider promise, wire advertisement, launch/model checks, lossy UI projection |
| Change a run stop rule | 7 files: core `workflow/iteration.rs`; engine `workflow/workflow_engine.rs`; CLI `orchestration/loop_driver/driver.rs`, `orchestration/commands/next/cmd_next_loop.rs`, `cmd_next_loop_headless.rs`, `cmd_next_loop_cockpit.rs` in that same next directory; cockpit `cockpit/update.rs` | Two policies, persistence/recovery, host dispatch, loop outcome presentation |

That table retains the original reading set. After `arch-03-run-semantics`, the
stop-rule definition and its direct tests live in domain `workflow/run_state.rs`
and `run_state.tests.rs`. Core and engine adapters delegate to that definition.
Changes to effect timing, serialization or displayed outcomes still require
reading their adapters and CLI consumers; one policy owner does not eliminate
those integration costs.

The gui2 permission reading set adds five files under `magistr-gui2/src`:
`comp_moment.rs`, `comp_root.rs`, `frame.rs`, `native.rs`, `route.rs`.
Two map semantic permission controls; three trace generic target/input/dispatch.
For stop controls the five are `comp_rail.rs`, `ledger_verbs.rs`,
`comp_root.rs`, `native.rs`, `route.rs`. Desktop has zero automatic-stop
policy owners. It has zero provider-capability policy owners; shared
`agent/translate.rs` currently drops capability data before `agent/event.rs`.
Zero here describes missing integration, not zero implementation cost.

Reproduce branch comparisons with `git diff <main-sha> <gui2-sha> -- <path>`
and inspect a listed historical file with `git show <sha>:<path>`.
These sets describe change locality, not latency, defect rates or a performance
improvement. Existing release-latency gates remain the performance evidence.

[gui-guide]: https://github.com/AlrikOlson/magistr/blob/f0d6370ac32877160e6d40852b8051736e4ca6ec/docs/architecture/desktop.md
[gui-window]: https://github.com/AlrikOlson/magistr/blob/f0d6370ac32877160e6d40852b8051736e4ca6ec/crates/magistr-gui2/src/window.rs
[gui-native]: https://github.com/AlrikOlson/magistr/blob/f0d6370ac32877160e6d40852b8051736e4ca6ec/crates/magistr-gui2/src/native.rs
[gui-route]: https://github.com/AlrikOlson/magistr/blob/f0d6370ac32877160e6d40852b8051736e4ca6ec/crates/magistr-gui2/src/route.rs
