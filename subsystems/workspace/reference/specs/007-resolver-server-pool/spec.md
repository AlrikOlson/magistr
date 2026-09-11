# Feature Specification: Resolver Language-Server Session Pool

**Feature Branch**: `007-resolver-server-pool`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description: "Persistent, supervised language-server session pool for the resolver lane (chunk:backlog-resolver-server-pool). The resolver lane spawns language servers per invoke; a persistent, supervised server session pool would let slow real servers (tsserver-class, seconds to initialize) contribute Tier-2 answers instead of circuit-breaking on the per-request budget. Sessions currently live inside one guest invoke (the per-invoke pool drops with the store — the teardown guarantee). The pool needs a deliberate home in engine-child infrastructure, must not break statelessness, and must keep the kill-on-fuel/trap/restart guarantee. Precondition for rust-resolver: rust-analyzer takes many seconds to initialize and a per-invoke spawn breaches the 2 s per-request budget every time."

> Scope note: constitution Development Workflow rule 8 binds a feature spec to exactly one
> roadmap chunk. This spec covers `chunk:backlog-resolver-server-pool` only. Adding
> rust-analyzer to the host detection table is `chunk:rust-resolver` (a later feature) and is
> out of scope here; this feature makes that chunk possible.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A slow real server contributes on the second request (Priority: P1)

An agent works in a project whose detected language server takes seconds to become useful
after it starts (rust-analyzer, tsserver). Today every references request starts a fresh
server, runs the full handshake, and usually loses the per-request budget, so the upgrade
never arrives and the circuit keeps opening. With this feature the first references request
starts the server and hands it back warm; the second and every later request reuse the same
running server without repeating the handshake, and Tier-2 edges arrive inside the budget.

**Why this priority**: this is the whole reason the chunk exists and the precondition for
every Rust Tier-2 claim downstream (`rust-resolver`, the 1M-LOC bench, the dogfood gate).
Without it the resolver plugin is correct but inert for exactly the servers that matter.

**Independent Test**: with a language server whose handshake is deliberately slower than the
per-request budget, issue references twice. The first answer is symbolic (budget lost, or
empty), the second carries resolver edges, and the server received exactly one handshake.

**Acceptance Scenarios**:

1. **Given** a detected server whose handshake takes longer than the per-request budget,
   **When** the agent requests references twice, **Then** the second answer carries
   resolver-verified edges and the server's handshake ran exactly once across both requests.
2. **Given** a warm session, **When** a later request reuses it, **Then** the guest is told
   the session is reused so it skips its protocol handshake, and frames buffered while the
   session sat idle are not delivered to the new request.
3. **Given** a warm session that has become unusable (the server died, stopped answering, or
   answered malformed data), **When** the guest closes it, **Then** the process is killed and
   the next request starts a fresh server for that language.
4. **Given** two requests in flight at once for the same language, **When** the second asks to
   open the session the first holds, **Then** it is refused with a typed open error (it never
   waits) and that language answers empty for the second request.

---

### User Story 2 - Every server process dies when it should (Priority: P1)

A pooled server outlives the request that started it, so the teardown guarantee that used
to come for free (the session pool dropped with the guest's store) has to be made explicit.
No matter how the guest or the engine child ends — a clean answer, running out of fuel, a
trap, a memory-cap kill, a deliberate shutdown, or a crash that restarts the child — no
language-server process is left running that nothing supervises.

**Why this priority**: an orphaned rust-analyzer holding gigabytes on a developer machine is a
worse outcome than no upgrade at all. The chunk's own acceptance names this guarantee; the
spec makes each ending a test.

**Independent Test**: start a warm session, end the guest in each failing way and the engine
child in each ending way, and assert the server process is gone within a bounded time.

**Acceptance Scenarios**:

1. **Given** a guest invoke that ends by fuel exhaustion, trap, or memory-cap kill, **When**
   the invoke ends, **Then** every session it held is killed, not returned to the pool.
2. **Given** a guest invoke that ends normally, **When** it ends, **Then** sessions it did not
   close are returned warm and sessions it closed are dead.
3. **Given** the engine child is stopped deliberately or ends because its parent went away,
   **When** it exits, **Then** every pooled server process is killed.
4. **Given** the engine child dies abruptly (crash, forced kill) and is restarted, **When** the
   new child serves its first request, **Then** no server spawned by the old child is still
   running — on the platforms where the operating system can enforce this (see Assumptions).
5. **Given** a warm session that no request has used for longer than the idle limit, **When**
   the limit passes, **Then** its server is killed and the next request starts fresh.

---

### User Story 3 - The product stays stateless and compatible (Priority: P2)

A warm pool is process-lifetime infrastructure, not session state: any request is answerable
from a cold pool, the identity of the server to use (workspace root plus the host-curated
launch spec) travels on every request, and nothing about a request's meaning depends on an
earlier request. Plugins built against the previous subprocess contract keep working exactly
as before — pooling is something a guest opts into by speaking the new contract — and
existing grant records keep authorizing the capability.

**Why this priority**: the statelessness gate and the versioned-contract rule are
release-blocking; a pool that quietly broke either would be a regression disguised as a
feature.

**Independent Test**: the statelessness suite stays green unchanged; a guest built against
the previous subprocess contract gets per-invoke sessions byte-for-byte as before; a grant
recorded under the previous contract version still authorizes the new one; the WIT
compatibility gate accepts the change as additive.

**Acceptance Scenarios**:

1. **Given** the release statelessness suite, **When** it runs after this feature, **Then** it
   passes unmodified.
2. **Given** a guest that imports the previous subprocess contract version, **When** it opens,
   uses and closes sessions, **Then** sessions are per-invoke and killed at invoke end
   exactly as before, and the guest is never told a session is reused.
3. **Given** a plugin record whose grant names the previous subprocess contract version,
   **When** the rebuilt resolver (declaring the new version) is resolved, **Then** the
   capability is still effective — a grant covers the same capability within one major.
4. **Given** the WIT compatibility gate, **When** it runs, **Then** the subprocess package's
   change is accepted as additive (stamp moved within the major) and every other package is
   unchanged.

---

### User Story 4 - The claim carries a number (Priority: P3)

The product records, for one real language server on this machine, the latency of a
resolver-upgraded references call when the session is cold versus warm, so the claim "slow
servers now contribute" is a measurement, not an argument (Constitution VI).

**Why this priority**: the chunk's fourth acceptance item; cheap once US1 exists; the number
is also the baseline the `rust-resolver` chunk's own p95 will be compared against.

**Independent Test**: run the recorded scenario with the real server present; it prints cold
and warm latencies and the handshake count. With the server absent the scenario skips with a
message naming the binary — it never claims a number it did not measure.

**Acceptance Scenarios**:

1. **Given** a real language server on PATH, **When** the recorded scenario runs, **Then** it
   reports cold and warm latency for a resolver-upgraded references call and asserts the
   warm call reused the session.
2. **Given** the binary is absent, **When** the scenario runs, **Then** it skips with an
   explicit message naming the binary, and no number is recorded.

---

### Edge Cases

- A warm server crashes between requests: the next open detects the dead process, discards
  the session and spawns fresh; the request that found it dead answers that language empty.
- The guest's host-side budget expires while the guest is still mid-conversation: the
  in-flight invoke keeps the lease until it ends (it is never interrupted mid-frame); a
  concurrent open for that language is refused typed; when the invoke ends normally the
  session returns warm, so the next request benefits from the work already done.
- Frames the server emits while a session is idle (progress, diagnostics): discarded, so the
  server's output pipe never fills and a reused session starts with an empty read buffer.
- Two different workspaces (roots) served by the same engine child: sessions are keyed by
  root plus spec; they never cross.
- A spec changes (different binary or arguments for the same language): it is a different
  key; the old session is reaped by the idle limit, never reused for the new spec.
- Guest ends normally but never called close and never used the session: returned warm.
- Session ceiling reached for an invoke: typed refusal, as today.
- The platform cannot enforce kill-on-parent-death (see Assumptions): deliberate shutdown
  and the idle limit still apply; the gap is named in the plan, not hidden.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Language-server sessions MUST be able to outlive the guest invoke that opened
  them and be reused by later invokes, keyed by the workspace root and the host-curated launch
  spec, inside the engine child that owns the sandbox.
- **FR-002**: A guest MUST be able to learn whether an opened session is reused (warm) or
  fresh, and MUST be able to return a session to the pool without killing it; closing a
  session MUST still kill its process.
- **FR-003**: A session MUST be held by at most one guest invoke at a time; a second invoke
  asking for a held session MUST be refused with a typed open error and MUST NOT wait.
- **FR-004**: Frames a server emits while its session is idle MUST be discarded; a reused
  session MUST start the new lease with an empty read buffer; the server's output MUST keep
  draining while idle so the server never blocks on a full pipe.
- **FR-005**: Sessions held by a guest invoke that ends by fuel exhaustion, trap, memory-cap
  kill, or any other non-normal end MUST be killed, never returned; sessions held by an
  invoke that ends normally MUST be returned unless the guest closed them.
- **FR-006**: Every pooled server process MUST be killed when the engine child exits
  deliberately or because its input stream closed, and MUST be killed by the operating system
  when the engine child dies abruptly on platforms that can express kill-on-parent-death; the
  platforms on which the guarantee holds only through the server's own end-of-input exit MUST
  be named.
- **FR-007**: A warm session unused for longer than a bounded idle limit MUST be killed.
- **FR-008**: The host MUST remain protocol-unaware: it moves frames and manages processes;
  the handshake-skipping decision on a reused session belongs to the guest.
- **FR-009**: The subprocess contract change MUST be additive under the WIT evolution rules:
  existing functions, types and variant cases unchanged; the package stamp moves within its
  major; a guest importing the previous version MUST keep per-invoke session semantics.
- **FR-010**: A capability grant recorded under one version of the subprocess contract MUST
  authorize the same capability at any later version within the same major; the blessed-tier
  restriction MUST apply to every version of the capability equally.
- **FR-011**: Nothing about a request's answer MAY depend on in-memory session state: the pool
  is a cache of live processes addressed by request-carried identity; the circuit breaker and
  every other health fact stay on disk; the statelessness suite MUST pass unmodified.
- **FR-012**: The bundled resolver guest MUST be updated to the new contract: skip its
  handshake on a reused session, close only sessions it judges unusable, and otherwise leave
  sessions for the pool; it MUST remain conformant to the plugin TCK unmodified.
- **FR-013**: The resolver outcome returned to the host MUST carry, additively, how many
  sessions the invoke opened fresh and how many it reused, so reuse is observable at the
  host boundary without inspecting server processes.

### Key Entities

- **Server pool**: the engine-child-lifetime registry of live language-server sessions, keyed
  by workspace root plus launch spec; owns process lifecycle, leasing, idle reaping and
  shutdown.
- **Session lease**: one guest invoke's exclusive hold on a pooled session; begins at open,
  ends at invoke end; its ending (normal or not) decides warm return versus kill.
- **Launch spec**: the host-curated, per-request description of which binary serves a
  language (existing entity); with the root it forms the pool key.
- **Idle limit**: the bounded time a warm, unleased session may wait before it is killed.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With a server whose handshake exceeds the per-request budget, the second
  references request carries resolver-verified edges and the server received exactly one
  handshake across the two requests (pinned by test over a scripted server).
- **SC-002**: In every ending scenario (normal, fuel, trap, memory cap, close, deliberate child
  stop, child input closed, idle limit), the affected server processes are gone within a
  bounded wait (pinned by test); abrupt child death is pinned on the platforms that enforce
  it and skipped with a named reason elsewhere.
- **SC-003**: The statelessness suite, the sandbox-escape suite, the plugin TCK and the WIT
  compatibility gate pass unmodified apart from the additive subprocess stamp.
- **SC-004**: A previous-contract guest observes byte-identical per-invoke behavior (pinned
  over a minimal component importing the previous version).
- **SC-005**: For one real language server present on this machine, cold and warm latency of
  a resolver-upgraded references call are recorded in the ship trace with the handshake
  count; when the server is absent the scenario skips naming the binary.
- **SC-006**: Zero new dependencies enter the lock files; any operating-system calls needed
  for kill-on-parent-death use crates already present in the lock.

## Assumptions

- The bundled resolver is the only blessed plugin today, so the contract bump and the guest
  update ship in one change; third-party blessed plugins do not exist yet, but the
  previous-version path is kept so that they could.
- Kill-on-parent-death can be expressed on Windows (job objects) and Linux (parent-death
  signal). On macOS the guarantee for an abruptly killed engine child rests on the server
  exiting when its input closes, which well-behaved language servers do; this is recorded as
  a named gap, and the abrupt-death test is environment-gated there.
- The per-request budget, the circuit breaker and its expiry, and the host detection table
  are unchanged by this feature; the first request against a cold slow server may still lose
  the budget and record a breach — that is the existing contract, and the pool makes the
  breach expiry useful instead of futile.
- No pre-warming: the host cannot run the guest's protocol handshake, and starting the
  process alone saves little; a request pays the handshake once. Revisit with dogfood data
  if first-request latency proves to matter.
- rust-analyzer, rustfmt and cargo are present on the machine that shapes this feature and
  typescript-language-server is not; the recorded real-server number therefore comes from
  rust-analyzer through the test seam over a fixture crate, and every real-binary scenario is
  environment-gated with a message naming the binary.
