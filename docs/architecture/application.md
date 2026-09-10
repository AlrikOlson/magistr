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
there before its engines open, and reading needs no admission. The scoped
workflow facade retains its existing adapters; these ports do not themselves
establish the owner.

## Use cases and effect owners

| Module | Application use cases | Adapter obligation |
| --- | --- | --- |
| `sessions` | Start or resume a session, submit a prompt, cancel a turn, close a session | Resolve provider capabilities and models, own live resources, maintain explicit provider bindings, observe terminal events |
| `permissions` | Resolve a scoped decision by selecting an offered option or cancelling | Correlate the live request, enforce project policy before approval, map the option to the provider response |
| `effects` | Read or write text files, create terminals, observe and release owned terminals | Resolve actual targets, authorize through the shared session policy, enforce cancellation and resource ownership, perform host I/O |
| `checks` | Execute a named gate for a scoped verification task | Resolve and run the configured command, record real evidence, distinguish execution from recording |
| `workflow` | Start and complete a native chunk with a scoped task reference | Admit native mutations and preserve the native engine's canonical records and lifecycle rules |
| `configuration` | Read resolved settings, initialize project setup, and patch project settings | Resolve configuration layers, preserve file ownership and user changes, refuse setup over existing configuration |
| `projections` | Read a run's typed state with provenance | Produce a coherent observation with an honest revision, generation, sequence and freshness |

Each scoped facade borrows a `ProjectScope` and its own port. The public methods
name use cases rather than exposing a universal message dispatcher or repository.
Small command enums and request structures describe only the relevant family.
Native chunks and tasks have scoped references; their full native records and
transition rules are not replicated here.

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

`Revision`, `OwnerGeneration` and `EventSequence` are different types.
Revisions are comparable only within the same project authority history.
A generation identifies an owner incarnation, and a sequence orders its events;
neither can be substituted for a state revision. Projection freshness is explicit.
Configuration snapshots use that same authoritative project revision history.
Setup requires absent project configuration; patches preserve unmentioned settings
and the ownership of other configuration layers.

## Outcomes and recovery

Session readiness, prompt acceptance, requested cancellation and observed session
closure are distinct outcomes. Accepting a prompt is not completing a turn.
Requesting cancellation is not observing termination or authorizing session reuse.
This separation matches the protocol distinction between submission and the
terminal response in the
[ACP prompt-turn lifecycle](https://agentclientprotocol.com/protocol/v1/prompt-turn).
The application does not import ACP types or equate a saved provider binding with
supported restoration; that capability belongs at the adapter.

Checks retain the exact configured gate name (including custom and archetype
gates), the domain outcome category, and separate evidence-recording status.
Recorded evidence names the requested scoped task. A passing process result
alone is not a completed workflow task or chunk.
Native completion names a real scoped verification task; native admission and
evidence validity remain the adapter's responsibility.

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

## Verification and limits

The application companion suites exercise exact request forwarding, rejected
foreign scopes before dispatch, receipt and response correlation, explicit
outcomes and typed error propagation. Ready fake futures run without an external
executor. Scope regressions were observed failing before their guards were added.

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
chunks. Shared permission decisions
are now implemented by the [permission service](permissions.md); production adapters use
that policy for both provider permission requests and [host effects](effects.md),
while the scoped permission-command facade remains a separate contract.
