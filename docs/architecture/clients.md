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
| `magistr run` (cockpit) | `NativeEngines`, `Host` role, serving its `OwnerEndpoint` for the run | `HostedWorkspace::admit`: the checkout's owner for the run, endpoint served, `WorkspaceSourceWriter` for the session's authorized writes; the cockpit's `McpWorkbench` port spawns the `.mcp.json` bridge, which attaches to this host; the Changes tab reads `magistr workspace journal` | `magistr-cockpit` `engine/client.rs` `EngineClient::admit_or_attach_config` on `writer_config` (refusing a config without evidence), then `commands/run/cmd_run_cockpit.rs` `admit_workspace` | `cmd_run_cockpit.tests.rs::the_engines_and_the_agent_come_before_the_terminal_and_their_failures_are_printed`, `magistr-cockpit` `engine/client.tests.rs::a_second_host_on_the_same_project_is_refused_until_the_first_is_gone`, `::a_writer_config_without_evidence_is_refused_before_the_lease`, `tests/source_pipeline_routing.rs::an_authorized_write_lands_through_the_pipeline_with_a_journal_intent_and_a_receipt` |
| A second `magistr run` on a project whose Host lease is held | attached `OwnerClient` (`commands`) behind `AttachedEnginePort`: every cockpit verb is carried out by the running host's writer; reads come from the cockpit's own `NativeReader`, the feed and the re-seed timer | `HostedWorkspace::admit` answers `None` (a live owner holds the lock): nothing admitted, nothing attached for writing; the session's writes go direct and say so; its bridge attaches to whoever owns the root | `EngineClient::admit_or_attach_config` after `WouldBlock`; `admit_workspace` | `engine/client.tests.rs::a_second_cockpit_attaches_to_the_running_host_and_acts_through_it`, `::a_held_host_without_an_endpoint_fails_closed`, `::an_attached_port_reports_a_gone_owner_and_opens_nothing`, `tests/workflow_consistency.rs::an_attached_command_is_current_on_the_owner_and_stale_on_the_other_role_until_it_reloads`, `tests/source_pipeline_routing.rs::without_an_admitted_workspace_the_effect_writes_directly_and_says_so` |
| `magistr next` and `magistr next-loop` in the cockpit | `NativeEngines`, `Host` role, handed to `IterationLoop`; the cockpit attaches to that writer in-process (`EngineClient::attach`), never a second one; every gate through `AdmittedCheck`; the judge completes through the same writer | `HostedWorkspace::admit` per loop run in `run_cockpit_loop`, so the agent's bridge and the cockpit's port attach to the loop; gates observe through `WorkspaceEvidence` | `orchestration/roadmap/admit_native_host.rs` from `cmd_next_cockpit` / `cmd_next_loop_cockpit` | `commands/next/run_cockpit_loop.tests.rs::the_loops_admitted_writer_is_attached_before_the_terminal_and_never_re_admitted`, `orchestration/loop_driver/driver.tests.rs::every_gate_the_loop_runs_reaches_the_trace_as_a_verified_check`, `::a_judged_completion_names_the_task_its_gates_were_recorded_on`, `tests/application_composition.rs` |
| `magistr next-loop` plain and `--headless` | `NativeEngines`, `Host` role, via `admit_native_host`; a loop opened without a writer admits the host itself (`magistr-core` `workflow/iteration.rs` `auto_native`) | no hosted workspace: the agent's writes go direct (backlog `source-route-headless-hosts`); gates observe through `WorkspaceEvidence` | `cmd_next_loop.rs`, `cmd_next_loop_headless.rs` → `IterationLoop::open_or_recover` | `commands/next/cmd_next_loop_headless.tests.rs`, `orchestration/roadmap/open_native_reader.tests.rs::native_project_admits_one_host_and_readers_on_the_configured_store`, `native-write-path` gate rules 8 and 10 |
| `magistr next` plain, `--headless` and `--continue` (one iteration) | `NativeReader` for the progress summary (`WorkflowConfig::engines`); the writer is the agent's own `magistr mcp-serve` (first row) | none in this process; the agent's bridge attaches or admits | `magistr-core` `workflow/mod.rs` `WorkflowConfig::engines` → `NativeReader::open` | `commands/next/cmd_next_headless.tests.rs::workflow_session_uses_selected_project_for_execution_and_context`, `tests/next_iteration_integration.rs::headless_iteration_is_guarded_on_ship_state_and_cross_referenced` |
| `magistr run --headless` and the plain workflow session (`cmd_run_workflow`) | none in this process; the agent's `magistr mcp-serve` writes | none; ACP writes are direct and stamped `route: direct` | `commands/run/cmd_run_headless.rs`, `cmd_run_workflow.rs` | `commands/run/cmd_run_headless.tests.rs`, `magistr-acp` `capabilities/local_effect_port.tests.rs` (the direct route says so) |
| Bare `magistr`, `magistr roadmap` (observer cockpit), the Home launcher | `NativeReader` via `EngineClient::observe`; the feed when the Service publishes, the re-seed timer otherwise; no port | none | `commands/cockpit.rs` | `engine/client.tests.rs::a_second_cockpit_attaches_to_the_running_host_and_acts_through_it` (the observer beside the host, serving no endpoint), `tests/mcp_project_owner.rs::a_reader_command_answers_beside_the_running_owner_without_a_lease` |
| `magistr roadmap show`, `next`, `status`, `export`; `magistr guard` (the stop hook) | `NativeReader` via `open_native_reader` | none | `commands/misc/cmd_roadmap.rs`, `commands/config/cmd_guard.rs` | `tests/mcp_project_owner.rs::a_reader_command_answers_beside_the_running_owner_without_a_lease`, `tests/roadmap_export_policy.rs`, `commands/config/cmd_guard.tests.rs::apply_engine_state_reads_the_ship_and_roadmap_documents` |
| `magistr validate`, `magistr next --gate-only` | none: `ProjectValidation` runs the gate commands and records nothing, so its run is a report, never evidence a chunk can complete on | none | `magistr-core` `validate.rs` | `tests/validation_project_gates.rs::validation_modes_enforce_required_custom_and_archetype_gates` |
| ACP `fs/write_text_file` inside a host session | none | `WorkspaceSourceWriter` over the hosted service's `write` verb after the session's permission decision (journal intent, `MutationReceipt`, event `route: workspace`); without a hosted workspace direct host I/O and `route: direct` | `magistr-acp` `capabilities/local_effect_port.rs` with `AcpTransport::source_writer` | `tests/source_pipeline_routing.rs` (all five), `magistr-acp` `capabilities/local_effect_port.tests.rs` |
| ACP `fs/read_text_file`, `terminal/*` | none | direct host I/O after authorization | `magistr-acp` `handlers/fs.rs`, `handlers/terminal.rs` | `tests/effect_capabilities.rs::direct_reads_and_approved_writes_use_the_shared_profile`, `::a_remembered_process_refusal_prevents_direct_terminal_spawn` |
| `magistr mcp-launch workbench-mcp <root>` and `magistr workspace serve <root>` — the bridge every `.mcp.json`, Codex `config.toml` and the cockpit's port launch | none | the admitted `WorkspaceService` when the root is free, else an attached bridge (`OwnerClient::attach_workspace`) within the five-second window; explicit `--config` refuses attachment; reconnects lazily on owner loss | `workspace_mcp/connection.rs` `WorkspaceConnection::open` | `tests/workspace_attachment.rs` (sixteen scenarios), `tests/workspace_served.rs::status_is_byte_identical_across_fresh_owners_and_a_second_owner_is_refused`, `tests/workspace_served_stdio.rs`, `tests/mcp_launch_integration.rs::relocated_project_is_served_natively_from_a_nested_directory` |
| `magistr codex …` (the Codex wrapper) and Codex's parent and subagent threads | none in the wrapper; the threads launch the first row and the bridge from `.codex/config.toml` | as those rows | `codex_launch.rs`; `scaffold/codex_config.rs` registers only `magistr` and `workbench-mcp` (`BUILTIN_MCP_SERVERS`) | `tests/codex_runtime.rs::native_codex_launch_preserves_argv_stdio_environment_and_exit_status`, `scripts/codex-subagent-check.py` (the optional `codex-subagent` gate; passed 2026-09-12) |
| `magistr workspace journal`, `snapshot`, `doctor` | none | lock-free readers of committed state: `JournalInspection::rows`, `SourceSnapshot::capture`, `DoctorRun`; no lock taken, so they answer beside a live owner | `workspace_cli/journal.rs`, `snapshot.rs`, `doctor.rs` | `tests/workspace_cli_verbs.rs::journal_and_doctor_run_beside_a_live_owner_without_taking_or_disturbing_the_lock`, `::snapshot_prints_the_source_observation_beside_a_live_owner_and_refuses_bad_inputs` |
| `magistr workspace init`, `key rotate`, `migrate` | none | none opens a service: `init` seeds `workbench.toml`, `key rotate` rewrites a served workspace's key store, `migrate` moves a dead legacy lock record aside | `workspace_cli/init.rs`, `key.rs`, `migrate.rs` | `tests/workspace_cli_verbs.rs::init_seeds_the_commands_table_once_and_never_touches_an_existing_one`, `::key_rotate_needs_a_served_workspace_and_then_replaces_the_key`, `tests/workspace_served_stdio.rs::legacy_lock_refusal_preserves_the_record_and_never_opens_workspace_stores` |
| `magistr --workspace-worker` | none | none: a private child of one owner with no lock and no service; ends with its parent | `workspace_mcp` worker supervision | `tests/workspace_attachment.rs::a_killed_owner_takes_its_worker_with_it`, `::an_orphaned_worker_whose_stdin_stays_open_ends_by_itself` |
| The loop's judge sessions | the loop's own writer, in-process (no capability of their own) | none | `orchestration/loop_driver/driver.rs` | `driver.tests.rs::an_ok_verdict_over_green_gates_completes_the_chunk_through_the_engines` |

Not clients: `magistr acp-agent` (a provider adapter whose tools arrive
through the first row and the bridge, launched by the provider CLI); the
`magistr-tui` cockpit example (an observer, not shipped). The standalone
`think-and-ship` binary and CLI were deleted by `arch-14f4-rehome-naming`;
the `think-and-ship` name in `scaffold/codex_config.rs` only recognizes an
entry a user wrote before the cutover. There is no row for a daemon, a remote
endpoint, a Bun or NAPI path, or an internal MCP round trip.

## Duplicate ownership is refused

- **A second writer of a role.** `ProjectAuthority::admit` takes the role's
  exclusive OS lock before any engine opens; a second admission of the same
  role — in another process or in the same one — is `WouldBlock` with the
  role's own words, and a clone keeps the lease while a reader never holds
  it. Evidence: `magistr-engine` `native/project_authority.tests.rs::one_writer_per_role_is_admitted_and_the_roles_coexist`,
  `tests/workflow_consistency.rs::a_second_writer_of_a_role_is_refused_in_process_and_the_lease_ends_with_the_last_clone`,
  `tests/mcp_service_owner.rs`, `engine/client.tests.rs::a_second_host_on_the_same_project_is_refused_until_the_first_is_gone`.
- **A second workspace owner.** `WorkspaceService::open` holds
  `.workbench/server.lock`; `Locked` makes the bridge attach and makes the
  host admit nothing — neither opens a second service, in-process or
  across processes. Evidence: `tests/workspace_served.rs::status_is_byte_identical_across_fresh_owners_and_a_second_owner_is_refused`,
  `tests/workflow_consistency.rs::a_second_hosted_workspace_on_one_root_admits_nothing_while_the_first_lives`,
  `tests/source_pipeline_routing.rs::without_an_admitted_workspace_the_effect_writes_directly_and_says_so`,
  `tests/workspace_attachment.rs::eight_simultaneous_clients_admit_exactly_one_owner_and_all_receive_the_tool_surface`.
- **No new path.** The `native-write-path` gate pins the inventory: rule 9
  allows `WorkspaceService::open` only in `workspace_mcp/connection.rs` and
  `application/hosted_workspace.rs`; rule 10 allows
  `ProjectAuthority::admit` only in `mcp_tools/service.rs`,
  `orchestration/roadmap/admit_native_host.rs`, `magistr-core`
  `workflow/iteration.rs` and `magistr-cockpit` `engine/client.rs`; rule 11
  allows `OwnerClient::attach*` only in the cockpit client, the `mcp-serve`
  proxy and the workspace bridge. A production file that opens, admits or
  attaches anywhere else fails the required gate with its line.

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
workflow store gives read-your-writes inside one process and none of the
cross-process guarantees except where a use case reads through, as stated
below.

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

### Read-through and cached reads

| Family | Mutation | Query | Cross-process refresh |
| --- | --- | --- | --- |
| roadmap | merges on save; operates on the cached roadmap | cached | `NativeReader::reload_roadmap` |
| ship | every use case re-reads the persisted cycle under the engine lock before operating (`refresh_current`); check settlement compares identity and appends under the file lock | cached (`ship_status`, `ship_tasks`, `ship_active_task_id`) | `reload_ship` |
| think | appends; the archive on disk is the record | `think_steps_from_store` reads the archive directly; the pinned view and `think_step` read the live trace | none needed for the archive; the live trace answers what this process recorded |
| signal | merges on save; operates on the cached inbox | cached | `reload_signal` |

The distinction that matters: a ship *verb* from a stale writer acts on the
current cycle — the Service records a gate on a task the Host started
without either reloading, and the Host's completion of that task keeps the
Service's check — while a ship *query* on the other role answers its cache
until it reloads. A roadmap or signal verb from a stale writer neither sees
nor erases the other role's record: the merge keeps both on disk, and the
stale process still answers its cache, even after its own write.
`tests/workflow_consistency.rs` and `magistr-engine`
`native/native_engines.tests.rs::two_writers_merge_without_data_loss`,
`::reload_picks_up_another_writers_changes`, `::reload_signal_picks_up_another_writers_capture`
and `::think_steps_from_store_sees_another_writers_steps_without_reopening`
are the executable statements of this table.

### What attachment and the roles make coherent

| Pair | Coherent | Not coherent |
| --- | --- | --- |
| One process: a writer and the readers derived from it (`NativeEngines::reader`, the cockpit loop's `EngineClient::attach`) | read-your-writes and monotonic reads: the same engines behind one lock | — |
| Owner and an attached client (a second `magistr run`) | the command runs on the owner's cached engines, so the owner's own views are current at once and the reply carries the record; a stale generation is refused until the client reconnects | the client's reads are its own `NativeReader`, current only through the feed, the re-seed timer or a reload; the client gets read-your-writes only from the reply, not from its next query |
| Host and Service writers (the cockpit or a loop, and the agent's `mcp-serve`) | nothing acknowledged is lost (merge on save); ship verbs read through; a settled check lands on the task instance it was admitted for or settles `Stale`; the Service's feed carries its frames to a cockpit that holds the socket | roadmap and signal queries — and every ship query — answer each role's cache until it reloads; there are no monotonic reads across roles without a reload; one record written by both roles between a refresh and a save resolves by the family's conflict rule (recency, rank or last writer), not by refusal; the owner generation fences attached clients of a role, not the other role |
| Readers (`magistr roadmap status`, the observer cockpit, the guard) | a fresh open or a reload reads the merged file | a reader opened earlier answers what it opened on until it reloads; the cockpit reloads only when `store_stamp` moved and stands down while the feed holds the socket; the loop reloads the roadmap and re-seeds signals at each iteration boundary |

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
a store file or a deleted lock file are outside admission. Cached reads
across roles are a documented limit, not a defect the audit fixed;
`workflow-read-through-queries` in the backlog names the follow-up.
[Projections](projections.md) made what a client reads typed and
freshness-aware; they did not change when a cached query reloads. The consistency tables describe
one machine, one user and Unix sockets; off Unix there is no attachment and
no endpoint.
