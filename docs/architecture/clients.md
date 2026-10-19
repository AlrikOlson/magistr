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

| Client | Workflow capability | Source capability | Owning composition and evidence |
| --- | --- | --- | --- |
| Desktop window | ProjectResources admits or attaches to the project authority; sessions use execution-bound views | One admitted source resource per working copy | SessionRegistry and InteractiveHost; resident process and native window journeys |
| Plain and headless `magistr run` | The agent's connected MCP tools act through the project owner | WorkspaceAdmission, then the session's SourceRoute | AgentSession; source_pipeline_routing and effect_capabilities |
| `magistr next`, `magistr next-loop` | NativeReader for progress; the agent's MCP tools use the project owner. IterationLoop owns no workflow lease. | Admitted workspace shared across attempts | Streamed/headless command composition; next_loop_recovery and mcp_project_owner suites |
| Hosted work driver | Host supplies NativeEngines explicitly for mutations, judging and AdmittedCheck; IterationLoop receives its reader | The host's admitted source resource | HostedWork and loop_driver suites |
| Workflow MCP | One Service writer when admitted, otherwise attachment to the current authority | Lock-free evidence for gates | MagistrService and mcp_bridge; mcp_project_owner and mcp_service_owner |
| Source operations through `magistr mcp` | None | One WorkspaceService when admitted, otherwise an attached bridge | HostedWorkspace; workspace_attachment and workspace_cli_verbs |
| Workspace journal, snapshot and doctor | Read-only checkpoint inspection | Lock-free source readers | Workspace CLI; workspace_cli_verbs |
| Roadmap queries and guard | NativeReader | None | Native workflow reader; main/system-contract and command suites |

Provider, workspace worker and MCP children are dispatched by the CLI and desktop
executables before opening a window. Every control carries the admitted session
address. Session history is written after provider settlement; project discovery
combines settled history with current resident snapshots. The desktop creates no
parallel writer, catalog or provider registry.

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

The audit is of Magistr's current binaries. A direct edit of a store file or a
deleted lock file is outside admission. Read-through is per family: there is no
transaction across roadmap, execution, signal and reasoning owners. Each owning
mutation locks and validates current data before applying its change, then
acknowledges its committed source. Cross-family promotion explicitly reports a
partial commit and resumes by the existing record identity. [Projections](projections.md)
defines the typed read and mutation contracts, including uncertain writes.
Every read validates the selected file; equal metadata does not skip validation.
The consistency tables describe one machine and one user. Workflow command
and MCP attachment still require Unix sockets. Workspace attachment also has
an authenticated Windows named-pipe path. Off Unix the store stamp carries
no inode. See [the Claude adapter](claude-mod.md) for the difference between
a reachable source connection and an unavailable workflow connection.
