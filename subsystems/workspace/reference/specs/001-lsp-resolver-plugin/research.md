# Phase 0 Research: LSP Resolver Plugin

All decisions grounded in the repository (SDD, constitution, wit/, src/tools/plugins.ts,
crates/workbench-core/src/sandbox.rs) — no external research was required; the SDD
settled the architecture questions in v1.1 and M4 built the substrate.

## D1 — The host-call bridge is the feature's real deliverable

**Decision**: Extend the component lane in `crates/workbench-core/src/sandbox.rs` with
host-implemented WIT imports (the "host-call bridge"), starting with exactly one
capability: subprocess. The bridge honors the grant model `plugins.ts` already records:
effective capabilities = manifest imports ∩ recorded grants, default deny, refused
before instantiation.

**Rationale**: `src/tools/plugins.ts` (header, chunk:m4-capability-manifests) states
the guest is a pure `(string) -> string` invoke and "the bridge lands with the first
first-party consumer (M5); grants recorded now are what it will honor." The grant
records, tier field (`blessed`/`verified`), pre-instantiation denial, and the
disk-backed circuit breaker all exist; only the bridge is missing.

**Alternatives considered**: a separate resolver host process outside the sandbox
(rejected: violates "first-party features MUST be built on the public SDK" — it would
be a private special case, exactly what the chunk forbids); WASI sockets/exec
(rejected: the M4 escape suite pins WASI capabilities OUT of the grant vocabulary).

## D2 — Subprocess capability: host-curated specs, never command lines

**Decision**: The subprocess WIT interface takes a **language-server spec id** (e.g. a
language id resolved by host-side detection), not a command line. The host maintains
the detection table (binary, args, transport=stdio); the guest can only select from
what the host detected. Grant name is the WIT import name (world import names are the
grant vocabulary, SDD §8.2); the host additionally requires inventory tier `blessed`
for this one import — a non-blessed plugin declaring it is `PLUGIN_CAP_DENIED` before
instantiation even if a human granted it.

**Rationale**: Constitution IV: "no general shell execution anywhere in the system, at
any trust tier." A guest-supplied command line is general execution with extra steps.
Host-curated specs keep the capability exactly as narrow as the SDD's phrase
"project-detected language servers". The blessed-tier check implements "the one
capability reserved to blessed plugins" (§8.3).

**Alternatives considered**: guest-supplied argv with host allowlist filtering
(rejected: allowlist-by-string is the `run` tool's model, and even `run` refuses
free-form composition; a filter is a bypass magnet); granting via manifest alone
without the tier check (rejected: sideloads are permanently untrusted and a human can
be tricked into granting — the escape suite's premise).

## D3 — WIT placement: new package `workbench:subprocess@0.1.0`

**Decision**: The capability lives in its own WIT package with its own version stamp,
imported by the resolver plugin's world; `workbench:resolver` stays at 0.1.0 export
vocabulary (a world-import addition to the plugin's own world does not alter the
published resolver package).

**Rationale**: wit/README design decisions: primitives duplicated per package because
"independence of the stamps is the point"; a new package is purely additive to
`wit/baseline.json` (bless required, `WIT_BASELINE_BLESS=1`). Other future blessed
plugins can declare the same import without touching the resolver package.

**Alternatives considered**: adding an import interface inside `resolver.wit`
(rejected: couples the capability's evolution to the resolver export stamp);
reusing `workbench:tool` host imports (rejected: those are the script-lane read/write
lanes — a different trust shape, and the escape suite pins their scope).

## D4 — Resolver port evolution: v1 → v2, additive

**Decision**: `PORT_VERSIONS.resolver` bumps to 2. v2 adds per-edge provenance to
resolve results (and the plugin-backed adapter). Existing symbolic resolution is
untouched; `InProcessResolver` and any engine-lane implementor advertise v2 by
carrying the provenance field with symbolic values.

**Rationale**: `wit/resolver.wit` provenance note says the internal port "grows INTO
this surface via a PORT_VERSIONS bump when the LSP plugin lands (M5)". versions.ts
documents every prior bump as additive-with-comment; same pattern.

**Alternatives considered**: position-based resolve requests in v2 (deferred: the WIT
surface's `resolve-references: func(symbol: string)` is name-based today; adding
position-based lookups is a later additive bump when the surface itself grows — doing
it now would put a port ahead of the public contract it mirrors).

## D5 — Provenance vocabulary extends the existing confidence strings

**Decision**: The references lane already emits per-hit `confidence` + `provenance`
(`tier1-symbols`, `tier1-imports`, `tier1-text` — see `search.ts` references mode).
Resolver-upgraded edges add a distinct provenance value (e.g. `resolver-lsp`) with
`confidence: "high"`; symbolic fallbacks keep their current labels. Envelope shape is
additive — no schema major.

**Rationale**: SC-003 (never label symbolic as resolver-verified) becomes a string
inequality pinned by test; agents already parse this vocabulary.

## D6 — Circuit breaker: reuse the disk-backed plugin-health mechanism

**Decision**: Resolver health rides `.workbench/plugin-health.json` exactly as any
plugin crash does (keyed plugin name + component hash), plus per-language-server
breaker entries for the subprocess targets (a TS server dying must not open the
circuit for the Rust server — spec edge case). Recovery is lazy and bounded:
health is consulted per request (stateless), a changed component hash clears the
breaker (existing semantics), and a spec'd retry budget guards hopeless targets.

**Rationale**: the mechanism exists, is stateless-compatible, and the TCK already
exercises its crash path. Inventing a second breaker would fork the recovery story.

## D7 — Testing strategy: fake language server as the primary rig

**Decision**: Acceptance tests drive a scripted fake language-server executable
(speaks just enough LSP over stdio: initialize, references, crash-on-command,
hang-on-command) registered through the detection table's test seam. One real-world
detection heuristic ships (spec Assumptions: breadth is not acceptance); the kill
test (SC-001) runs the full tool surface with the resolver absent, crashing, and
timing out.

**Rationale**: deterministic CI without depending on a real language server install;
mirrors the test-rig-mirrors-live-lane discipline the project just reinforced in
d0e2492 (rig fakes must throw/behave like the live lane).

## D8 — MVP slice = User Story 1 with the fake server

**Decision**: The independently-shippable MVP is US1+US3 mechanics over the fake
server (plugin boots, resolves, provenance labeled) with US2's degradation guarantee
proven by the kill test from day one; real-server detection is the last mile.

**Rationale**: matches spec priorities; the degradation guarantee (US2) is cheap to
pin early because it is the pre-feature product plus assertions.
