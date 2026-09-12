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
deleted together with the `Capability` variants only they named. What remains
here is the contract layer live adapters implement: sessions, permissions,
effects and configuration, plus the scope, command and receipt types they
share. The place where workflow and workspace capabilities are composed is
the [composition boundary](#the-composition-boundary) in the entry-point crate.

## Use cases and effect owners

| Module | Application use cases | Adapter obligation |
| --- | --- | --- |
| `sessions` | Start or resume a session, submit a prompt, cancel a turn, close a session | Resolve provider capabilities and models, own live resources, maintain explicit provider bindings, observe terminal events |
| `permissions` | Resolve a scoped decision by selecting an offered option or cancelling | Correlate the live request, enforce project policy before approval, map the option to the provider response |
| `effects` | Read or write text files, create terminals, observe and release owned terminals | Resolve actual targets, authorize through the shared session policy, enforce cancellation and resource ownership, perform host I/O |
| `configuration` | Read resolved settings, initialize project setup, and patch project settings | Resolve configuration layers, preserve file ownership and user changes, refuse setup over existing configuration |

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
operations for an attached client. Its associated event type is the client's
portable vocabulary; the trait imports neither ACP nor a renderer. Host admission
errors remain strings on this live adapter contract, and its session-local
permission IDs do not claim durable domain attempt/revision correlation. The
scoped `Sessions` facade remains a separate contract for that later migration.

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
revision (see the [identity map](#identity-map)). Configuration snapshots use
that same authoritative project revision history. Setup requires absent project
configuration; patches preserve unmentioned settings and the ownership of other
configuration layers.

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
| Loop driver gates (`next`, `next-loop`) | `NativeEngines` (Host role via `writer_config`), one admitted verification task for the set | the same capability on the host's config | `AdmittedCheck` per gate, refusing a superseded verification target |
| Native `ship_check` (agent) | `ShipUseCases::admit_check` / `settle_check` inside the workflow crate | the workflow's composed `EvidencePort` | the workflow's own async runner; the stale reply carries the receipt |
| `roadmap_complete_chunk` (MCP, owner command, cockpit Complete, loop judge) | `RoadmapUseCases` with the ship family as completion authority | `required_gates()` and a fresh observation at completion | `verify_completion` before the transition; refusals name every outstanding gate |
| Cockpit `run` | `NativeEngines` or an attached `OwnerClient` | the agent's workspace owner over the spawned bridge | engine port; `magistr workspace journal` for the Changes tab |
| ACP effects (`fs/*`, `terminal/*`) | none | direct host I/O after session authorization | `LocalEffectPort` (routing through the pipeline is `arch-14f2`) |
| `magistr workspace snapshot` | none | lock-free `SourceSnapshot::capture` | CLI verb |

### The check-admission and source-capability interface

`AdmittedCheck::admit(engines, root, task, gate, command, required)` asks the
workflow — the concrete task instance the check belongs to, refused before any
byte is hashed when there is no task — which publishes the durable receipt and
observes the workspace through the evidence capability the writer was admitted
with; an unobservable workspace refuses the admission, so an unobservable gate
is never recorded as evidence. The command runs outside every lock with the
owner's edits still flowing. `settle(engines, execution, details)` takes the
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
in `magistr_core::evidence` because the loop host is admitted there too.

### Identity map

| Identity | Authority | Type | Changes when |
| --- | --- | --- | --- |
| Project | native workflow store (`.think-and-ship/project.json`) | `project_id` on `TaskAdmission`; domain `ProjectId` | never for a project |
| Run | ship objective | `run_id` on `TaskAdmission` | a fresh objective after finalize |
| Task and task instance | ship plan | `task_id`, `task_instance` on `TaskAdmission` | the same name in a later run is a new instance |
| Attempt | loop operation ledger | domain `AttemptId` (command plus attempt number) | each retry of one command |
| Workflow owner generation | role lease (`ProjectAuthority`) | `OwnerGeneration` | a new writer of that role is admitted |
| Workspace | admitted `WorkspaceService` | `WorkspaceIdentity { root, generation }` | the canonical root never; the generation on every owner admission |
| Source revision | the selected bytes | `SourceSnapshot::revision()` (content-addressed) | any selected byte, mode, kind or explicit input |
| Mutation receipt revision | the journal's resolved intent | `MutationReceipt::revision` | per applied intent, over the applied files only |

`CheckIdentities` carries the workflow row and the workspace row side by side.
No type combines them: a workflow run and a workspace generation are
independent stores with independent histories, and a source revision is a
statement about bytes, not about either store. `Revision` in this crate stays
the project authority's own counter.

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
chunks; routing Magistr's own source writes through the workspace pipeline is
`arch-14f2`, the client audit `arch-14f3`, and rehoming the adopted source
under Magistr names `arch-14f4`. Shared permission decisions
are now implemented by the [permission service](permissions.md); production adapters use
that policy for both provider permission requests and [host effects](effects.md),
while the scoped permission-command facade remains a separate contract.
