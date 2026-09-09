# Deterministic domain

`magistr-domain` owns a curated set of Magistr values and rules. It is a
`no_std` crate using `alloc`, with serialization through Serde. It has no
project loader, native store, provider SDK, executor, clock, process launcher,
filesystem, network client, or renderer.

| Module | Ownership |
| --- | --- |
| `identity` | Scoped Magistr identifiers and explicit provider conversation bindings |
| `workflow` | Phase transitions, iteration status and values, spec completion and iteration-limit predicates, workflow command values |
| `policy` | Existing governance rules, matching, severity composition, charter decisions and model tiers |
| `gate` | Gate kinds, statuses and observed outcomes |

The native think-and-ship library continues to own chunks, signals, reasoning,
execution tasks and their durable records. These are not replicated in the
domain. `magistr-engine::tas` remains the native adapter. Engine and core still
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
and application command admission are subsequent architecture work.

## Compatibility and run rules

Engine policy and gate modules re-export the domain's types **and module paths**.
Existing imports therefore use the same definitions, rather than copied models.
The engine retains configuration, lifecycle hooks, tracing and effect adapters.
`WorkflowEngine` delegates phase admission and spec/iteration-limit predicates
to the domain values.

This extraction preserves the [contract baseline](contracts.md): equal and
forward phase transitions are accepted, and a completed phase can restart at
any phase. Spec completion requires a nonempty spec and checked count at least
the total. The iteration limit stops at or beyond the configured count.
The historical two-versus-three no-progress thresholds and recovery differences
remain unchanged until canonical run-state consolidation. Gate status labels,
icons, cached-pass behavior and serialization remain compatible.

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

Focused verification:

```sh
cargo nextest run -p magistr-domain -p magistr-engine
cargo nextest run -p magistr-cli --test domain_boundary
cargo test -p magistr-domain --doc
```

The moved policy/gate suites retain their behavior and serialization assertions.
Additional tests cover identity scope separation, invalid serialized keys,
provider correlation, the phase transition matrix, stop boundaries, and negative
dependency/source fixtures. Formatting and every configured standard/custom gate
remain required. This boundary changes no interface rendering or goldens.

