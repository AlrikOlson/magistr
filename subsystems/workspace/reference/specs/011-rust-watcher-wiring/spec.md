# Feature Specification: Watcher Wiring

**Feature Branch**: `011-rust-watcher-wiring`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description (chunk:rust-watcher-wiring): "Wire the fswatch adapters so edits
made outside the tools (cargo fmt, git checkout, IDE and rust-analyzer saves) refresh the
Tier-1 index; doctor's watcher line becomes live; the freshness fuzzer gains an external
mutator."

> Scope note: constitution Development Workflow rule 8 binds a feature spec to exactly one
> roadmap chunk. This spec covers `chunk:rust-watcher-wiring` only. The adapters themselves
> (`FsWatchAdapter`, `PollingWatcherAdapter`, their debouncer, and the watcher→index bridge)
> already ship and are tested; this feature WIRES them into the serving path, makes their
> behavior under real workloads bounded and observable, and records the numbers. It does NOT
> add a watcher-lag line to `map` (filed separately), does NOT change query-time freshness
> verification (already the correctness guarantee), and is language-agnostic even though it
> lands in the Rust readiness group (Rust workflows churn files outside the tools constantly).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Edits made outside the tools reach the index (Priority: P1)

An agent runs `cargo fmt`, an IDE saves a file, rust-analyzer applies a code action, or a
branch is checked out. Today line numbers stay correct (every answer is verified against the
file before it leaves the server) but a NEW symbol in a NEW file is invisible to symbol-mode
search and read until the index is rebuilt, because nothing tells the index the file exists.
After this feature the server subscribes to the workspace at startup, settled external changes
flow into incremental re-indexing, and a new symbol is findable within a recorded lag.

**Why this priority**: it is the whole point of the chunk, and the adapters are already built
and tested — the missing piece is that nothing in the serving path uses them.

**Independent Test**: start the real index path over a temp workspace with the watcher wired;
write a new file from outside the tools; poll symbol-mode search until the symbol appears;
record the lag for the native adapter and the polling fallback. Delete the file; it leaves the
index.

**Acceptance Scenarios**:

1. **Given** a served workspace at Tier 1, **When** a new file declaring a symbol is written by
   another process, **Then** symbol-mode search returns it within the recorded lag without any
   tool-initiated write or rebuild.
2. **Given** the same workspace, **When** the file is deleted externally, **Then** the symbol
   is no longer returned.
3. **Given** a workspace on a network filesystem (as detected today for the journal mode),
   **When** the server starts, **Then** the polling fallback is selected instead of the native
   watcher, because native watching is untrustworthy there.
4. **Given** the watcher mode is set to `off` (session flag or config), **When** the server
   starts, **Then** no subscription is made and behavior is exactly today's.
5. **Given** a served workspace, **When** the server stops, **Then** the subscription is closed
   before the engine child is stopped — no events after shutdown.

---

### User Story 2 - Bursts are bounded, and the watcher costs nothing idle (Priority: P1)

A `git checkout` can touch thousands of files in a second. Each settled event becomes an
incremental re-index request to the engine child; the child refuses requests beyond its
pending ceiling, and a refusal would leave those files stale until a query verifies them one
at a time. After this feature the bridge bounds how many refreshes are in flight, coalesces
repeated events for the same path, and drains the rest in order — so a checkout-sized burst
never produces a refusal and settles in a recorded time. And because a watcher that costs CPU
while idle would violate the grep cost structure, the idle cost is measured and recorded.

**Why this priority**: Constitution I (no cost the shell lacks; degrade, never fail). A wired
watcher that either refuses under load or spins while idle is worse than no watcher.

**Independent Test**: bind the bridge to a recording index with a small in-flight limit and
emit a burst; assert the in-flight ceiling, the ordering, and the per-path coalescing. Then,
over the real engine path, write 2000 files at once and record the time until all are indexed
with zero refusals. Measure process CPU over an idle window with no watcher, the native
watcher, and the polling adapter over the same tree; record all three.

**Acceptance Scenarios**:

1. **Given** the bridge with an in-flight limit of K, **When** N ≫ K events arrive at once,
   **Then** at most K refreshes are in flight at any moment and all N complete.
2. **Given** two events for the same path while it is queued, **When** they are drained,
   **Then** one refresh runs; a removal event supersedes a queued refresh.
3. **Given** a 2000-file external burst over the real engine path, **When** it settles,
   **Then** every file is indexed, the engine reported zero busy refusals from the watcher, and
   the settle time is recorded.
4. **Given** a 2000-file tree, **When** the server idles for a fixed window, **Then** the
   native watcher's CPU cost is indistinguishable from no watcher (recorded), and the polling
   adapter's cost is recorded and stated as the reason `auto` never picks it on a local disk.
5. **Given** the write engine's own temp files (`*.workbench-tmp-<pid>`), **When** a tool write
   lands, **Then** the bridge ignores the temp-file events and refreshes only the target.

---

### User Story 3 - doctor tells the truth about the watcher (Priority: P2)

`workbench doctor` today prints a fixed sentence: the adapters are present but not active.
After this feature it reports what is actually happening: which adapter is running for which
server process, since when, how many events and refreshes it has handled and when the last one
was — or that the last session ended, or that the watcher is off. doctor runs in its own
process with no lock, so the server publishes this into its state directory, the way it already
publishes plugin health.

**Why this priority**: an invisible subsystem cannot be trusted; the 004 feature deliberately
left this line honest-but-static and said it would become a real probe when the watcher was
wired.

**Independent Test**: start the watcher over a temp workspace; run doctor; the line says live
and names the adapter and pid. Stop; run doctor; it says last session. Delete the file; it
says not active.

**Acceptance Scenarios**:

1. **Given** a running server with the watcher wired, **When** doctor runs, **Then** the
   environment section reports the watcher live with adapter, pid, start time, event and
   refresh counts, and last-event time.
2. **Given** a server that has exited, **When** doctor runs, **Then** it reports the last
   session's record and that the process is gone.
3. **Given** no record (watcher off, or a server that never ran this build), **When** doctor
   runs, **Then** it says the watcher is not active and why it might be.
4. **Given** the record is unreadable, **When** doctor runs, **Then** it says so and names the
   file to set aside — never crashes.

---

### User Story 4 - The freshness fuzzer races a truly external mutator (Priority: P2)

The fuzzer already races a mutator against the real engine child with the watcher bound, for
both adapters. Its mutator lives in the server process. After this feature a second mutator
runs in a separate process (as `cargo fmt`, git and IDEs do), and a checkout-style burst
rewrites every file at once; the oracles — no stale line number, no warm answer older than the
disk — stay green for both adapters.

**Why this priority**: the chunk names it, and the difference between in-process and
cross-process mutation is exactly the difference between a test that exercises the watcher and
one that happens to pass without it.

**Independent Test**: `bun test test/freshness-fuzz.test.ts`.

**Acceptance Scenarios**:

1. **Given** the fuzzer with a spawned-process mutator, **When** it runs for both adapters,
   **Then** every oracle holds and the closing exam is warm and complete.
2. **Given** a checkout-style burst phase, **When** it runs, **Then** the same oracles hold.

---

### Edge Cases

- Events arriving before the Tier-1 build has finished: a refresh upserts one file; the build
  overwrites it later with the same facts — harmless, and not waited for.
- The engine child crashes and restarts mid-burst: queued refreshes continue against the
  restarted child; a refresh that fails is dropped (query-time verification still answers
  correctly, slower) and counted in the status record.
- `.git/` and `.workbench/` events are filtered (already); the write engine's temp-file suffix
  is filtered too; junk paths are upserted like any other (the index already classes them).
- The status record is cache-class: unreadable ⇒ reported and ignored; a stale record from a
  dead pid is "last session", never "live".
- `watcher.mode=native` on a network filesystem is honored (the user asked) and the doctor
  record says so; `poll` on a local disk likewise.
- Shutdown order: watcher closed first, then the engine child, then the journal and lock.
- The watcher lives in the server process and reaches the index over IPC; the SDD places it
  in the engine child. Writes still happen only in the child (single writer); the deviation is
  recorded, not hidden.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The server MUST subscribe a watcher to the workspace root at startup, bound to
  the index through the existing bridge, unless the watcher mode is `off`.
- **FR-002**: A `watcher.mode` setting MUST exist with values `auto`, `native`, `poll`, `off`
  (default `auto`); `auto` MUST select the polling adapter when the workspace is on a detected
  network filesystem and the native adapter otherwise.
- **FR-003**: The subscription MUST be closed during orderly shutdown before the engine child
  stops.
- **FR-004**: The bridge MUST bound concurrent refreshes, coalesce queued events per path
  (removal superseding refresh), and drain in arrival order; it MUST NOT let a burst reach the
  engine's pending ceiling.
- **FR-005**: The bridge MUST ignore the write engine's temp files.
- **FR-006**: The server MUST publish a watcher status record in the state directory
  (cache-class): adapter, mode, pid, root, start time, counters (events, refreshes, removals,
  failures), last-event time, last error; updated at least once per second while events flow
  and at stop.
- **FR-007**: doctor MUST read that record without a lock and report live (pid alive), last
  session (pid gone), not active (no record), or unreadable (named file), with the record's
  detail.
- **FR-008**: The external-write-to-visible lag MUST be measured for both adapters through the
  real index path and recorded in the ship trace (Constitution VI).
- **FR-009**: A 2000-file external burst MUST complete with zero watcher-caused busy refusals
  and its settle time MUST be recorded.
- **FR-010**: Idle CPU cost MUST be measured over the same tree for no watcher, native, and
  polling, and recorded; the native cost MUST be within the "no cost the shell lacks" bar.
- **FR-011**: The freshness fuzzer MUST gain a separate-process mutator and a checkout-style
  burst phase and MUST stay green for both adapters.
- **FR-012**: Tool-initiated writes and every existing freshness guarantee MUST be unchanged;
  `watcher.mode=off` MUST reproduce today's behavior exactly.

### Key Entities

- **Watcher mode**: the resolved setting (`auto` → native/poll by detection, or forced).
- **Bounded bridge**: the watcher→index forwarder with an in-flight limit, a per-path queue,
  and counters.
- **Watcher status record**: the cache-class file the server publishes and doctor reads.
- **Recorded numbers**: lag (native/poll), burst settle time, idle CPU (none/native/poll).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A new external file's symbol appears in symbol-mode search within the recorded
  lag for both adapters; the lags are in the ship trace — pinned by test.
- **SC-002**: Bridge bound: at most K in flight, per-path coalescing, removal wins, all N
  complete — pinned by test; a 2000-file burst indexes every file with zero busy refusals and a
  recorded settle time.
- **SC-003**: Idle CPU recorded for none/native/poll over a 2000-file tree; native within the
  bar — pinned by test with the numbers in the trace.
- **SC-004**: doctor reports live / last session / not active / unreadable correctly — pinned
  by test; `watcher.mode=off` leaves no record and no subscription.
- **SC-005**: The freshness fuzzer passes for both adapters with the spawned-process mutator
  and the burst phase.
- **SC-006**: Mode selection pinned: auto+local ⇒ native, auto+network ⇒ poll, forced modes
  honored, off ⇒ nothing.
- **SC-007**: Full gate sweep green.

## Assumptions

- The shipped adapters and bridge are correct as tested; this feature changes the bridge only
  to add the bound, the temp-file filter, and counters.
- Network-filesystem detection is the existing `detectNetworkFs`; no new detection.
- The state directory is already cache-class and gitignored; one more JSON record there needs
  no migration and no version beyond a `v` field.
- Idle-cost numbers are machine-dependent; the test asserts a generous bound and records the
  measured values rather than asserting them tightly.
