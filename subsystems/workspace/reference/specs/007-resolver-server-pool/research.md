# Phase 0 Research: Resolver Language-Server Session Pool

All decisions grounded in the repository — `crates/workbench-core/src/sandbox.rs`
(SubprocessPool, invoke_resolver_references), `plugins/lsp-resolver/guest/src/lib.rs`,
`wit/subprocess.wit` + `wit/README.md` + `crates/workbench-core/tests/wit_contracts.rs`,
`src/protocol/resolver-plugin.ts`, `src/adapters/engine-ipc/{engine-child,supervisor}.ts`,
`src/tools/plugins.ts` (effectiveCapabilities), `test/statelessness.test.ts`,
`test/resolver-plugin.test.ts` + `test/support/fake-lsp.ts`, `Cargo.lock`. Pinned in
think:150 (open) and think:151 (close). No external research was required: the only outside
facts used are the LSP rule that `initialize` may be sent once per connection and the per-OS
process-lifetime primitives, both stated in the decisions below.

## R1 — The pool lives in the engine child's sandbox module, process-global, keyed by request identity

**Decision**: A `ServerPool` (a `OnceLock<Mutex<…>>` in `sandbox.rs`, the same pattern as
the Wasmtime `engine()` singleton) owns live sessions for the lifetime of the engine child
process, keyed by `(workspace_root, language_id, bin, args)` — the detection spec that
already travels on every `sandbox/resolver` request plus the root. The existing per-invoke
`SubprocessPool` inside `ComponentState` becomes a **lease view**: `open` leases a pooled
session (or spawns and leases), reads/writes go to the leased session, and invoke end hands
every still-leased session back to the pool or kills it (R4).

**Rationale**: the chunk asks for "a deliberate home in engine-child infrastructure". The
engine child is the one long-lived process that already hosts the sandbox and already
caches process-lifetime substrate (the Wasmtime engine, the qjs bytes). The key is exactly
the identity the request carries, so statelessness holds by construction (R7): no request
reaches into the pool with anything it did not bring. Different roots and different specs
never collide.

**Alternatives considered**: a pool in the TS host process holding child processes and
proxying frames to the engine child (rejected: frames would cross IPC twice, and the
sandbox's kill-on-fuel/trap guarantee lives in Rust next to the Store); a separate
long-lived "resolver host" process (rejected: the constitution pins one engine child crash
domain for third-party code, and a new process is a new crash domain and a new supervisor);
leaving sessions per-invoke and raising the per-request budget (rejected: the budget is the
agent-facing latency ceiling; a 10 s budget means a 10 s references call every time).

## R2 — The guest learns "reused" and can "release"; the host stays protocol-unaware

**Decision**: Two additive functions in `workbench:subprocess`:
`reused: func(session: u32) -> bool` (true when the handle is a warm session opened by an
earlier invoke) and `release: func(session: u32)` (return to the pool without killing).
`close` keeps its kill semantics. The guest skips `initialize`/`initialized` when
`reused` is true; it calls `close` only when it judges the server unusable (dead, read
timeout, malformed/error reply on the handshake-dependent requests); on success it leaves
the session for invoke-end return (or calls `release`).

**Rationale**: LSP forbids a second `initialize` on one connection (servers answer an
error), so a reused session must skip the handshake — a decision only the protocol speaker
can make, and the protocol speaker is the guest (wit/subprocess.wit: "transport, not
protocol"). `reused` is a host fact (lease history), not a protocol fact, so exposing it
keeps the host transport-only. `release` is cheap, explicit, and lets a guest hand back a
session before doing unrelated work; invoke-end return covers guests that do not call it.

**Alternatives considered**: host intercepts and answers a second `initialize` from a cached
result (rejected: makes the host an LSP client; D2 of feature 001 put the protocol in the
guest for exactly this reason); guest probes the server ("send initialize, treat
'already initialized' as success") (rejected: server-specific error text sniffing, the
thing FR-007 of feature 001 forbids host-side and the guest should not do either); encoding
warmth in `specs()` strings (rejected: a hack on a list of language ids).

## R3 — Additive WIT bump: `workbench:subprocess@0.1.0 → @0.2.0`; pooling is opt-in by version; grants match within a major

**Decision**: The two new functions move the package stamp to `0.2.0` (wit/README rule 2:
additive within a major with a version bump; the gate treats 0.1.0→0.2.0 as a move within
major 0, no re-bless). The guest's world imports `workbench:subprocess/subprocess@0.2.0`, so
the grant name moves with it. The host:

- defines the subprocess instance on the linker under **the name the guest imports**
  (`@0.1.0` or `@0.2.0`); a `@0.1.0` import gets the **per-invoke** pool exactly as today
  (spawn on open, kill on close, kill at invoke end, `reused` never offered); a `@0.2.0`
  import gets the lease view over the process-global pool;
- in `effectiveCapabilities` (TS), treats a grant as covering the same
  `package/interface@major` (so records carrying `…@0.1.0` still authorize `…@0.2.0`), and
  applies the blessed-tier rule to every version of the subprocess capability;
- keeps `CAP_SUBPROCESS`/`SUBPROCESS_IMPORT` as the **current** name (`@0.2.0`) and adds a
  `isSubprocessImport(name)` / `is_subprocess_import(name)` predicate that accepts any
  `workbench:subprocess/subprocess@0.x` for gating.

**Rationale**: additive-within-major is the project's own rule for every contract; the
import name carrying the version is the SDK's design (world import names are the grant
vocabulary), so honoring it means serving both names. Version-keyed opt-in keeps a
third-party 0.1.0 guest byte-for-byte unchanged (it never learns `reused`, so it would
re-initialize a warm session — giving it per-invoke sessions is the only correct reading).
Same-major grant matching is the grant-side reading of the same rule; the trust shape of
the capability did not change (still blessed-only, still host-curated specs).

**Alternatives considered**: a new package `workbench:subprocess-pool@0.1.0` holding the
two functions (rejected: two packages for one capability, two grants for one trust
decision, and `wit/README` designed per-package stamps for exactly this evolution);
relying on Wasmtime's semver-aware import matching (rejected: 0.1→0.2 is not
semver-compatible, and the grant check is a string match in two languages — explicit is
safer); bumping to `1.0.0` (rejected: that is the deliberate re-bless act, and nothing
here is breaking).

## R4 — Lease ending decides warm return versus kill; kill is the default

**Decision**: `invoke_resolver_references` (and `invoke_component_json` for the `@0.2.0`
case) calls `pool_view.finish(outcome_ok)` before the Store drops: on `Ok` every still-leased
session returns warm (its lease cleared, its idle clock reset); on fuel exhaustion, trap,
memory cap, invalid guest, or any error path every leased session is killed. `Drop` of the
lease view (the path when nobody called `finish`, e.g. a panic) kills. `close` always kills
and removes the pooled entry.

**Rationale**: a guest that died mid-frame leaves the server's conversation in an unknown
state (half-written frame, pending reply); reusing it would hand the next invoke a poisoned
session. Kill-by-default means the only way a session survives is the explicit success path,
which is the chunk's "teardown guarantees survive" made structural.

**Alternatives considered**: always return warm and let the next guest discover breakage
(rejected: turns one crash into two failed requests); per-language "suspect" flags with a
protocol ping (rejected: host would need protocol knowledge).

## R5 — Teardown across the engine child's endings: explicit shutdown + OS-level kill-on-parent-death

**Decision**: Layers, all required:

1. **Normal child exit** (stdin EOF ends the frame loop in `engine-child.ts`): the child
   calls a new binding `sandboxServerPoolShutdown()` after `Promise.all(inFlight)` — Rust
   statics are never dropped at process exit, so the kill is explicit.
2. **Abrupt child death** (supervisor `stop()` uses `proc.kill()`, crash restarts, chaos
   `process.exit`): on **Windows** the pool creates one Job Object with
   `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and assigns every spawned server to it; the job
   handle lives in the pool, so when the process dies for any reason the OS closes it and
   kills the job. On **Linux** every server is spawned with
   `prctl(PR_SET_PDEATHSIG, SIGKILL)` in `pre_exec`; because the death signal is tied to the
   spawning **thread**, spawns run on one dedicated, process-lifetime pool thread (a
   `std::thread` fed by a channel), never on a napi worker thread. On **macOS** there is no
   equivalent primitive; the guarantee rests on the server exiting when its stdin closes
   (rust-analyzer and typescript-language-server both do), recorded as a named gap; the
   abrupt-death test skips on darwin with that reason.
3. **Idle limit**: a reaper (lazy sweep on every pool access plus a low-frequency ticker
   thread) kills unleased sessions idle longer than `SERVER_IDLE_TTL_MS` (300 000 ms).
4. **Lease ending** (R4) for the guest-side endings.

Dependencies: `windows-sys` (already in `Cargo.lock` at 0.61.2, transitively) under
`[target.'cfg(windows)'.dependencies]` with the `Win32_System_JobObjects`,
`Win32_Foundation`, `Win32_System_Threading` features; `libc` (already in the lock at
0.2.189) under `[target.'cfg(target_os = "linux")'.dependencies]`. Zero new crates in the
lock (SC-006).

**Rationale**: the supervisor's own shutdown is a hard kill, so destructors cannot be the
guarantee; only an OS-held handle or a kernel death signal survives a `process.exit`. The
stdin-EOF behavior of LSP servers is a belt worth naming, not a guarantee worth claiming.

**Alternatives considered**: a pid registry in `.workbench/` reaped by the next engine
child (rejected for v1: pid reuse makes a blind kill unsafe; revisit only if the macOS gap
shows up in dogfood); a per-server watchdog env var (rejected: third-party servers do not
honor it); process groups + `killpg` (rejected: needs a live sender, which a dead child is
not).

## R6 — Idle drain, busy refusal, and the budget-expired lease

**Decision**: Each session's reader thread keeps reading the server's stdout forever (so
the pipe never fills); an `AtomicBool leased` decides whether a de-framed payload is queued
for the guest or dropped. At lease start the queue is drained (empty read buffer, FR-004).
A lease request for a held session answers `spawn-refused("session leased by another
invoke")` — the `open-error` variant cases are frozen (wit/README rule 3), so no new case,
and `spawn-refused` is the honest existing one. When the TS host's budget expires, the
child invoke is not interrupted (no epoch interruption is configured and a mid-frame kill
would poison the session); it finishes on its own — bounded by the guest's per-frame read
timeout — and its normal end returns the session warm, so the work the slow server did is
not thrown away.

**Rationale**: rust-analyzer emits progress/diagnostic notifications continuously; an
unbounded queue would grow while idle and a bounded one would block the server. Busy must
not wait (feature 001 US2). Not interrupting the stuck invoke is what makes the pool pay
off on the very next request.

**Alternatives considered**: per-lease bounded channel with backpressure (rejected: blocks
the server); a second concurrent session per key (rejected: two rust-analyzers on one
workspace is the memory failure mode this chunk is trying to avoid).

## R7 — Statelessness reading: a cache of capability substrate, not session state

**Decision**: The pool is classed with the Wasmtime engine singleton and the qjs bytes
cache: process-lifetime infrastructure addressed only by request-carried identity. No per-
client, per-session, or per-request data lives in it; a cold pool answers every request
(at most with a symbolic answer, which is already the contract when the upgrade is not
available); the circuit breaker and every health fact stay on disk. The statelessness suite
(fresh server processes, byte-identical envelopes over `workbench_status` and write-path
calls) is unaffected and MUST pass unmodified (SC-003).

**Rationale**: Constitution V's "no in-memory session state" targets request meaning and
identity (negotiation per request via `_meta`), not whether a process caches a warmed
engine; the resolver's availability was already time-varying (budget, breaker expiry) before
this feature, and provenance labels make that honest.

## R8 — Test rig: the fake server grows `slow-init` and an initialize counter; the real number comes from rust-analyzer, environment-gated

**Decision**: `test/support/fake-lsp.ts` gains modes: `slow-init:<ms>` (delay the
`initialize` reply), `count` (report the number of `initialize` requests seen inside the
`workspace/symbol` response and in a sidecar file named by argv), and `ignore-eof` (keep
running after stdin closes, so the OS-level kill is what the orphan test proves). The
recorded real-server scenario (SC-005) drives rust-analyzer through the detection **test
seam** over a fixture crate in a temp dir: cold call, warm call, handshake count,
latencies printed and recorded via `ship_record`; `describe.if(which rust-analyzer && which
cargo)` with a skip message naming both binaries. typescript-language-server is absent on
this machine, so no number is claimed for it.

**Rationale**: feature 001 D7 — the fake is the deterministic rig, the real server is the
last mile; Constitution VI wants a number, the run parameters want honesty about which
binary produced it.

## R9 — Resolver outcome gains additive session counts

**Decision**: `ResolverOutcome` (Rust) → `SandboxResolverOutcome` (napi) → the
`sandbox/resolver` IPC result → `ResolverGuestOutcome` (TS) gain
`sessions: { opened: number, reused: number }` (optional on the TS type for the IPC-less
test rigs). `PluginResolver` ignores it today beyond typing; tests pin reuse at the host
boundary with it.

**Rationale**: FR-013 — reuse observable without inspecting processes; additive fields on
an IPC result and a napi object are within-major evolution.
