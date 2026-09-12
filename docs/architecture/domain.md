# Deterministic domain

`magistr-domain` owns a curated set of Magistr values and rules. It is a
`no_std` crate using `alloc`, with serialization through Serde. It has no
project loader, native store, provider SDK, executor, clock, process launcher,
filesystem, network client, or renderer.

| Module | Ownership |
| --- | --- |
| `identity` | Scoped Magistr identifiers and explicit provider conversation bindings |
| `workflow` | Canonical phases, run modes and stopping policy, attempt settlement, iteration status and values, spec completion, workflow command values |
| `project` | Project context snapshot supplied by effectful adapters |
| `policy` | Existing governance rules, matching, severity composition, charter decisions and model tiers |
| `gate` | Gate kinds, statuses and observed outcomes |

The native think-and-ship library continues to own chunks, signals, reasoning,
execution tasks and their durable records. These are not replicated in the
domain. `magistr-engine::native` remains the native adapter. Engine and core still
contain effects; neither crate is a substitute for the domain dependency boundary.
The old core `DomainState` remains a session/interface aggregate.

## Identity scopes

An identity includes its parent's identity, so the same local key under another
parent is a different value. Constructors accept externally allocated opaque
keys; they neither infer identity from paths nor generate it from clocks or
randomness. Empty keys and control characters are rejected, including on JSON
deserialization. Other spelling is preserved.

| Identity | Scope |
| --- | --- |
| `ProjectId` | Project authority's key |
| `RunId` | Project |
| `SessionId` | Run |
| `TurnId` | Session |
| `CommandId` | Run |
| `AttemptId` | Command plus a nonzero attempt number |
| `DecisionId` | Requesting turn |

A `ProviderSessionId` holds a provider namespace and its opaque conversation ID.
Its explicit `bind_to(SessionId)` operation produces a
`ProviderSessionBinding`; no implicit conversion to a Magistr session exists.
The binding records correlation, not a claim that the provider supports resume.
Application adapters must allocate fresh identities and maintain bindings; the
values themselves do not enforce global uniqueness or authorize effects.

The existing `magistr_core::SessionId` remains the unscoped provider wire value
used in agent messages. Its `bind_provider_id` adapter requires both a provider
namespace and an independently allocated domain session. Existing transport
payloads and stored runtime sessions are unchanged. Shared session supervision
and production command admission are subsequent architecture work. The
[application contracts](application.md) now reuse these identities for scoped
use cases, with concrete adapters supplied by hosts.

## Compatibility and run rules

Engine policy and gate modules re-export the domain's types **and module paths**.
Existing imports therefore use the same definitions, rather than copied models.
The engine retains configuration, lifecycle hooks, tracing and effect adapters.
Core and engine now re-export the same
[`WorkflowPhase`](../../crates/magistr-domain/src/workflow/workflow_phase.rs),
[`IterationStatus`](../../crates/magistr-domain/src/workflow/iteration_status.rs)
and [`ProjectContext`](../../crates/magistr-domain/src/project.rs) values.
Phase labels are explicit: the domain supplies `Researching`, for example, and
the [core notification adapter](../../crates/magistr-core/src/workflow_state.rs)
adds progress punctuation. Equal and forward phase transitions remain accepted,
and a completed phase can restart at any phase. Spec completion still requires
a nonempty spec and checked count at least the total. Gate labels, icons,
cached-pass behavior and serialization remain compatible.

Architecture chunk `arch-03-run-semantics` implements the run-policy decisions
recorded in the [contract baseline](contracts.md#implemented-run-consolidation).
[`RunMode`](../../crates/magistr-domain/src/workflow/run_mode.rs) distinguishes
conversation, one-chunk execution and bounded chunk loops.
[`RunState`](../../crates/magistr-domain/src/workflow/run_state.rs) owns the
attempt cursor, iteration ceiling, consecutive no-commit count, cumulative
budget observation and limit, settlement marker and terminal reason.
[`StopReason`](../../crates/magistr-domain/src/workflow/stop_reason.rs) names
completion, limits, no progress, budget, cancellation, failure, abort and pause.
Conversation has no Git no-progress guard; one-chunk mode stops after its
selected attempt; chunk loops stop after two consecutive no-commit attempts.
Repeated settlement of the same attempt cannot increase that count. An observed
commit clears it, while recovery and provider restarts retain it. Only an
explicit pause is resumable through the domain's resume operation; a later
observation does not clear another terminal reason.

[`IterationLoop`](../../crates/magistr-core/src/workflow/iteration.rs) and
[`WorkflowEngine`](../../crates/magistr-engine/src/workflow/workflow_engine.rs)
delegate admission and settlement decisions to `RunState`. Core's loop-state
DTO retains its old iteration/max fields as validated projections and preserves
the historical `success` JSON spelling for canonical `Committed`. The domain
iteration projection retains `u32` spec counts; core spec/context collections
retain `usize` counts. `WorkflowPhase::step_index` and `ProjectContext` use the
existing core `usize` API width without narrowing adapter observations.

The [context builder](../../crates/magistr-core/src/workflow/context_builder.rs)
still gathers Git/filesystem/native observations and formats prompts. Supplying
its shared `ProjectContext` to the engine requires no model conversion and does
not establish an execution cycle.

## Recovery boundaries

Headless, plain and cockpit next-loop commands use core's `open_or_recover`.
Recovered mode, limits, budget, settlement marker and terminal state remain
authoritative. The persisted Git baseline belongs to an explicit attempt, so a
restart cannot hide a commit made while the process was absent. Missing run
evidence or inconsistent compatibility projections refuse recovery. Legacy
`.magistr/loop-state.json` files without this evidence require an explicit new
run: preserve the file elsewhere or remove it deliberately. Recovery never
silently supplies a zero count or overwrites the invalid checkpoint. Completed
run checkpoints are retained too; starting another run requires the same explicit
file preservation or removal.

Headless loops checkpoint priced usage before reporting it and save settlement
before fallible progress output. Cockpit loops retain observed cumulative cents
across provider sessions and drain queued observations before opening a successor.
Headless EOF without a terminal result records failure; cancelled or incomplete
cockpit turns cannot proceed to verification. These adapters preserve evidence
they actually receive, without estimating unreported provider spend.

Engine [`SessionSnapshot`](../../crates/magistr-engine/src/session/session_snapshot.rs)
format version 2 requires the canonical run and complete
[`CostGovernor`](../../crates/magistr-engine/src/cost/cost_governor.rs), including
budget policy, spend, warning state and attribution. Restoration validates run,
iteration, count and cost projections before installing them. Version 1 and
missing required evidence are refused. The
[`EngineHandle`](../../crates/magistr-engine/src/handle/engine_handle.rs) installs
the recovered governor rather than constructing a new budget from session cost.
A fresh handle can import a valid snapshot; a live handle refuses an older one
that would erase known spend, counters, limits, warnings or terminal outcomes.
The cockpit ledger's engine snapshot and the production loop checkpoint remain
separate adapters. Plain command drains still expose a coarse success/error result;
shared provider supervision is needed to unify their terminal-event contract.
Peer reservations and worker request/receipt state remain separately owned by
the [peer journal](peers.md); they are not session-snapshot fields.

These changes do not provide a shared provider supervisor. `arch-08` supplies
one; `arch-13` makes the loop checkpoint atomic and records the run's
operations for recovery — see [durable operations](durability.md). No
live-provider recovery or desktop integration is claimed.

## Enforcement and verification

The CLI integration suite `domain_boundary` runs in the required workspace test
gate. It checks Cargo dependency metadata and the domain's Rust syntax, including
inactive configuration branches and companion tests. The reviewed runtime
dependency closure is Serde and its derive machinery. It rejects effectful
dependencies, explicit standard-library access, unsafe/FFI, source inclusion,
environment macros and unreviewed macro expansion. Serde JSON is a test-only
dependency.

The syntax check complements compilation: Rust's
[`no_std` reference](https://doc.rust-lang.org/reference/names/preludes.html#the-no_std-attribute)
explicitly permits manually linking `std`.
[Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html)
supplies dependency kinds and package declarations. The guard walks those
declarations for the production Serde chain, including disabled optional edges.
It also checks the resolved test dependency chain by package ID, rejecting local
replacements and unreviewed enabled dependencies of Serde JSON. The check is an architecture
regression guard, not a sandbox for hostile Rust code or a proof about arbitrary
third-party implementations. Expanding its dependency or syntax allowlist
requires reviewing the new capability.

The Workbench source adoption requires Wasmtime 48 and Indexmap 2.14. Its
workspace feature unification also enables Hashbrown's collection helpers in
the test-only Serde JSON graph. The reviewed development closure includes
Hashbrown's allocator-api2, equivalent, foldhash and Serde edges; the production
Serde declaration closure remains unchanged. Inspection of the locked sources
found foldhash 0.2.0's clock access behind `std` in `src/seed.rs`, and
allocator-api2 0.2.21's I/O trait implementations behind `std`. The guard
therefore requires resolved feature metadata, admits no foldhash features and
only allocator-api2's `alloc` feature, and keeps both packages as dependency
leaves. These checks apply to each resolved package ID, so workspace feature
unification cannot silently enable the excluded capabilities. Negative fixtures
cover clock/I/O-enabling features, runtime/filesystem dependency grafts and local
package replacements. Hash seeds inside test collections are not a domain
identity source or a promise of deterministic hash-table internals.

Focused verification:

```sh
cargo nextest run -p magistr-domain -p magistr-engine -p magistr-core
cargo nextest run -p magistr-cli --test domain_boundary
cargo test -p magistr-domain --doc
```

The moved policy/gate suites retain their behavior and serialization assertions.
Additional tests cover identity scope separation, invalid serialized keys,
provider correlation, the phase transition matrix, stop boundaries, and negative
dependency/source fixtures. Run suites cover duplicate settlement, two-attempt
stopping, observed-commit reset, retained budgets and terminal outcomes, invalid
legacy recovery, and full engine-governor restoration. Core context tests pass
the gathered value directly to the engine; compatibility tests pin phase labels,
API widths and every legacy iteration-status spelling. Formatting and every configured standard/custom gate
remain required. This boundary changes no interface rendering or goldens.
