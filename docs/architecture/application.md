# Application contracts

`magistr-application` owns typed, project-scoped use cases and the narrow ports
their adapters implement. It is a `no_std` crate with `alloc` and a single
dependency on [the domain](domain.md). No provider, native workflow engine,
MCP request context, concrete executor, process launcher, filesystem or renderer
is imported into this boundary.

The crate was introduced by `arch-04-application-contract`. Its scoped use cases
and fake-port tests are contracts for adapter migration. `arch-06-effect-capabilities`
adds the first live host effect adapter: ACP delegates filesystem and terminal
requests through the [effect capability contract](effects.md). The live cockpit
uses `sessions::SessionHandle`, implemented by the same CLI supervisor as headless
execution. Project-wide writer admission is the engine's
[`ProjectAuthority`](project-authority.md): every native writer is admitted
there before its engines open, and reading needs no admission.

`arch-14f-native-application` reconciled this crate with the native workflow
use cases that now exist: the contract-only `workflow`, `checks` and
`projections` facades — start/complete a chunk, run a configured gate, read a
run projection — had no adapter and duplicated what the first-party family
use cases and `NativeEngines` already do under real admission, so they were
deleted together with the `Capability` variants only they named.
`audit-260912-14-config-inputs` retired the contract-only `configuration`
facade the same way: no adapter implemented its port, no native authority
holds the project-configuration revision it promised, and integrating it would
have required the speculative ownership layer the audit forbids. Project,
preset, provider, plugin and workspace inputs are instead
[captured once](configuration.md#project-inputs) in `magistr-core`. What
remains here is the contract layer live adapters implement: sessions,
permissions and effects, plus the scope, command and receipt types they
share. The place where workflow and workspace capabilities are composed is
the [composition boundary](#the-composition-boundary) in the entry-point crate.

## Use cases and effect owners

| Module | Application use cases | Adapter obligation |
| --- | --- | --- |
| `sessions` | Start or resume a session, submit a prompt, cancel a turn, close a session | Resolve provider capabilities and models, own live resources, maintain explicit provider bindings, observe terminal events |
| `permissions` | Resolve a scoped decision by selecting an offered option or cancelling | Correlate the live request, enforce project policy before approval, map the option to the provider response |
| `effects` | Read or write text files, create terminals, observe and release owned terminals | Resolve actual targets, authorize through the shared session policy, enforce cancellation and resource ownership, perform host I/O |

Each scoped facade borrows a `ProjectScope` and its own port. The public methods
name use cases rather than exposing a universal message dispatcher or repository.
Small command enums and request structures describe only the relevant family.
Native chunks, tasks and checks are not modeled here at all: their records,
transition rules and admission live in the native workflow use cases.

The port methods return `PortFuture`, an explicit boxed
[`core::future::Future`](https://doc.rust-lang.org/core/future/trait.Future.html).
This allows the host to select the executor. Rust's
[trait compatibility rules](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility)
allow these traits to be used through `dyn`; returning an opaque async type
would prevent that. The allocation is explicit and no async-trait macro or
runtime dependency is introduced.

The `effects` contract uses its own `EffectFuture`, with the same boxed future
shape and a `Send` bound for host task dispatch. `EffectPort` receives typed
requests and an `EffectAuthorizer`; neither imports ACP nor performs I/O in the
application crate. These requests retain provider session strings for the live
adapter's ownership checks. They do not claim the domain `AttemptId`, revision
admission or attempt correlation required by the scoped facades.

`SessionHandle` supplies explicit turn, permission, terminal, detach and shutdown
operations for an attached client and observes application-owned `AgentEvent`
values. The same `sessions` module owns startup `SessionOptions`, `SessionPolicy`,
permission requests and choices, and bounded tool records. These contracts use
only `alloc`, `core` and the domain crate; no ACP, runtime, channel, filesystem or
JSON library belongs here. Tool payloads distinguish text from encoded JSON, and
locations retain wire path text. Truncated JSON becomes text rather than claiming
to remain structured data.

The CLI translates ACP into this vocabulary. Policy requests retain the full
encoded input; the host decodes and normalizes it, refuses malformed input, and
keeps offered option IDs and reply channels. Tool-record display bounds never
truncate the input used to decide permission. Host admission errors remain
strings, and session-local permission IDs do not claim durable domain
attempt/revision correlation. The scoped `Sessions` facade remains a separate
contract for that later migration.

## Identity and admission

A `ProjectScope` is an identity validator, not a lease, authorization grant or
second workflow authority. The host must acquire authority — a
`ProjectAuthority` in its writer role — before constructing its concrete ports. Commands carry a `CommandContext`: an externally allocated
domain `AttemptId` and the expected project `Revision`. An attempt contains its
command, run and project identities.

Facades reject foreign projects and mismatched run targets before calling a port.
Session, turn and decision identities retain their domain parent scopes. Permission
decisions cannot be substituted for one another, even inside the same run.
A provider conversation binding is explicit and cannot silently become a Magistr
session identity.

The adapter must check the expected revision with the operation itself. The
facade cannot make a separate read and write atomic. A returned
`CommandReceipt` must identify the exact attempt and cannot predate its
expected revision. These checks reject misrouted acknowledgements; they do not
provide durable deduplication or make a dishonest adapter authoritative.

`Revision` is comparable only within the same project authority history; it is
neither the workflow owner generation nor a workspace generation nor a source
revision (see the [identity map](#identity-map)). No configuration revision
exists in this history: project settings are files the scaffold owns, read
through the captured project inputs, not a revisioned application aggregate.

## Outcomes and recovery

Session readiness, prompt acceptance, requested cancellation and observed session
closure are distinct outcomes. Accepting a prompt is not completing a turn.
Requesting cancellation is not observing termination or authorizing session reuse.
This separation matches the protocol distinction between submission and the
terminal response in the
[ACP prompt-turn lifecycle](https://agentclientprotocol.com/protocol/v1/prompt-turn).
The application does not import ACP types or equate a saved provider binding with
supported restoration; that capability belongs at the adapter.

`ApplicationError` preserves machine-readable recovery categories:

| Failure | Next kind of action |
| --- | --- |
| Invalid request or identity scope | Correct the request |
| Revision conflict | Refresh the state before a new admission decision |
| Capability denied | Review policy or configuration; a Deny does not become an override prompt |
| Capability unavailable | Select a supported capability |
| Persistence failure before a possibly applied effect | Restore persistence |
| Provider disconnected with known outcome | Reconnect the provider |
| Unknown outcome or invalid acknowledgement | Reconcile the operation's actual outcome |

Recovery actions are guidance, not automatic retries. If an effect may already
have happened, adapters report `UnknownOutcome` with its attempt identity.
A disconnect or write error must not erase that uncertainty. Dropping a pending
port future also does not prove cancellation. The loop's own operation
records, their retries and the atomic checkpoint are described in
[durable operations](durability.md); the scoped facades' `CommandContext`
still has no production adapter.

## The composition boundary

`magistr::application` (`crates/magistr-cli/src/application`) is the one place
where a native workflow use case and the workspace's source evidence are
composed for a consumer. It lives in the entry-point crate because every real
consumer of both capabilities is there, together with the pty gate runner,
gate resolution and project configuration; a crate with one consumer would be a
boundary with one side. The `native-write-path` gate (rule 7) keeps check
admission and settlement inside this module and the writer itself.

### Consumers

| Consumer | Workflow capability | Workspace capability | Through |
| --- | --- | --- | --- |
| `magistr_gate` in `magistr mcp-serve` | `NativeEngines` (Service role, the same writer the MCP session serves) | `WorkspaceEvidence` attached in `MagistrService::new` | `AdmittedCheck::admit` before the pty run, `settle` with the run's `CheckExecution` after; the answer carries the receipt id, source revision and freshness |
| Hosted loop driver gates | `NativeEngines` (Host role via `writer_config`), one admitted verification task for the set; the set is `required_gates()` of the same capability | the same capability on the host's config | `AdmittedCheck` per gate, refusing a superseded verification target |
| Native `ship_check` (agent) | `ShipUseCases::admit_check` / `settle_check` inside the workflow crate | the workflow's composed `EvidencePort` | the workflow's own async runner; the stale reply carries the receipt |
| `roadmap_complete_chunk` (MCP, owner command, cockpit Complete, loop judge) | `RoadmapUseCases` with the ship family as completion authority | `required_gates()` and a fresh observation at completion | `verify_completion` before the transition; refusals name every outstanding gate |
| Cockpit `run` and the cockpit loops | `NativeEngines` or an attached `OwnerClient` | the [`HostedWorkspace`](#the-hosted-workspace) this process admits for the run — the agent's bridge and the cockpit's port attach to it | engine port; `magistr workspace journal` for the journal model |
| ACP `fs/write_text_file` | none | the host's `SourceRoute`: `WorkspaceSourceWriter` over the hosted workspace's `write` verb after session authorization, or a `SourceRefusal` | `LocalEffectPort` routes the authorized write through the pipeline (journal intent, `MutationReceipt`) and stamps the `fs_write` event `route: workspace`; with a refusal it advertises no write capability and refuses the call before authorization (`route: refused`) — there is no direct write |
| ACP `fs/read_text_file`, `terminal/*` | none | direct host I/O after session authorization | `LocalEffectPort` |
| `magistr workspace snapshot` | none | lock-free `SourceSnapshot::capture` | CLI verb |

### The hosted workspace

`WorkspaceAdmission::admit(root)` is how every source-writing host — `magistr
run` on the cockpit, plain and headless, the cockpit, plain and headless loops,
`magistr roadmap generate` and the init interview — tries to become the
checkout's one workspace owner for the run. Admitted, it holds a
`HostedWorkspace`: it opens `WorkspaceService` on
the same open options the bridge composes (this binary as the worker, the host
environment's configuration layers, the embedded interpreter), serves the
existing owner endpoint so every `workbench-mcp` bridge the agent or the
cockpit spawns attaches to this process, and hands the session a
`WorkspaceSourceWriter` — one explicit journal session over the service's
`write` verb — as its `SourceRoute`. Refused, it holds a `SourceRefusal`:
another owner holds the lock (`OwnerElsewhere`), or the root could not be
admitted (`NotAdmitted`, in the owner's words: a marker that needs migration,
a configuration the policy refused, durable data from a newer binary). The
host then admits nothing — it never attaches to write through a socket and
never opens a second owner — and the session's source writes are refused
rather than written directly: the port advertises no write capability, so a
cooperating agent runs its own workspace tools (external effects, observed by
`SourceSnapshot`, never routed), and any write it asks for anyway is answered
with the refusal before any permission decision, with nothing on disk, no
journal row and no receipt. The reason is said once on the normal terminal.
The judge's sessions are composed with `NoAuthority` and refuse every write.
The `write` verb is the pipeline every other client
writes through — anchored on the current bytes, verified, journaled, atomic —
with formatting off (the session's bytes are the intent) and no confirm gate
(the session's permission decision is the confirmation). What the pipeline
refuses (a protected path, a syntax error into a valid source, a stale
anchor) comes back to the agent as a typed effect error in the pipeline's
words. Writes by an editor, a provider CLI or git stay external: no journal
row, no receipt, only `SourceSnapshot` validation naming the changed paths.
The workspace generation, the source revision and the mutation receipt's
revision remain three distinct facts.

### One process mechanism

Every command Magistr runs for a caller — a quality gate on its pty, a
workflow `ship_check` command, a workspace declared command — is a
`magistr_process::OwnedProcess`: its own session and process group, stdin
closed, both streams under a bounded head+tail capture, a timeout and a
cancel that end the whole group (gracefully with a `SIGTERM` grace, or at
once), every chunk to a live sink, and a kill on drop so an abandoned future
never leaves a running command behind. What is shared is the mechanism, not
the admission: the gate runner passes the configured gate line and the
workflow passes the caller's own command string as `Launch::Shell`; only the
workspace's `CommandAdmission` produces a `Launch::Argv`, and nothing turns
one into the other.

The spawn acquires a `ProcessGroup` guard and hard-exit registration together.
Normal completion also settles remaining group members, including descendants
that closed their pipes or outlived the leader. Graceful termination waits for
the group, then escalates to SIGKILL and allows two seconds for reaping and
group disappearance. Permission-denied probes retain ownership until a later
probe or the deadline; signal delivery alone never proves settlement.
A cleanup fault makes `ProcessOutcome::is_complete` false, and CLI command
results cannot report success or hide that fault behind cancellation.
Partial output, elapsed time and terminal outcome attribution remain available.

Dropping a process or its run future requests group termination synchronously.
`ReapingChild` keeps child reaping alive outside the async runtime, and the group
guard observes disappearance before retiring the registry entry. If bounded
cleanup fails, fallback observation continues with the group still registered.
These Unix guarantees cover members of the owned group; descendants that create
their own session escape it. Windows retains child-only cleanup pending its
separate job-object work.

### The check-admission and source-capability interface

`AdmittedCheck::admit(engines, root, task, gate, command, required)` asks the
workflow — the concrete task instance the check belongs to, refused before any
byte is hashed when there is no task — which publishes the durable receipt and
observes the workspace through the evidence capability the writer was admitted
with; an unobservable workspace refuses the admission, so an unobservable gate
is never recorded as evidence. Named gates run in the selected project root;
server-executed `ship_check` commands run in the workspace named by their
admitted source observation, and relative report paths resolve there too.
The process directory never selects a different tree for that receipt.
The command runs outside every lock with the owner's edits still flowing.
`settle(engines, execution, details)` takes the
run's `CheckExecution` (an exit code, a timeout, a cancel — a flake is its
passing retry's exit) and settles the check under the captured admission: the
workflow re-observes the source first, then `SettledCheck` carries the
`CheckSettlement` (`Applied` with the recorded check, or `Stale` with the
admission and outcome intact when the run or task was superseded) and, either
way, the settled `CheckReceipt` with its freshness. A nextest junit report is
parsed additively from the project's `target/nextest/<profile>` path; only an
exit decides `passed`.

The receipt, the evidence capability and the completion rule are the
workflow's (see [receipts](project-authority.md#receipts-and-the-evidence-capability)
and [completion](project-authority.md#completion-is-governed-by-receipts));
the boundary observes nothing itself and composes the consumer's typed shape
over what the workflow admitted. `WorkspaceEvidence` and `writer_config` live
in `magistr_core::evidence` as shared composition for the MCP and interactive
hosts. IterationLoop reads progress and recovery records without admitting a
workflow writer; HostedWork supplies its writer explicitly to LoopDriver.

### Identity map

| Identity | Authority | Type | Changes when |
| --- | --- | --- | --- |
| Project | native workflow store (`.think-and-ship/project.json`) | `project_id` on `TaskAdmission`; domain `ProjectId` | never for a project |
| Execution | project owner's execution registry | `ExecutionContext { working_copy, execution_id }`, `ExecutionBinding` on admissions | another working copy or task session is selected |
| Run | contextual ship objective | `run_id` on `TaskAdmission` | a fresh objective after finalize |
| Task and task instance | ship plan | `task_id`, `task_instance` on `TaskAdmission` | the same name in a later run is a new instance |
| Attempt | loop operation ledger | domain `AttemptId` (command plus attempt number) | each retry of one command |
| Workflow owner generation | project lease (`ProjectAuthority`) | `OwnerGeneration`; `epoch` on every feed line | a new project writer is admitted |
| Workflow event sequence | the owner's broadcaster | `seq` on every feed line, `EventCursor { epoch, sequence }` decoded | every frame that owner emits, whatever its family |
| Workflow read provenance | the opened family persistence owner | `ReadSource::File(StoreStamp { inode, modified, length })`, `Missing` or `Memory` within a successful observation | that family's source is observed; it does not certify another family |
| Workflow record stamp | the family engine | `updated_at` on chunks and signals; ids and timestamps on ship actions and checks | that record is written |
| Workflow schema version | the think history file | `schema_version` (1) | the file format changes; roadmap, signal and ship files carry none |
| Workspace | admitted `WorkspaceService` | `WorkspaceIdentity { root, generation }` | the canonical root never; the generation on every owner admission |
| Source revision | the selected bytes | `SourceSnapshot::revision()` (content-addressed) | any selected byte, mode, kind or explicit input |
| Mutation receipt revision | the journal's resolved intent | `MutationReceipt::revision` | per applied intent, over the applied files only |

`CheckIdentities` carries the workflow row and the workspace row side by side.
No type combines them: a workflow run and a workspace generation are
independent stores with independent histories, and a source revision is a
statement about bytes, not about either store. `Revision` in this crate stays
the project authority's own counter. The workflow rows are the projection
identities of [projections](projections.md); none of them is a revision of
the store as a whole, which has none.

## Verification and limits

The application companion suites exercise exact request forwarding, rejected
foreign scopes before dispatch, receipt and response correlation, explicit
outcomes and typed error propagation. Ready fake futures run without an external
executor. Scope regressions were observed failing before their guards were added.

The composition boundary's companion tests admit against a real Host writer on
the production writer config and a disposable root: an external edit during
the run is reported at settlement with its path, a missing task is refused
before hashing, a rolled-over objective settles `Stale` with the target, exit
code and receipt intact, a timeout and a cancel are recorded as what they were,
and a missing root is a typed source refusal at admission. The
`application_composition` integration suite composes a real Host writer with a
real `WorkspaceService` (real worker binary): a check admitted with source
observes an edit made through the owner, lands on its task with a durable
receipt on disk, agrees with the owner's own `MutationReceipt` on the
generation while keeping the two revisions distinct, and is visible to an
unadmitted reader of the same store; completion refuses that gate by name and
accepts once every gate re-ran; completion refuses after an owner edit, a
gate-configuration edit and an in-flight acceptance edit until the gates
re-run; a reused task name in a later run cannot collect the late result and
has no evidence of its own; the cockpit's Complete action refuses in words and,
with current evidence, completes naming `task:verify` while the finalize report
separates the recovered failure from outstanding ones; `roadmap_set_status`
cannot reach `done` and a foreign-scoped objective is refused.

The CLI `application_boundary` integration suite checks Cargo declarations,
workspace membership, the library target and application Rust syntax, including
inactive configuration branches. It shares the domain AST scanner under an
explicit application policy that permits runtime-independent async syntax.
The original domain policy still rejects async. Negative fixtures reject concrete
adapter paths, standard-library effects, unsafe/FFI, source inclusion and
unreviewed dependencies. The existing domain dependency guard continues to
certify the domain's transitive boundary. These checks are regression guards,
not a sandbox for hostile Rust or a proof of third-party implementation behavior.

Focused checks:

```sh
cargo nextest run -p magistr-application
cargo nextest run -p magistr-cli --test application_boundary --test domain_boundary
cargo nextest run -p magistr-cli -E 'test(/application::/) | binary(application_composition)'
cargo test -p magistr-application --doc
```

Formatting and every required gate in `.magistr/project.json` remain mandatory,
including existing interface and architecture-index checks. This chunk changes
no rendering, goldens, motion rules or roadmap export policy. The host supervisor
now owns live headless and cockpit sessions. Native mutations run one
implementation on every surface — the library's tool handler behind the MCP
verb, reached through the admitted writer (see
[one write path](project-authority.md#one-write-path)). Durable command
admission, full setup consolidation and further host migration remain subsequent
chunks; Magistr's own source writes go through the workspace pipeline and the
three process runners share one mechanism (`arch-14f2`, above); every
client's capability, admission path and evidence, the ownership rules the
`native-write-path` gate pins and the consistency model are the
[client inventory](clients.md) (`arch-14f3`); rehoming the adopted source
under Magistr names is `arch-14f4`. Shared permission decisions
are now implemented by the [permission service](permissions.md); production adapters use
that policy for both provider permission requests and [host effects](effects.md),
while the scoped permission-command facade remains a separate contract.
