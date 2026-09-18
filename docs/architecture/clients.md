# Clients and consistency

Every supported Magistr surface that reads or writes the native workflow
store or the workspace reaches one canonical capability. This document is
the audit of those surfaces (`arch-14f3-client-audit-consistency`): which
capability each one holds, how it is admitted, and which test proves it;
how duplicate ownership is refused; why the owner-command vocabulary was
not extended; and the consistency model across the Host writer, the Service
writer, attached clients and readers — as it actually behaves, not as an
ideal.

The capabilities are those of the [project authority](project-authority.md)
and the [workspace owner](workspace-owner.md):

| Store | Write | Read | Attached |
| --- | --- | --- | --- |
| Workflow (think-and-ship families) | `NativeEngines`, one per `WriterRole` (`Host`, `Service`) per project and store, only from `ProjectAuthority::open`/`serve` | `NativeReader`, no lease | `OwnerClient` on the role's endpoint: typed commands (`commands`) or an MCP session (`mcp`); no lease, no engine |
| Workspace (one checkout) | the admitted `WorkspaceService`, one per canonical root, only from `WorkspaceService::open` | lock-free readers: `SourceSnapshot::capture`, `JournalInspection`, `DoctorRun` | a bridge attached over `OwnerClient::attach_workspace` (`mcp`), speaking the owner's tool surface |

## Client inventory

Every row is a surface that exists in the binary today; the evidence column
names the test that exercises the row's admission. Paths are under
`crates/magistr-cli` unless another crate is named.

| Client | Workflow capability | Workspace capability | Admission path | Evidence |
| --- | --- | --- | --- | --- |
| `magistr mcp-serve <root>` and its alias `magistr mcp-launch magistr` — the agent's tools from `.mcp.json`, the temporary config `tools/server.rs` writes, and Codex's `config.toml` | `NativeEngines`, `Service` role; `ProjectAuthority::serve` hands the same family services to the MCP wire and the writer to `MagistrService` | `WorkspaceEvidence` (lock-free `SourceSnapshot` + gate resolver) attached to the writer config in the constructor; `magistr_gate` admits and settles through `AdmittedCheck`; no owner | `mcp_tools/service.rs` `MagistrService::new` | `tests/mcp_service_owner.rs`, `tests/mcp_project_owner.rs`, `tests/mcp_serve_integration.rs::one_server_serves_both_surfaces_and_records_gates_through_ship`, `::a_green_gate_run_is_the_only_check_the_done_phase_needs` |
| A second `magistr mcp-serve` on a project whose Service lease is held | attached `OwnerClient` (`mcp`): its stdio is proxied to the owner's rmcp session; no lease, no engine; ends when the owner ends | none | `mcp_tools/owner_attach.rs` `proxy_to_owner` after the lease refusal | `tests/mcp_project_owner.rs::simultaneous_stdio_start_has_one_owner_and_kill_allows_fresh_restart`, `magistr-engine` `tas/owner_endpoint.tests.rs::an_mcp_attachment_hands_the_socket_to_the_acceptor_after_the_welcome`, `::an_mcp_only_owner_refuses_typed_commands_at_the_handshake` |
| `magistr run` (cockpit) | `NativeEngines`, `Host` role, serving its `OwnerEndpoint` for the run | `WorkspaceAdmission::admit` (`HostedWorkspace`): the checkout's owner for the run, endpoint served, `WorkspaceSourceWriter` as the session's `SourceRoute`; the cockpit's `McpWorkbench` port spawns the `.mcp.json` bridge, which attaches to this host; the journal model reads `magistr workspace journal` | `magistr-cli` `application/workflow_client/client.rs` `EngineClient::admit_or_attach_config` on `writer_config` (refusing a config without evidence), then `commands/run/cmd_run_cockpit.rs` `admit_workspace` | `cmd_run_cockpit.tests.rs::the_engines_and_the_agent_come_before_the_terminal_and_their_failures_are_printed`, `magistr-cli` `application/workflow_client/client.tests.rs::a_second_host_on_the_same_project_is_refused_until_the_first_is_gone`, `::a_writer_config_without_evidence_is_refused_before_the_lease`, `tests/source_pipeline_routing.rs::an_authorized_write_lands_through_the_pipeline_with_a_journal_intent_and_a_receipt` |
| A second `magistr run` on a project whose Host lease is held | attached `OwnerClient` (`commands`) behind `AttachedEnginePort`: every cockpit verb is carried out by the running host's writer; reads come from the cockpit's own `NativeReader`, the feed and the re-seed timer | `WorkspaceAdmission::Refused(OwnerElsewhere)` (a live owner holds the lock): nothing admitted, nothing attached for writing; the session advertises no write capability and refuses `fs/write_text_file`; its bridge attaches to whoever owns the root | `EngineClient::admit_or_attach_config` after `WouldBlock`; `admit_workspace` | `application/workflow_client/client.tests.rs::a_second_cockpit_attaches_to_the_running_host_and_acts_through_it`, `::a_held_host_without_an_endpoint_fails_closed`, `::an_attached_port_reports_a_gone_owner_and_opens_nothing`, `tests/workflow_consistency.rs::an_attached_command_is_current_on_the_owner_and_stale_on_the_other_role_until_it_reloads`, `tests/source_pipeline_routing.rs::a_held_workspace_refuses_the_second_hosts_writes_and_invents_no_receipt`, `tests/effect_capabilities.rs::a_held_workspace_refuses_the_write_without_asking_and_the_read_goes_on` |
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
implementation was not counted as a production caller.

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
| CLI foundation, model and persistence aliases; core/engine domain and workflow value reexports | Refer to owning definitions used by live callers and compatibility fixtures; they introduce no independent state or implementation. Retain. |
| `agent::engine`, its root alias, local/channel/remote clients, reconnect and transport task | No caller outside their own tests. Removed together with the unserved `magistr-protocol` crate and its stdio/Unix/TCP/WebSocket transports. |
| Engine fleet/batch modules and reexports; CLI `agent::multi` constructors and aliases | Only simulated integration tests and internal callers. Removed; no distributed execution is advertised. |
| Engine `peer` | Preserve the tested bounded agreement/replay rules as an opt-in experiment. Production dependency resolution must not enable `experimental-peer`. |

The application dependency test checks that the retired wire crate is absent
and the peer feature is disabled in production resolution. The workspace port
tests exercise refusal before process launch and journal reads through the owner.
Existing session, checkpoint, admission and canonical command tests cover the
retained entrances. Owned subsystem licenses, provenance and compatibility
inputs remain in `subsystems/`.

## Duplicate ownership is refused

- **A second writer of a role.** `ProjectAuthority::admit` takes the role's
  exclusive OS lock before any engine opens; a second admission of the same
  role — in another process or in the same one — is `WouldBlock` with the
  role's own words, and a clone keeps the lease while a reader never holds
  it. Evidence: `magistr-engine` `native/project_authority.tests.rs::one_writer_per_role_is_admitted_and_the_roles_coexist`,
  `tests/workflow_consistency.rs::a_second_writer_of_a_role_is_refused_in_process_and_the_lease_ends_with_the_last_clone`,
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

## Owner commands

The typed vocabulary an owner endpoint serves is `OwnerCommand`: every
`NativeEngines` verb (sixteen roadmap, three think, seven ship, five
signal commands), carried out by `NativeEngines::apply` on the owner's own
writer. The inventory has exactly one producer of typed commands: the
attached cockpit's `AttachedEnginePort`, which sends `StartChunk`,
`CompleteChunk`, the three proposal resolutions, `ClearBlockedBy`,
`PinStep`, `Checkpoint`, `Promote`, `Ignore` and `Snooze` — every verb the
cockpit's `EnginePort` has. The `mcp-serve` proxy and the workspace bridge
relay MCP frames and send no typed command. No consumer needs a command
that does not exist, so none was added; the retired
[typed attachment on the Service owner](project-authority.md#typed-attachment-on-the-service-owner-retired)
stays retired. A future one-shot writer attaches through the Host's
existing `OwnerEndpoint::serve` path and uses the verbs above.

## Workflow consistency model

The vocabulary is the client-centric one from
[Jepsen's consistency models](https://jepsen.io/consistency/models):
*read-your-writes* (a process sees its own earlier write) and *monotonic
reads* (a process never sees an older state after a newer one). Magistr's
workflow store gives read-your-writes and monotonic reads per family
across every role and reader on one machine: each family's engine
observes its file before it answers or acts, as stated below. What it does
not give is atomicity across the four families, and a write racing inside
one family's read-to-save window resolves by that family's merge rule.

### The store and its merge

Each family persists one JSON file per project under the store's data root
(`roadmap/sessions`, `ship/sessions`, `think/sessions`, `signal/…`),
written atomically (temp file, rename). Every save takes the exclusive lock
on the sibling `.lock` file, re-reads the file, merges it into the saving
process's memory with the family's policy, and writes the result — so a
process holding stale memory never erases what another process already
acknowledged. The policies:

| Family | Merge on save | Conflict on one record |
| --- | --- | --- |
| roadmap | chunks union by id; refresh notes, note sections, tracker links union; the preamble keeps memory unless empty | the copy with the newest `updated_at` wins whole |
| ship | a newer cycle (`objective.created_at`) wins outright; within one cycle tasks union by id and `next_action_id` takes the max | memory wins per shared task (last writer wins) |
| think | steps union by number or by content identity, so a renumbered trace is recognized rather than duplicated | none: steps are append-only |
| signal | signals union by id | the furthest lifecycle rank wins (`Promoted` outranks `Dismissed`); equal rank is last writer wins |

### Read-through: the store stamp

Each persisted family's engine remembers the *store stamp* of the file it
last read or wrote — the inode, modification time and length, as
`StoreStamp` in `magistr-workflow` `infra/store_stamp.rs` defines it.
Before any query or mutation the engine `stat`s its file and compares:
an unchanged stamp answers memory, a moved stamp re-reads the file whole
and replaces memory (every mutation persists at once, so the file is the
merged truth of everything acknowledged; a missing file is the empty
family). A save hands the merged file back to the writer's memory together
with the stamp taken under the lock after the rename, so an own write
leaves memory equal to disk and the next access is a `stat`. The stamp is
per family and orders nothing — no revision is built over the four files.
Every Magistr writer renames a fresh temporary over the store, so a write
always moves the inode; the known gap is an editor rewriting the file in
place inside one timestamp tick at the same length, which is the direct
edit the audit already excludes.

| Family | Mutation | Query | Explicit reload |
| --- | --- | --- | --- |
| roadmap | observes the store (`refresh_if_stale` under the use-case lock), validates the transition against the current record, merges on save and adopts the merged file | observes the store, then answers memory (`NativeReader` queries, `roadmap_status`, the MCP resources) | `NativeReader::reload_roadmap` — the same re-read, forced |
| ship | observes the store before every use case (the former unconditional per-verb re-read, now the stamp gate); check settlement compares identity and appends under the file lock and adopts the written stamp | observes the store (`ship_status`, `ship_tasks`, `ship_active_task_id`, `ship_objective`) | `reload_ship` |
| think | appends; the archive on disk is the record | `think_steps_from_store` reads the archive directly; the pinned view and `think_step` read the live trace | none needed for the archive; the live trace answers what this process recorded |
| signal | observes the store, validates against the current record, merges on save and adopts the merged file; promotion observes the roadmap before it reads or creates the chunk | observes the store (`signal_list`, `signal_status`, `signal`; the pending count on `roadmap_status`) | `reload_signal` |

The published `NativeSnapshot` observes each of the three families under
its own lock before reading it, so a snapshot is one consistent state per
family, never across them. A store that cannot be stated or read refuses
the mutation or the use-case query (`RoadmapError::Store`,
`SignalError::Store`, the ship use case's error) and is logged on a
`NativeReader` query, which answers memory — a query has no refusal to give.
An explicit reload is the engine's own re-read: the wiring (broadcaster,
mirror, cloud, receipts) stays and no load banner prints.
`tests/workflow_consistency.rs`, `magistr-workflow`
`tests/store_freshness.rs` and `magistr-engine`
`native/native_engines.tests.rs::two_writers_merge_without_data_loss`,
`::reload_picks_up_another_writers_changes`, `::reload_signal_picks_up_another_writers_capture`
and `::think_steps_from_store_sees_another_writers_steps_without_reopening`
are the executable statements of this table.

### Conflicting transitions

A transition validates against the record as the store holds it at the
moment of the call, so two roles acting in turn see each other's outcome:
the Host cannot start a chunk the Service just obsoleted, and a second
promotion of a signal another role promoted creates nothing. Two writes
that both land inside one family's window between a read and its save —
a stat, a mutation, a locked merge-write — are not serialized; the
family's merge rule decides the record and the loser observes the winner
on its next access:

| Family | Both roles wrote one record in the window | Outcome |
| --- | --- | --- |
| roadmap | any two mutations of one chunk | the copy with the newest `updated_at` wins whole, the other mutation is superseded (`store_freshness.rs::a_conflicting_roadmap_transition_resolves_by_the_newest_record_and_the_loser_observes_it`) |
| signal | any two mutations of one signal | the further lifecycle rank wins, then the larger enrichment count, then the last writer (`::a_conflicting_signal_write_resolves_by_lifecycle_rank_not_by_who_wrote_last`) |
| ship | two writes in one cycle | tasks union by id, memory wins per shared task, `next_action_id` takes the max; a newer cycle on disk wins the file and the stale writer re-reads it on its next verb (`::a_ship_engine_re_reads_only_when_the_store_stamp_moved`) |
| think | two appends | steps union by number or content identity; nothing is superseded |

No timestamp, generation or source revision is promoted into a global
revision to arbitrate these: the rule is the family's, on the record.

### What attachment and the roles make coherent

| Pair | Coherent | Not coherent |
| --- | --- | --- |
| One process: a writer and the readers derived from it (`NativeEngines::reader`, the cockpit loop's `EngineClient::attach`) | read-your-writes and monotonic reads: the same engines behind one lock | — |
| Owner and an attached client (a second `magistr run`) | the command runs on the owner's engines, so the owner's own views are current at once and the reply carries the record; the client's own `NativeReader` observes the store on its next query, so it reads its write back without the feed; a stale generation is refused until the client reconnects | the feed and the re-seed timer decide when the client *repaints*, not what it reads: a view painted from an earlier snapshot shows that snapshot until the next seed |
| Host and Service writers (the cockpit or a loop, and the agent's `mcp-serve`) | nothing acknowledged is lost (merge on save); every family's queries and verbs observe the store, so each role reads the other's acknowledged writes and validates its transitions against them; a settled check lands on the task instance it was admitted for or settles `Stale`; the Service's feed carries its frames to a cockpit that holds the socket | one record written by both roles inside one family's read-to-save window resolves by the family's rule above, not by refusal; the four families are four files with four locks, so a query across them can see one family's write and not another's; the owner generation fences attached clients of a role, not the other role |
| Readers (`magistr roadmap status`, the observer cockpit, the guard) | every query observes the merged file; a reload is the same read, forced | the cockpit re-seeds only when the store cursor moved past the snapshot it was seeded with and stands down while the feed holds the socket — a snapshot the feed asked for over an unchanged store is confirmed, not re-read, and that confirmation is what returns a projection marked stale by a hangup, a gap or a new owner to current; the loop reloads the roadmap and re-seeds signals at each iteration boundary — both keep their cost, since an unchanged store is a `stat` |

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
them re-read. The consistency tables describe one machine, one user and
Unix sockets; off Unix there is no attachment and no endpoint, and the
stamp carries no inode.
