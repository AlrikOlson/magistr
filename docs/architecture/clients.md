# Clients and consistency

Every supported Magistr surface that reads or writes the native workflow
store or the workspace reaches one canonical capability. This document is
the audit of those surfaces (`arch-14f3-client-audit-consistency`): which
capability each one holds, how it is admitted, and which test proves it;
how duplicate ownership is refused; why the owner-command vocabulary was
served; and the consistency model across the project owner, working-copy
executions, attached clients and readers.

The capabilities are those of the [project authority](project-authority.md)
and the [workspace owner](workspace-owner.md):

| Store | Write | Read | Attached |
| --- | --- | --- | --- |
| Workflow (think-and-ship families) | `NativeEngines`, one admitted project authority, with execution-bound views, only from `ProjectAuthority::open`/`serve` | `NativeReader`, no lease | `OwnerClient` on the project endpoint: typed commands (`commands`) or an MCP session (`mcp`); no lease, no engine |
| Workspace (one checkout) | the admitted `WorkspaceService`, one per canonical root, only from `WorkspaceService::open` | lock-free readers: `SourceSnapshot::capture`, `JournalInspection`, `DoctorRun` | a bridge attached over `OwnerClient::attach_workspace` (`mcp`), speaking the owner's tool surface |

## Client inventory

Every row is a surface that exists in the binary today; the evidence column
names the test that exercises the row's admission. Paths are under
`crates/magistr-cli` unless another crate is named.

| Client | Workflow capability | Workspace capability | Admission path | Evidence |
| --- | --- | --- | --- | --- |
| `magistr mcp-serve <root>` and its alias `magistr mcp-launch magistr` — the agent's tools from `.mcp.json`, the temporary config `tools/server.rs` writes, and Codex's `config.toml` | `NativeEngines`, `Service` role; `ProjectAuthority::serve` hands the same family services to the MCP wire and the writer to `MagistrService` | `WorkspaceEvidence` (lock-free `SourceSnapshot` + gate resolver) attached to the writer config in the constructor; `magistr_gate` admits and settles through `AdmittedCheck`; no owner | `mcp_tools/service.rs` `MagistrService::new` | `tests/mcp_service_owner.rs`, `tests/mcp_project_owner.rs`, `tests/mcp_serve_integration.rs::one_server_serves_both_surfaces_and_records_gates_through_ship`, `::a_green_gate_run_is_the_only_check_the_done_phase_needs` |
| A second `magistr mcp-serve` on a project whose project lease is held | attached `OwnerClient` (`mcp`): its stdio is proxied to the owner's rmcp session; no lease, no engine; reconnects on the next request after owner loss | none | `mcp_tools/owner_link.rs` through the shared `mcp_bridge` after the lease refusal | `tests/mcp_project_owner.rs::simultaneous_stdio_start_has_one_owner_and_kill_allows_fresh_restart`, `magistr-engine` `tas/owner_endpoint.tests.rs::an_mcp_attachment_hands_the_socket_to_the_acceptor_after_the_welcome`, `::the_handshake_is_validated_before_anything_is_served` |
| `magistr run` (cockpit) and the `magistr-desktop` executable (both compose `application/interactive_host.rs` `InteractiveHost::start`; the desktop's launch evidence is in [desktop](desktop.md#launch-evidence-and-platforms)) | `NativeEngines`, `Host` role, serving its `OwnerEndpoint` for the run | `WorkspaceAdmission::admit` (`HostedWorkspace`): the checkout's owner for the run, endpoint served, `WorkspaceSourceWriter` as the session's `SourceRoute`; the cockpit's `McpWorkbench` port spawns the `.mcp.json` bridge, which attaches to this host; the journal model reads `magistr workspace journal` | `magistr-cli` `application/workflow_client/client.rs` `EngineClient::admit_or_attach_config` on `writer_config` (refusing a config without evidence), then `commands/run/cmd_run_cockpit.rs` `admit_workspace` | `cmd_run_cockpit.tests.rs::the_engines_and_the_agent_come_before_the_terminal_and_their_failures_are_printed`, `magistr-cli` `application/workflow_client/client.tests.rs::a_second_host_on_the_same_project_is_refused_until_the_first_is_gone`, `::a_writer_config_without_evidence_is_refused_before_the_lease`, `tests/source_pipeline_routing.rs::an_authorized_write_lands_through_the_pipeline_with_a_journal_intent_and_a_receipt` |
| A second `magistr run` on a project whose project lease is held | attached `OwnerClient` (`commands`) behind `AttachedEnginePort`: every cockpit verb is carried out by the running host's writer; reads come from the cockpit's own `NativeReader`, the feed and the re-seed timer | `WorkspaceAdmission::Refused(OwnerElsewhere)` (a live owner holds the lock): nothing admitted, nothing attached for writing; the session advertises no write capability and refuses `fs/write_text_file`; its bridge attaches to whoever owns the root | `EngineClient::admit_or_attach_config` after `WouldBlock`; `admit_workspace` | `application/workflow_client/client.tests.rs::a_second_cockpit_attaches_to_the_running_host_and_acts_through_it`, `::a_held_host_without_an_endpoint_fails_closed`, `::an_attached_port_reports_a_gone_owner_and_opens_nothing`, `tests/workflow_consistency.rs::an_attached_command_is_current_on_the_owner_and_stale_on_the_other_role_until_it_reloads`, `tests/source_pipeline_routing.rs::a_held_workspace_refuses_the_second_hosts_writes_and_invents_no_receipt`, `tests/effect_capabilities.rs::a_held_workspace_refuses_the_write_without_asking_and_the_read_goes_on` |
| `magistr next` and `magistr next-loop` in the cockpit | `NativeEngines`, `Host` role, handed to `IterationLoop`; the cockpit attaches to that writer in-process (`EngineClient::attach`), never a second one; every gate through `AdmittedCheck`; the judge completes through the same writer | `WorkspaceAdmission::admit` per loop run in `run_cockpit_loop`, so the agent's bridge and the cockpit's port attach to the loop and every iteration's session shares one `SourceRoute`; gates observe through `WorkspaceEvidence` | `orchestration/roadmap/admit_native_host.rs` from `cmd_next_cockpit` / `cmd_next_loop_cockpit` | `commands/next/run_cockpit_loop.tests.rs::the_loops_admitted_writer_is_attached_before_the_terminal_and_never_re_admitted`, `orchestration/loop_driver/driver.tests.rs::every_gate_the_loop_runs_reaches_the_trace_as_a_verified_check`, `::a_judged_completion_names_the_task_its_gates_were_recorded_on`, `tests/application_composition.rs` |
| `magistr next-loop` plain and `--headless` | `NativeEngines`, `Host` role, via `admit_native_host`; a loop opened without a writer admits the host itself (`magistr-core` `workflow/iteration.rs` `auto_native`) | `WorkspaceAdmission::admit` once per loop (`admit_workspace`), the route handed to every iteration's session and released before the loop's exit; a refusal means the sessions' source writes are refused; gates observe through `WorkspaceEvidence` | `cmd_next_loop.rs`, `cmd_next_loop_headless.rs` → `IterationLoop::open_or_recover` | `commands/next/cmd_next_loop_headless.tests.rs`, `orchestration/roadmap/open_native_reader.tests.rs::native_project_admits_one_host_and_readers_on_the_configured_store`, `native-write-path` gate rules 8 and 10 |
| `magistr next` plain, `--headless` and `--continue` (one iteration) | `NativeReader` for the progress summary (`WorkflowConfig::engines`); the writer is the agent's own `magistr mcp-serve` (first row) | `WorkspaceAdmission::admit` for the one run (`cmd_next`, `cmd_next_continue`, `cmd_next_headless`), released after the session; the agent's bridge attaches to it | `magistr-core` `workflow/mod.rs` `WorkflowConfig::engines` → `NativeReader::open` | `commands/next/cmd_next_headless.tests.rs::workflow_session_uses_selected_project_for_execution_and_context`, `tests/next_iteration_integration.rs::headless_iteration_is_guarded_on_ship_state_and_cross_referenced` |
| `magistr run` plain and `--headless`, the plain workflow session (`cmd_run_workflow`), `magistr roadmap generate` and the init interview | none in this process; the agent's `magistr mcp-serve` writes | `WorkspaceAdmission::admit` for the run (the caller of `cmd_run_workflow` admits and passes the `SourceRoute`), released after the session; a refusal means the session's source writes are refused, never direct | `commands/run/cmd_run.rs`, `cmd_run_headless.rs`, `cmd_run_workflow.rs`, `commands/misc/cmd_roadmap.rs`, `commands/init/cmd_init_interactive.rs` | `commands/run/cmd_run_headless.tests.rs`, `tests/effect_capabilities.rs::an_admitted_workspace_routes_the_approved_write_and_the_session_goes_on`, `magistr-acp` `capabilities/local_effect_port.tests.rs::a_refused_route_withholds_the_write_capability_and_refuses_before_authorization` |
| Bare `magistr`, `magistr roadmap` (observer cockpit), the Home launcher | `NativeReader` via `EngineClient::observe`; the feed when the Service publishes, the re-seed timer otherwise; no port | none | `commands/cockpit.rs` | `application/workflow_client/client.tests.rs::a_second_cockpit_attaches_to_the_running_host_and_acts_through_it` (the observer beside the host, serving no endpoint), `tests/mcp_project_owner.rs::a_reader_command_answers_beside_the_running_owner_without_a_lease` |
| `magistr roadmap show`, `next`, `status`, `export`; `magistr guard` (the stop hook) | `NativeReader` via `open_native_reader` | none | `commands/misc/cmd_roadmap.rs`, `commands/config/cmd_guard.rs` | `tests/mcp_project_owner.rs::a_reader_command_answers_beside_the_running_owner_without_a_lease`, `tests/roadmap_export_policy.rs`, `commands/config/cmd_guard.tests.rs::apply_engine_state_reads_the_ship_and_roadmap_documents` |
| `magistr validate`, `magistr next --gate-only` | none: `ProjectValidation` runs the gate commands and records nothing, so its run is a report, never evidence a chunk can complete on | none | `magistr-core` `validate.rs` | `tests/validation_project_gates.rs::validation_modes_enforce_required_custom_and_archetype_gates` |
| ACP `fs/write_text_file` inside a host session | none | the host's `SourceRoute`: `WorkspaceSourceWriter` over the hosted service's `write` verb after the session's permission decision (journal intent, `MutationReceipt`, event `route: workspace`); a `SourceRefusal` withholds the write capability at initialize and refuses the call before authorization (event `route: refused`); there is no direct write | `magistr-acp` `capabilities/local_effect_port.rs` with `AcpTransport::local_effects(route)` | `tests/source_pipeline_routing.rs` (all seven), `tests/effect_capabilities.rs`, `magistr-acp` `capabilities/local_effect_port.tests.rs`, `tests/transport.rs::a_refused_source_route_withholds_the_write_capability_and_reads_go_on`, `native-write-path` gate rule 12 |
| ACP `fs/read_text_file`, `terminal/*` | none | direct host I/O after authorization | `magistr-acp` `handlers/fs.rs`, `handlers/terminal.rs` | `tests/effect_capabilities.rs::direct_reads_and_approved_writes_use_the_shared_profile`, `::a_remembered_process_refusal_prevents_direct_terminal_spawn` |
| `magistr mcp-launch workbench-mcp <root>` and `magistr workspace serve <root>` — the bridge every `.mcp.json`, Codex `config.toml` and the cockpit's port launch | none | the admitted `WorkspaceService` when the root is free, else an attached bridge (`OwnerClient::attach_workspace`) within the five-second window; explicit `--config` refuses attachment; a held lock without an endpoint or an unmigrated marker defers into the bridge, which answers `initialize` and the listing from the shared capability definition and reconnects on its next request (`audit-260912-20`); reconnects lazily on owner loss | `workspace_mcp/connection.rs` `WorkspaceConnection::open` | `tests/workspace_attachment.rs` (twenty scenarios, two of them startup deferral), `tests/workspace_served.rs::status_is_byte_identical_across_fresh_owners_and_a_second_owner_is_refused`, `tests/workspace_served_stdio.rs`, `tests/mcp_launch_integration.rs::relocated_project_is_served_natively_from_a_nested_directory` |
| `magistr codex …` (the Codex wrapper) and Codex's parent and subagent threads | none in the wrapper; the threads launch the first row and the bridge from `.codex/config.toml` | as those rows | `codex_launch.rs`; `scaffold/codex_config.rs` registers only `magistr` and `workbench-mcp` (`BUILTIN_MCP_SERVERS`) | `tests/codex_runtime.rs::native_codex_launch_preserves_argv_stdio_environment_and_exit_status`, `scripts/codex-subagent-check.py` (the optional `codex-subagent` gate; passed 2026-09-12) |
| `magistr workspace journal`, `snapshot`, `doctor` | none | lock-free readers of committed state: `JournalInspection::rows`, `SourceSnapshot::capture`, `DoctorRun`; no lock taken, so they answer beside a live owner. `doctor` adds one bounded `probe` handshake on the owner endpoint (`OwnerProbe::workspace`: welcomed, told the owner's report, closed; never a session, never an admission) and a read of the run checkpoint as it lies (`IterationState::read`) | `workspace_cli/journal.rs`, `snapshot.rs`, `doctor.rs`, `doctor_owner.rs`, `doctor_operations.rs` | `tests/workspace_cli_verbs.rs::journal_and_doctor_run_beside_a_live_owner_without_taking_or_disturbing_the_lock`, `::snapshot_prints_the_source_observation_beside_a_live_owner_and_refuses_bad_inputs`, `::doctor_explains_stalled_then_recoverable_operations_and_a_legacy_marker`, `tests/workspace_attachment.rs::doctor_reports_the_live_owner_and_counts_attached_bridges_as_they_come_and_go` |
| `magistr workspace init`, `key rotate`, `migrate` | none | none opens a service: `init` seeds `workbench.toml`, `key rotate` rewrites a served workspace's key store, `migrate` moves a dead legacy lock record aside | `workspace_cli/init.rs`, `key.rs`, `migrate.rs` | `tests/workspace_cli_verbs.rs::init_seeds_the_commands_table_once_and_never_touches_an_existing_one`, `::key_rotate_needs_a_served_workspace_and_then_replaces_the_key`, `tests/workspace_served_stdio.rs::legacy_lock_refusal_preserves_the_record_and_never_opens_workspace_stores` |
| `magistr --workspace-worker` | none | none: a private child of one owner with no lock and no service; ends with its parent | `workspace_mcp` worker supervision | `tests/workspace_attachment.rs::a_killed_owner_takes_its_worker_with_it`, `::an_orphaned_worker_whose_stdin_stays_open_ends_by_itself` |
| The loop's judge sessions | the loop's own writer, in-process (no capability of their own) | none: composed with `SourceRoute::Refused(NoAuthority)`, so a judge advertises no write capability and any write is denied | `orchestration/loop_driver/driver.rs`, `commands/next/run_cockpit_loop.rs` `judge` | `driver.tests.rs::an_ok_verdict_over_green_gates_completes_the_chunk_through_the_engines` |

Not clients: `magistr acp-agent` (a provider adapter whose tools arrive
through the first row and the bridge, launched by the provider CLI); the
`magistr-tui` cockpit example (an observer, not shipped). The standalone
`think-and-ship` binary and CLI were deleted by `arch-14f4-rehome-naming`;
the `think-and-ship` name in `scaffold/codex_config.rs` only recognizes an
entry a user wrote before the cutover. There is no row for a daemon, a remote
endpoint, a Bun or NAPI path, or an internal MCP round trip.

## Host API inventory

The September 2026 retirement audit followed public constructors and aliases
through their callers, including tests and examples. A test of a disconnected
implementation was not counted as a production caller. The decisions below
record that audit; [pre-release policy](../../AGENTS.md#before-public-release) supersedes any
requirement to retain names, aliases, old formats or migration paths.

| Entrance or alias | Consumer and decision |
| --- | --- |
| Agent `SessionSupervisor::start*`, `AgentSession`, `CockpitSession` | Headless, cockpit and judge sessions; retain the shared admission, bounded message and settlement paths. |
| Workflow `EngineClient::with_config`, `admit_or_attach_config`, `observe`, `observe_config`, `attach` | Host admission, attachment, observation and loop reuse are distinct capabilities. All writes reach `ProjectAuthority`; readers never create writers. Retain. |
| `NativeEnginePort::new`, `AttachedEnginePort::new` | The workflow client's two admitted command routes. Both translate the same interface commands to canonical use cases. Retain. |
| `ProjectAuthority::admit` / `admit_for_project`, `open` / `serve`; `NativeReader::open` / `open_for_project`; `OwnerClient::attach*` | Host, Service, reader and bridge entrances listed above. Explicit project/config variants preserve captured inputs; no alternate store bootstrap. Retain. |
| `HostedWorkspace::admit` / `admit_with`, `WorkspaceAdmission::admit` / `admit_with`, `WorkspaceSourceWriter::new` | Host composition, configured options, refusal reporting and generation-checked source writes. Retain. |
| `AdmittedCheck::admit` | MCP and loop checks share admission, source observation and settlement. Retain. |
| `McpWorkbench::new` / `with_workspace` | Cockpit bridge from project configuration. Accept the native `mcp-launch workbench-mcp` launch shape; refuse old standalone settings before spawning. Journal and undo use one owner connection. |
| `EngineHandle::new` / `with_run_mode` / `with_native` | Session checkpoint capture/recovery and its underlying run semantics. Retain; this is not the deleted agent client facade. |
| CLI foundation, model and persistence aliases; core/engine domain and workflow value reexports | Currently re-export owning definitions. They impose no preservation requirement: remove redundant aliases and update current callers and tests directly. |
| `agent::engine`, its root alias, local/channel/remote clients, reconnect and transport task | No caller outside their own tests. Removed together with the unserved `magistr-protocol` crate and its stdio/Unix/TCP/WebSocket transports. |
| Engine fleet/batch modules and reexports; CLI `agent::multi` constructors and aliases | Only simulated integration tests and internal callers. Removed; no distributed execution is advertised. |
| Engine `peer` | Preserve the tested bounded agreement/replay rules as an opt-in experiment. Production dependency resolution must not enable `experimental-peer`. |

The application dependency test checks that the retired wire crate is absent
and the peer feature is disabled in production resolution. The workspace port
tests exercise refusal before process launch and journal reads through the owner.
Existing session, checkpoint, admission and canonical command tests cover the
retained entrances. Owned subsystem licenses, provenance and reference
inputs remain in `subsystems/`.

## Optional Claude interface

The [Claude Code Mod](claude-mod.md) is an optional client of the agent's
existing MCP connections in the first and Workbench bridge rows. It holds no
native capability or lease itself. Read-only Mod MCP calls inspect canonical
queries; confirmed actions use Claude's normal tool path and the same native
application admission. It never opens stores, admits owners or writes source.
Its disposable snapshots are independent observations per family, with no
global cursor. The owner phase query reports only this service's explicitly
announced phase, associated with the objective and task; it is not a new
workflow engine.

Evidence: `mcp_serve_integration.rs::context_reads_owner_phase_and_canonical_evidence_without_mutation`,
`::context_refuses_malformed_and_unknown_task_requests_over_mcp`, and
`plugins/magistr/tests/*.contract.mjs`. Official Claude tests are in
`plugins/magistr/tests/mod.test.ts`; live acceptance is separately reported
in [integration verification](../reference/claude-mod-verification.md).

## Duplicate ownership is refused

- **A second project writer.** `ProjectAuthority::admit` takes the shared
  project and store locks before any engine opens. Another host or MCP
  writer receives `WouldBlock`; it attaches to the existing owner when an
  endpoint is available. Clones retain the lease; readers hold none.
  Evidence: `native/project_authority.tests.rs::one_project_writer_is_admitted_across_host_and_service_roles`,
  `tests/workflow_consistency.rs::a_second_project_writer_is_refused_in_process_and_the_lease_ends_with_the_last_clone`,
  `tests/mcp_service_owner.rs`, `application/workflow_client/client.tests.rs::a_second_host_on_the_same_project_is_refused_until_the_first_is_gone`.
- **A second workspace owner.** `WorkspaceService::open` holds
  `.workbench/server.lock`; `Locked` makes the bridge attach and makes the
  host admit nothing — neither opens a second service, in-process or
  across processes. Evidence: `tests/workspace_served.rs::status_is_byte_identical_across_fresh_owners_and_a_second_owner_is_refused`,
  `tests/workflow_consistency.rs::a_second_hosted_workspace_on_one_root_admits_nothing_while_the_first_lives`,
  `tests/source_pipeline_routing.rs::a_held_workspace_refuses_the_second_hosts_writes_and_invents_no_receipt`,
  `tests/workspace_attachment.rs::eight_simultaneous_clients_admit_exactly_one_owner_and_all_receive_the_tool_surface`.
- **No new path.** The `native-write-path` gate pins the inventory: rule 9
  allows `WorkspaceService::open` only in `workspace_mcp/connection.rs` and
  `application/hosted_workspace.rs`; rule 10 allows
  `ProjectAuthority::admit` only in `mcp_tools/service.rs`,
  `orchestration/roadmap/admit_native_host.rs`, `magistr-core`
  `workflow/iteration.rs` and `magistr-cli` `application/workflow_client/client.rs`; rule 11
  allows `OwnerClient::attach*` only in the cockpit client, the `mcp-serve`
  proxy and the workspace bridge; rule 12 forbids any direct file write in
  the ACP host, so a session's source writes have exactly the routed path or
  a refusal. A production file that opens, admits, attaches or writes
  anywhere else fails the required gate with its line.

## Working-copy clients

`magistr workspace working-copy` dispatches create/list/open/archive through
`application::WorkingCopies`, using Git's existing registry. The service owns
the lifecycle decisions and returns typed dirty/conflicted/active/unavailable
outcomes. CLI and future desktop callers use the same service.
[Project authority](project-authority.md#working-copy-lifecycle) defines archive
and reopen, including preservation of files, branch, journal and history.

## Owner commands

`OwnerCommand` carries the typed workflow verbs implemented by
`NativeEngines::apply`. `AttachedEnginePort` supplies them for cockpit actions.
The MCP bridge forwards MCP frames to the same owner. The CLI's
`MagistrService::serve_owner_endpoint` composes both routes with `serve_with`
for either host kind. The owner admits a canonical working copy and execution
id before welcoming a connection and passes the bound view to its MCP adapter.

## Workflow consistency model

One project writer owns the shared roadmap, reasoning and signals. Its registry
owns ship engines per canonical working-copy root and execution id. Identical
live contexts share handles; different contexts retain their own selected
objective, tasks, checks and receipts. Persistence reopens the same context
without replacing another context's selected scope. This is an ownership
contract, not a timestamp-based merge between competing workflow writers.

Every family persists atomically under its data partition. Ship is partitioned
under `ship/executions/<context-key>`; project planning stays under its existing
project identity. Family use cases hold their engine locks while validating and
mutating. A long-running check captures root, execution, workflow generation,
run and task instance before running and settles against that admission.
Source freshness comes from the independent source owner's observation.

Readers observe each family's store stamp (inode, modification time and length)
before answering. Changed stamps cause reads, unchanged stamps reuse memory.
Derived readers share the writer's handles; lease-free external readers observe
the persisted files. Snapshots are consistent per family, not across families.

The project event feed has one shared sequence and epoch. Task and phase/gate
events carry an execution key. A client advances its cursor for every decoded
frame, then delivers execution-specific events only to the selected execution.
Shared planning events reach every client. Reconnection requests a contextual
snapshot; owner loss never replays a possibly applied mutation.

`tests/workflow_consistency.rs` covers attached commands, shared views and
lease lifetime. Engine context tests cover independent scopes and persistence.
`tests/mcp_project_owner.rs` runs real concurrent linked-worktree processes,
including source edits, approval refusal across roots, actual checks,
independent receipt freshness, owner exit and bounded recovery. The feed
companion suite proves that foreign task events do not replace the selected
task or create sequence gaps.

### Workspace consistency

The workspace has one owner per canonical root, so every mutation — a
bridge's `edit`, the host's routed `write`, the cockpit's `undo` — runs
through one transaction pipeline, one journal and one index under one
generation: anchored edits are revalidated against current bytes and
applied serially, a confirmation is bound to its generation and session,
and a peer's edit is visible to the next call of every client. The
lock-free readers (`journal`, `snapshot`, `doctor`) read committed on-disk
state and never wait on the owner. External writes — an editor, a
provider CLI, git — bypass the pipeline: they leave no journal row and no
receipt, are seen by `SourceSnapshot` validation as changed paths and by
the index watcher, and are the reason a check's source freshness can say
`Changed`. The workspace generation, a source revision and a mutation
receipt's revision stay three facts (see the
[identity map](application.md#identity-map)).

## Limits

The audit is of Magistr's own binaries: an older binary, a direct edit of
a store file or a deleted lock file are outside admission, and an in-place
rewrite of a store file inside one timestamp tick at the same length is
invisible to the stamp. Read-through is per family: there is no
transaction across the roadmap, ship, signal and think files, and a
mutation is not serialized against a concurrent writer of the same record
— the merge rule decides. [Projections](projections.md) made what a client
reads typed and freshness-aware; the stamp decides when the engines behind
them re-read. The consistency tables describe one machine and one user. Workflow command
and MCP attachment still require Unix sockets. Workspace attachment also has
an authenticated Windows named-pipe path. Off Unix the store stamp carries
no inode. See [the Claude adapter](claude-mod.md) for the difference between
a reachable source connection and an unavailable workflow connection.