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
| Project | `TasConfig` and native identity select the project/store; `MagistrService` owns cooperating MCP admission. Configuration resolution also lives in CLI/core. | One admitted project application handle supplies mutations to every supported host. Workbench retains its separate service ownership. |
| Chunk, signal, reasoning, execution objective/task | The four native think-and-ship family engines behind `NativeEngines`; scope strings and cockpit selections do not create isolated execution cycles. | Keep native workflow canonical behind narrow commands and queries. |
| Run/attempt | CLI/core `IterationLoop` owns production iteration limits, Git progress and recovery; `LoopDriver` coordinates cockpit execution. Engine `WorkflowEngine` separately models phase, stop counters and checkpoints. | One deterministic run policy; runtime coordinates effects and persists checkpoints. |
| Session/turn | Headless/plain `AgentSession` and cockpit `agent::Session` are distinct lifecycle owners over the same `connect_transport` seam. Providers own opaque conversation IDs. | A shared supervisor owns live tasks, provider attempts and terminal outcomes; the interface observes and sends commands. |
| Decision | `CockpitPolicy` evaluates protected paths, profile, charter and governance; cockpit Session correlates requests and answers. Headless has a separate option-selection worker. | Shared policy decides Allow/Deny/Ask; hosts resolve Ask according to an explicit interaction mode. |
| Verification | MCP gate service and CLI `LoopDriver` execute checks; native Ship records them. Native roadmap completion and phase admission are separate operations. | Completion validates applicable required evidence and references the actual verification task. |
| Projection | `StoreSeed`, native feed frames, agent translation and cockpit `Model` describe stored/live facts. They are not another workflow authority. | Typed, freshness-aware views; revision, owner generation and event sequence have distinct meanings. |
| Presentation | TUI owns terminal protocols; cockpit currently includes shared state, cell view/layout and effect adapters. gui2 owns native pixel composition and geometry. | Share semantic state/actions; keep each surface's layout, input and renderer local. |

Current sources: [native facade](../../crates/magistr-engine/src/tas/native_engines.rs),
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
| MCP, including tools called by external coding agents | `MagistrService::{for_project,new}` → `open_engines` → embedded `UnifiedService`; native roadmap/ship/think/signal verbs, plus `magistr_*` phase and gate recording. | Owner lease acquired before opening engines; clones share it. Native verbs dispatch directly, not through the phase guard. |
| Interactive cockpit `run` | `cmd_run_cockpit` → `EngineClient` → `NativeEnginePort` → `engine/apply`. Pin/checkpoint reasoning; start/complete chunks; resolve proposals; clear blockers; promote/ignore/snooze signals. | Direct cached native writer, outside MCP lease admission. Completion passes no ship reference. |
| Cockpit `next` and `next-loop` | `run_cockpit_loop` → `LoopDriver`; installs the native port for each session, records gates, starts/completes judged chunks, sets human blockers, reopens self-completed chunks after failed required checks. | Direct writer outside MCP lease; completion also passes no ship reference. Outer gates cover only top-level check/test/lint. |
| Headless/plain workflow commands | `open_native_engines` and core loop handles supply roadmap reads/prompts; their agent's MCP service performs native workflow writes. | No outer `LoopDriver` judge/reopen/custom-gate guarantee. Loop JSON and cost/session records are separate persistence. |
| Bare/roadmap cockpit | `cockpit_model_on` opens/subscribes/reseeds `EngineClient`. | Does not install a native command port or session command consumer. Rendered controls alone do not establish a working writer. |
| Library escape hatches | Public `NativeEngines::{open,open_for_project,with_roadmap,with_think,with_ship,with_signal}`. | Constructor does not require a lease. No production callers of mutable `with_*` callbacks were found; future callers must not bypass the application boundary. |

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
writers do not participate in the [cooperative lease](project-authority.md).

Native persistence merges additions on disk but does not refresh another
handle's cached reads. The existing two-writer and reload scenarios demonstrate
this limit. They do not prove serializable application commands or atomic
cross-family writes. Detached cockpit reseeding reloads roadmap/ship/signal on
a changed store stamp; socket attachment or a recent timestamp is not proof of
a coherent global revision.

## Session entry inventory and behavior

| Entry | Lifecycle and permission route |
| --- | --- |
| `run --headless`, headless `next` / `next-loop` | `AgentSession::start` → session supervisor → shared transport; supervisor owns permission handling and callers await termination before exit/next attempt. |
| Plain `run` / workflow drains | `AgentSession::start` uses the same supervisor and resolved permission policy; drains reject missing terminal evidence. |
| Cockpit `run`, TUI init, home resume | `CockpitSession::start` → shared supervisor → application `SessionHandle` → cockpit relay; the supervisor owns pending Ask replies. Home resume supplies an explicit session ID and no opening prompt. |
| Cockpit `next` / `next-loop` | `Connector::live` → shared supervisor and application handle for each attempt; ordinary turns and delegated goals enter explicit owner operations. The outer driver owns workflow settlement. |
| Interactive setup / roadmap generation | `cmd_init_interactive` and `cmd_roadmap` start AgentSession for configuration/planning prompts; these are distinct from roadmap implementation. |
| Workflow judge | `SessionJudge::ask` uses a fresh `Connector` transport, its own event/permission handling, refuses tools and uses its explicit model override or the shared session/provider default. |
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
| Recovery | Core loop persistence omits its separate no-commit counter and recovery resets it to zero. Engine checkpoint restoration retains the count. Cockpit next-loop recovers an existing loop; headless/plain loops start fresh. |
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
timeout and TUI harness do not prove that complete path. Plain CLI SIGINT uses
`utils/signal_handler.rs` to exit with 130 rather than requesting session cancellation.

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

Desktop fixtures construct the same Model/StoreSeed/NativeFrame shapes as live
clients. Those values are sample data, not an additional project/run/session
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
| Admitted MCP owner lifetime/contention/aliases and occupied sockets | CLI integrations `mcp_service_owner`, `mcp_project_owner` |
| Native iteration with real recorded gates and task/think completion references | CLI integration `next_iteration_integration` |
| Red outer gates reopen self-completion; green evidence retains it | CLI `orchestration::loop_driver::driver::tests` |
| Cached-writer merge/reload and detached projection reseed | Engine `tas::native_engines::tests`; cockpit `engine::reseed::tests` |

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
