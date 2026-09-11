# Phase 0 Research: Watcher Wiring

Grounded in `src/adapters/fswatch/watcher.ts` (`FsWatchAdapter`, `PollingWatcherAdapter`,
`Debouncer`, `bindWatcherToIndex` — shipped, tested, imported by nothing outside tests),
`src/main.ts` (`composeSearch`: `EngineSupervisor` + `IpcIndexAdapter`, `stop()`, the lazy
build kick; the SIGINT/SIGTERM release path), `src/protocol/workspace.ts`
(`WorkspaceRuntime.detection.networkFs`), `src/config/schema.ts` (`CONFIG_KEYS`, the
`tracing.enabled` non-security precedent), `src/doctor/probes.ts` (`probeEnvironment`'s fixed
watcher line; `probePlugins` reading the server-written `plugin-health.json`),
`src/adapters/workspace-lock.ts` (`{pid, acquiredAtMs}` + `pidAlive` via `process.kill(pid,0)`),
`src/adapters/engine-ipc/supervisor.ts` (`DEFAULT_MAX_PENDING = 128` → `ENGINE_BUSY`),
`test/fswatch.test.ts`, `test/freshness-fuzz.test.ts`. Pinned in think:164/165.

## R1 — Wire in `composeSearch`, right after the index adapter; close first on stop

**Decision**: `composeSearch` selects a watcher (R2), binds it with the (now bounded, R4)
bridge to `workspace.root` and the `IpcIndexAdapter`, and returns a `stop()` that closes the
subscription BEFORE `supervisor.stop()`. The Tier-1 build is not awaited: a refresh before the
build simply upserts one file that the build later overwrites with identical facts.

**Rationale**: the supervisor and index are created there and nowhere else; the release path
already funnels through `composed.stop()`. Closing the watcher first means no refresh is
issued against a child that is going away.

**Alternatives considered**: binding in `main()` after composition (rejected: two owners of
the same lifecycle); moving the adapters into the engine child per SDD §6.2 (rejected for this
chunk: a process-boundary redesign for no freshness gain — writes already happen only in the
child, so single-writer holds; recorded as a deviation from the SDD, R8).

## R2 — `watcher.mode` is a nearest-wins enum: auto | native | poll | off

**Decision**: add `"watcher.mode": { kind: "enum", levels: ["auto","native","poll","off"],
default: "auto" }` to `CONFIG_KEYS` — NOT `capped` (it is operational, not security). `auto`
resolves to `poll` when `workspace.detection.networkFs` is true, else `native`. `off` wires
nothing. Session flag `--config watcher.mode=off` works like every other key.

**Rationale**: the adapter's own doc says native watching lies on network mounts, and the
server already decides network-vs-local for the journal mode; reusing that decision keeps one
fact with one source. A forced mode exists for the cases detection cannot see (the detection's
documented v1 limit) and for tests.

## R3 — doctor reads a server-written status record, like plugin health

**Decision**: the server writes `.workbench/watcher.json` (cache-class, gitignored with the
rest of the state dir): `{v:1, pid, mode, adapter, root, startedAtMs, updatedAtMs, stoppedAtMs?,
events, refreshes, removals, failures, lastEventAtMs?, lastError?}` — at subscribe, at most once
per second while events flow, and at stop. doctor's `probeEnvironment` reads it: pid alive ⇒
`live` with the detail; pid dead ⇒ `last session`; absent ⇒ `not active` (mode off, or no
server of this build has run); unreadable ⇒ reported with the file to set aside. Liveness uses
the lock's own rule (`process.kill(pid, 0)`).

**Rationale**: doctor is lock-free and out-of-process by design (004 FR-005), so "live" can
only mean "a record from a process that is still alive". `plugin-health.json` is the shipped
precedent for server-published state that doctor reports.

**Alternatives considered**: doctor connecting to the server (rejected: doctor must never
need a server); reading the tracing ring (rejected: opt-out-able, and indirect).

## R4 — The bridge bounds in-flight refreshes and coalesces per path

**Decision**: `bindWatcherToIndex` gains options `{ maxInFlight = 8, filter?, onStatus? }`.
Events enter a per-path map (arrival-ordered); a newer event for a queued path replaces it, a
removal supersedes a queued refresh; a drain loop keeps at most `maxInFlight` index calls in
flight. Counters feed the status record. The write engine's temp suffix
(`.workbench-tmp-<pid>`) is filtered alongside `.git/` and `.workbench/`.

**Rationale**: `EngineSupervisor.call` rejects with `ENGINE_BUSY` once 128 requests are
pending, and the bridge swallows failures — so a `git checkout` of 2000 files would silently
drop most refreshes and leave those files to synchronous query-time re-parse. Bounding in the
bridge keeps the watcher from ever competing with tool calls for the child's ceiling; 8 leaves
120 slots for requests that have a caller waiting.

**Alternatives considered**: raising `maxPending` (rejected: the ceiling protects the child;
the watcher is background work); retrying on `ENGINE_BUSY` (rejected: backoff storms).

## R5 — The lag is measured on a NEW file, through symbol-mode search

**Decision**: the lag test writes a new file from outside the tools and polls symbol-mode
search (`runSearch` over the real `IpcIndexAdapter`, stub core) until the symbol appears;
records ms for native and polling. The control is the same write with `watcher.mode=off`: the
symbol does not appear until a rebuild.

**Rationale**: query-time verification re-parses files the index already knows, so an edit to
an existing file can look fresh without any watcher; only a NEW file shows the watcher's
contribution. Measuring through the search tool, not just the port, is what the chunk asks
("readable via read/search symbol modes").

## R6 — Idle cost: `process.cpuUsage` deltas over one tree, three configurations

**Decision**: one test builds a 2000-file tree and measures `process.cpuUsage()` (user+system)
over a 1.5 s idle window with no watcher, with the native watcher subscribed, and with the
polling adapter at its default cadence; records all three in the ship trace; asserts only that
native is within a generous bound of none (e.g. ≤ 25 ms extra CPU over the window). The polling
number is recorded as the reason `auto` never chooses it on a local disk.

**Rationale**: Constitution VI wants the number recorded; a tight assertion on CPU time would
be flaky on shared CI, so the assertion is the bar and the record is the evidence.

## R7 — The fuzzer gets a spawned-process mutator and a checkout-style burst

**Decision**: `test/freshness-fuzz.test.ts` gains (a) a mutator variant that runs as a child
process (`bun -e` with the same oracle-encoding content generator) writing files while the
parent queries, and (b) a burst phase that rewrites every file in one tight loop, both for both
adapters; oracles unchanged.

**Rationale**: the existing mutator is already external to the ENGINE (a process boundary is
crossed), but it runs inside the server process; `cargo fmt`, git and IDEs are other processes,
and that is the acceptance wording.

## R8 — SDD deviation recorded: the watcher runs in the server process

**Decision**: record in plan.md that SDD §6.2 places the watcher in the engine child while the
shipped adapters run in the server process and reach the index over IPC; single-writer holds
because writes happen only in the child; revisit only if the IPC hop shows up in the lag.

## R9 — `map`'s watcher-lag line is out of scope; filed

**Decision**: SDD §11.2 lists watcher lag under `map`'s index health; the chunk acceptance does
not. Filed as `chunk:map-watcher-health` (backlog) so the status record gains a second reader
without widening this unit.

## R10 — Code placement

**Decision**: bridge options live in `src/adapters/fswatch/watcher.ts` (the bridge is there);
mode selection + status publishing live in a new `src/protocol/watcher.ts`
(`selectWatcherMode`, `startWatcher`) because the import-graph lint allows protocol → adapters
and not the reverse, and `plugin-health.json` is written from the protocol layer too; doctor's
probe reads the record in `src/doctor/probes.ts`; the key lives in `src/config/schema.ts`.
