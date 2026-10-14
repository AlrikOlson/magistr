# Project authority

Every native workflow writer is admitted by `ProjectAuthority` before opening
state. One project has one workflow owner across interactive hosts and MCP
clients. Plain and headless loops read progress through NativeReader; their
agent's tools use the project owner. Their separate loop lease protects the
checkpoint, not workflow state. `WriterRole` describes the host; it grants no separate
lease. A client attaches to that owner or receives an explicit refusal.

The independent [workspace owner](workspace-owner.md) admits source tools per
canonical working-copy root. Its generation, journal sessions and approvals
remain separate from workflow runs and workflow owner generations.
The [configuration boundary](configuration.md) captures store, project and
execution inputs once.

## Capabilities

| Capability | Admission and lifetime |
| --- | --- |
| `NativeReader` | Lease-free queries over persisted records, scoped to its configured execution. |
| `NativeEngines` | Constructed by `ProjectAuthority::open` or `serve`. Clones retain the project lease. `for_working_copy` binds a view through that writer's execution registry. |
| `UnifiedService` | The same family use cases exposed through MCP; the CLI retains its admitted writer beside them. |
| `OwnerClient` | A welcomed local connection; no independent engines or lease. Typed commands carry the admitted owner generation. |

## One write path

Every supported native mutation runs the first-party workflow application use
case for its family. `ProjectAuthority::open` composes `ThinkUseCases`,
`ShipUseCases`, `RoadmapUseCases`, and `SignalUseCases` once, including the
reciprocal roadmap/signal capabilities. NativeEngines calls their synchronous,
typed methods directly. `ProjectAuthority::serve` wraps those same capabilities
in thin MCP adapters; an attached local client reaches them through the
versioned owner command adapter.

Validation and cross-family sequencing live below transports. Promoting a
signal creates and cross-links its roadmap chunk; empty bodies, blank research
summaries, illegal transitions, and unknown identities receive the same native
refusals. JSON serialization belongs only to transport presentation. The old
`handler_call` bridge, internal MCP argument/result decoding, and one-poll
noop-waker invocation are deleted.

Actual effects are explicit async operations. Ship checks admit a target before
running a command, execute outside the workflow lock, then settle against that
admission. Tracker setup probes its provider without holding the roadmap lock,
then applies the shared native setup policy. Approval waits similarly use the
application capability. CLI and MCP presentation retain their external names
and ordinary success/error envelopes.

The `native-write-path` gate keeps these boundaries executable: Magistr engine
composition is confined to `native`; raw engine reads belong to `NativeReader`;
native writes and workflow application modules cannot depend on MCP handlers,
wire argument/result types, JSON result decoding, or noop-waker polling.
Magistr MCP code cannot call its own tools. Direct host dependencies remain
limited to `magistr-engine` and `magistr-cli`.

### Identities and check settlement

Each admitted workflow execution carries an objective run and task instances
within that run. New runs and task instances receive fresh identities.
Duplicate plan task IDs are refused. Removed instances are fenced so a stale
merge cannot resurrect them.

`TaskAdmission` names the execution binding, project, run, task ID, and task instance.
`CheckAdmission` binds that target to the check request before an effect
begins — together with the durable receipt it was admitted under, the
acceptance revision then current, and the source the workflow observed at
that moment. Native `ship_check` retains this admission across command
execution inside the workflow crate; `magistr_gate` and the loop driver
retain it through the [composition boundary](application.md#the-composition-boundary)'s
`AdmittedCheck`. The loop additionally fixes one task admission for the
complete gate set, including retries. Reusing a task name in a later
objective cannot collect an earlier command's result.

### Receipts and the evidence capability

Every admitted check writes a durable `CheckReceipt` before its command
runs (`ship/executions/<context-key>/receipts/<project>/<id>.json` beside the session record, one
file per receipt, published by temp → fsync → rename → fsync-directory;
in memory when persistence is off). The receipt names the run, task
instance and per-gate attempt, the gate contract as requested (name, type,
command, required), the objective's acceptance revision (a content digest
of description, acceptance criteria, constraints and scope — an in-flight
`ship_set_objective` keeps the run id but moves the revision) and, when the
writer carries the evidence capability, a `SourceObservation`: which
workspace and owner generation, under which scheme and selection policy,
the content-addressed revision of the selected bytes, the digest of those
bytes alone, the digest of applicable policies and shared project settings
([configuration evidence](configuration.md#configuration-evidence)) and
what that observation may claim. Settlement rewrites the same receipt once with how
the command ended (`CheckExecution`: exited with a code, timed out,
cancelled, launch failed, or self-reported), the timing, whether the source
still matched (`SourceFreshness`) and how the workflow settled it.

The evidence capability is the workflow's `EvidencePort`: `observe()`
answers a `SourceWitness` — held in the admitting process until the check
settles, consumed once to validate. Each admission captures anew; re-running
a named gate allocates a new receipt and attempt over the current selected
bytes. Direct workflow commands use that observation's workspace as their
working directory and resolve relative reports there. `required_gates()` answers the
`GateContract`s the project requires now. Magistr composes exactly one
implementation, `magistr_core::evidence::WorkspaceEvidence`, from the
workspace's lock-free `SourceSnapshot` under its default selection policy
(which hashes `.magistr/project.json` as a config input), the workspace's
`ConfigDigest` over the external layers the captured project inputs name and
the shared project settings,
and the same gate resolver `magistr_gate all` runs (shared as
`WorkspaceEvidence::gate_contracts` with a loop that runs without a
writer). `writer_config` attaches it to the
`NativeConfig` every production writer is admitted on — the MCP service and
cockpit host — and the `native-write-path` gate (rule 8)
refuses a production admission without it; the MCP service and the cockpit
refuse a writer config that lacks it before taking the lease. Every observation and every
command runs outside the engine lock, the ledger lock and any workspace
mutation lock: admission holds the engine lock only to name the target,
observes, then publishes; settlement validates the witness first and takes
the lock only to append the check and rewrite the receipt.

Settlement re-reads the current native ship state and compares identity under
the existing persistence lock before appending an ordinary check. A stale
settlement — the run or task instance superseded while the command ran —
returns its admission and execution outcome intact and settles the receipt
`Stale` with the reason under its original identity, so a late result after
an objective rollover keeps its record and never reaches the task that reused
the name; the MCP adapter reports `stale_result` with the receipt, and host
runners report an unrecorded result. An observation that cannot be taken
refuses the admission before anything runs: an unobservable gate is never
recorded as evidence.

### Receipt retention

Receipts remain available across objective changes until explicitly compacted.
`ship_compact_receipts` (also `NativeEngines::ship_compact_receipts` and the
attached owner's `ShipCommand::CompactReceipts`) permanently removes only
settled receipts outside the persisted current run. It keeps every current-run
attempt, every receipt with incomplete settlement, and every exact receipt ID
referenced by an unresolved loop operation. The report counts these disjoint
groups as `retained_current`, `retained_unsettled`, and `retained_referenced`,
alongside `removed`. There is no age threshold, automatic pruning, or archive.
A missing current objective refuses compaction.

The canonical payload stays `ship/executions/<context-key>/receipts/<project>/<id>.json`. A derived
`runs-v1/<sha256(run_id)>/<id>.ref` membership index makes completion and
attempt allocation read only that run's receipts; exact-ID reads and settlement
open just that payload. The derived index is built under the ledger lock
before publishing its ready marker. An interrupted build is retried.
Steady-state reads do not enumerate or decode lifetime receipt history; the
first index build and explicit compaction still do. Work within one long-lived
run grows with that run's receipts, all of which must be kept.

Membership and its directories are synced before an atomic payload publication.
Readers, admissions, settlements and compaction share the ledger lock, held only
for storage work. An interrupted admission may leave an empty membership with no
payload; it is not evidence and never advances an attempt. An acknowledged
receipt cannot be omitted from its run's index. The admitted project writer
owns this index and every contextual receipt partition.

Compaction holds the persisted cycle lock before the ledger lock, so an objective
rollover cannot change which run is protected during deletion. Under the ledger
lock, the host's `EvidencePort::retained_receipts` reads the typed
`.magistr/loop-state.json` checkpoint. The loop persists its `receipt:<id>`
reference before settling the receipt; this ordering prevents compaction from
missing the crash window between receipt settlement and operation settlement.
An unreadable or malformed checkpoint refuses compaction. A settled receipt is
eligible only after its referring operation is durably reconciled. Other hosts
with recovery references must implement this port method with the same ordering.

Compaction validates history before deleting any payload; corrupt history
refuses the pass. Each deletion is synced before removing its membership.
An I/O failure can leave a partially pruned history, but retained receipts remain
intact and the operation can be retried. Temporary files and diagnostic logs
never become receipts. Command output and parsed diagnostic reports remain on
ordinary session checks; they are not copied into the evidence ledger.

### Completion is governed by receipts

`roadmap_complete_chunk` is the one route to `done`, and every adapter lands
on it — the MCP tool, the owner command, the loop judge and the cockpit's
Complete action. The ship family is the roadmap's completion authority by
construction: `RoadmapUseCases::new` takes the `ShipUseCases` it completes
against, `FamilyServices` composes every writer's roadmap over its ship
family, and there is no permissive branch to compose instead
(`audit-260912-05`). Completion asks `ShipUseCases::verify_completion`
first: the current objective must be
scoped `chunk:<id>`; the verification task is the caller's `task:` reference
(a task of the current run) or, absent one, the instance the run's latest
receipt landed on, and the completion is stored with that reference so it
always names its task. For every gate contract the project requires, and for
every required verified receipt recorded on that instance, the latest receipt
must have settled `Applied`, passed, been run by a server, run the configured
command, seen the source and the external configuration unchanged during
its run, and certify the workspace, selection policy, source bytes,
external configuration and acceptance revision current now — each judged
on its own and named in the refusal; a new owner generation of the same
bytes is not a change
([configuration evidence](configuration.md#configuration-evidence)). Anything else is
`outstanding` with its reason, and the refusal lists every outstanding gate
and what re-running it would satisfy; earlier failures a later pass
superseded are reported as `recovered`. A self-reported pass neither
certifies a configured gate nor blocks. `roadmap_set_status(done)` and
accepting a `done` status proposal are refused toward `roadmap_complete_chunk`.
`ship_finalize`'s report separates `recovered_failures` from
`outstanding_failures`, and its failed-required warning names only the
outstanding ones.

Limits: an optimistic observation certifies agreement of two reads, never
isolation (`SourceExport` is the immutable path and is not consumed by
checks yet); a `Changed` freshness names paths only when the admitting
process settles the check, otherwise the source digests are compared
(`Diverged`) and a configuration drift is `Reconfigured` with no layer
named; receipt pruning follows the explicit retention policy above. Engines opened without the
capability (tests that never complete a chunk) record source-less checks
and are refused every completion with `NoEvidenceCapability`; the
`native-write-path` gate's rule 8 is supplementary — it keeps a production
writer from being admitted into that refusal rather than being the only
thing that prevents a permissive completion. A chunk born `done` (an import,
a tracker pull, a historical record) is a record, not a completion, and is
preserved.


## Roles

`Host` and `Service` describe how the owner was launched. Both contend on
`<data_dir>/roadmap/sessions/<project_id>.project-owner.lock`, or the
authority directory's `.magistr/project-owner.lock` when persistence is off.
An additional stable `.magistr/workflow-owner.lock` in the authority directory
prevents switching data directories from admitting two owners for one project.

## Working-copy identity

`ProjectLocation` resolves canonical source paths and Git's registered linked
worktree backlink and common directory. An ordinary project owns its
directory. A linked worktree uses the corresponding main working-copy project
directory for workflow identity and admission; its source directory remains
independent. Nested project paths preserve their relative directory.

Project-name overrides and pinned identity are resolved through the authority
directory. Copying a project pin cannot admit a second writer on its shared
store. A copied directory that is not a registered linked worktree cannot
attach as a working copy of the running project. Missing or malformed Git
links are refused at write admission. Linked worktrees whose common directory
does not identify a main working copy are explicitly unsupported.

`ExecutionContext` is a canonical working-copy root plus `MAGISTR_EXECUTION_ID`
(`default` when unspecified). Use a distinct execution id for independent work
in one directory. The authority owns a weak registry of live context services;
identical contexts reuse their engine handles, idle entries are pruned, and
admission refuses more than 128 live contexts. Every contextual view retains
an active working-copy lease.

Roadmap, reasoning and signals belong to the shared project. Ship's selected
scope, objectives, plans and receipts belong to the execution context, under
`ship/executions/<context-key>/sessions` and its receipt partition. Selecting
another context never replaces the first context's selected scope. Reopening
the same context restores its cycle. Its evidence capability observes that
context's source directory, the shared project's settings and the owner's
captured external policy layers. Settings load and write paths resolve through
`ProjectLocation` to the corresponding main project directory. Creating a copy
does not create another settings file; edits to the shared settings change
configuration evidence in every copy.

`ExecutionBinding` on task and check admissions names the root, execution id
and workflow owner generation. Settling an admission through a different
binding is refused before any check can be attached. Durable receipts also
retain workflow run and task-instance identity and the independent source
observation. Source editing remains useful without a workflow task: source
sessions and confirmation tokens are governed by the source owner, while
check receipts establish the relationship to an admitted workflow task.

## Working-copy lifecycle

The CLI's `application::WorkingCopies` is the application service for
create, list, open and archive. It uses `GitRepo` operations and Git's worktree
registry; presentation stores no parallel list.

```sh
magistr workspace working-copy --dir . create ../task-copy --branch task-copy
magistr workspace working-copy --dir . list
magistr workspace working-copy --dir . archive ../task-copy
magistr workspace working-copy --dir . open ../task-copy
```

Archive closes a clean, idle linked copy by locking its Git registry entry
with the reason `Archived by Magistr`. It preserves the directory, branch,
journal and execution history. Reopen unlocks that entry. External Git locks
retain their reason and are refused. The main copy cannot archive.
Dirty source changes, merge conflicts, active workflow or source owners, and
unavailable paths produce distinct outcomes. Unavailable entries remain listed;
nothing prunes or recreates them automatically. Owned runtime coordination and
journal files are excluded from source status; project configuration is included.

Archiving holds an exclusive lease at the physical Git working-copy root and
the workspace's existing `ServerLock`. Workflow views and source owners retain
shared leases there, including projects rooted in subdirectories.
The CLI supplies the lifecycle lock path to `WorkspaceService`, which retains
it until the source service closes. Both admissions recheck archive state after
taking their locks, preventing a new owner from racing an archive.

## Lock identity and generation

Stable lock inodes are never removed on release. OS-held locks are released
with the last writer or process exit. The counter at the shared project root mints a higher
`OwnerGeneration` on each successful admission, including a change of store location. This fences typed commands
and in-flight check settlement; it is neither a source revision nor a run id.

## Attachment

The project endpoint is derived from the shared project lock path, with a
short hashed runtime path when the local socket name is too long.
It serves typed commands and the host's MCP adapter through `serve_with`.
Workspace endpoints use the same local protocol with a separate root identity.

A hello names protocol version, build, project, caller and execution context.
Peer credentials must match the owner's user. Typed commands and probes
require the exact `BuildIdentity`; opaque MCP sessions use the owner's tool
definitions. Protocol mismatches are refused. The owner validates membership
and constructs the contextual view before welcoming a workflow client.
The MCP acceptor receives that admitted view, not an unbound service.

Connections are capped at 128, handshakes at five seconds, and native command
clients use deadlines. Endpoint shutdown stops admission, cancels sessions,
joins MCP cleanup and releases its writer references. A disconnect never
replays a possibly applied mutation. A surviving bridge can reconnect on its
next request and restore its own persisted execution.

## Event sockets

One project publisher supplies the shared sequence and owner epoch. A host can
start it after opening its engines; publication binds all existing execution
views before the host serves clients. Ship
events and Magistr phase/gate-output events also carry `execution_key`.
The CLI feed delivers those events only to the matching context while
advancing the cursor for every decoded frame. Shared roadmap, reasoning and
signal events reach every context. Snapshot readers use the same contextual
ship persistence partition. Long and short feed paths resolve through the
authority directory in every linked copy.

## Cached state and the store stamp

Each family observes its persisted file's inode, modification time and length
before answering or acting. Changed stamps trigger a read; unchanged stamps
reuse memory. Writes are atomic and adopt the written stamp. Contexts share
the project's planning engines and have their own ship handles. Queries across
families are not one transaction. Readers never grant mutation authority;
source revisions and owner generations remain distinct.

## Limits

Admission coordinates participating Magistr processes on one local machine.
Direct edits of native state and deliberate lock-file modification bypass it.
It is not distributed coordination. Git worktree operations also follow Git's
own refusal rules. This change introduces no desktop presentation or provider
session restoration; the application services are usable by current CLI and MCP
hosts and future desktop callers.

## Verification

Engine suites cover cross-role exclusion, copied identity and store contention,
canonical aliases, independent executions, foreign-root refusal, generation
fencing and endpoint cleanup. Workflow suites cover admission and receipt
settlement, persistence and source freshness. CLI lifecycle tests cover create,
archive, reopen, dirty/conflicted/active/unavailable copies and runtime files.

`mcp_project_owner::linked_worktrees_share_one_workflow_owner_and_keep_source_checks_and_recovery_separate`
uses real Git worktrees and real `magistr` processes. It proves one workflow
owner, distinct source edits, cross-tree approval refusal, correct check roots,
run/task binding, no foreign receipt delivery, source freshness, owner exit
and higher-generation recovery with bounded client cleanup.
`workflow_client::feed::tests` proves other executions cannot replace the
selected task or create false sequence gaps.
