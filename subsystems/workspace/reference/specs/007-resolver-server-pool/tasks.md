# Tasks: Resolver Language-Server Session Pool

**Input**: Design documents from `specs/007-resolver-server-pool/` (plan.md, research.md
R1–R9, data-model.md, contracts/subprocess-pool.md, quickstart.md). Substrate pinned in
think:150/151.

**Tests**: included — every acceptance criterion is pinned by a test or a gate exit code
(Constitution VI).

**Organization**: phases map to the spec's user stories; every story independently testable.
Execute in order; do not re-derive decisions recorded in research.md.

## Phase 1: Setup

- [x] T001 Contract first: edit `wit/subprocess.wit` to `package workbench:subprocess@0.2.0`
      adding `reused: func(session: u32) -> bool` and `release: func(session: u32)` with the
      doc comments from contracts/subprocess-pool.md §1 (existing items untouched); copy
      byte-for-byte to `plugins/lsp-resolver/guest/wit/deps/subprocess/subprocess.wit`;
      update `plugins/lsp-resolver/guest/wit/world.wit` to import `…@0.2.0`; run
      `cargo test --manifest-path crates/workbench-core/Cargo.toml --test wit_contracts`
      WITHOUT bless and confirm it passes (if it demands a bless, stop: the change is not
      additive) — commit the moved `wit/baseline.json` only if the gate itself rewrote it
- [x] T002 Add target-scoped dependency edges in `crates/workbench-core/Cargo.toml`:
      `[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.61", features =
      ["Win32_Foundation","Win32_System_JobObjects","Win32_System_Threading"] }` and
      `[target.'cfg(target_os = "linux")'.dependencies] libc = "0.2"`; verify `Cargo.lock`
      gains no new crate (`git diff --stat Cargo.lock` shows only edge changes)

## Phase 2: Foundational (blocking prerequisites)

- [x] T003 In `crates/workbench-core/src/sandbox.rs` implement `ServerPool` per data-model.md:
      `OnceLock<Mutex<ServerPool>>`, `PoolKey`, `PooledSession` with reader thread +
      `AtomicBool leased` + queue, `lease(key)` (drain queue on lease; busy ⇒
      `SubprocessOpenError::SpawnRefused("session leased by another invoke")`; dead child
      ⇒ reap + respawn), `return_warm(key)`, `kill(key)`, `sweep_idle(now)` with injectable
      clock and `SERVER_IDLE_TTL_MS = 300_000`, `shutdown()`, `stats()`; the spawner runs on
      one dedicated process-lifetime thread (R5)
- [x] T004 OS kill group in `sandbox.rs`: Windows — one Job Object with
      `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` created lazily by the pool, every spawned server
      assigned; Linux — `pre_exec` with `prctl(PR_SET_PDEATHSIG, SIGKILL)` on the spawner
      thread; other targets — no-op with a doc comment naming the gap (R5)
- [x] T005 Re-scope the per-invoke `SubprocessPool` into a `LeaseView` with
      `mode: PerInvoke | Pooled`; `is_subprocess_import(name)`; `define_subprocess(linker,
      import_name, mode)` linking `reused`/`release` only for `@0.2.0`; in
      `invoke_resolver_references` and `invoke_component_json` pick the mode from the
      component's actual subprocess import name and call `finish(ok)` before the Store
      drops (ok ⇒ return warm, else kill; `Drop` without `finish` kills) (R2/R3/R4)
- [x] T006 `ResolverOutcome` gains `sessions_opened`/`sessions_reused`; `bindings.rs`
      mirrors them on `SandboxResolverOutcome` and adds
      `sandbox_server_pool_shutdown()` / `sandbox_server_pool_stats()`; `src/adapters/core/
      loader.ts` types them (optional methods, like `sandboxResolveReferences`);
      `src/protocol/resolver-plugin.ts` `ResolverGuestOutcome.sessions?` (R9)
- [x] T007 Rust unit tests in `sandbox.rs` over a scripted server (the existing WAT probes +
      a tiny stdio script): lease/return/reuse across two invokes with `reused` true on the
      second; non-ok lease end (fuel exhaustion, trap) kills; `close` kills; busy lease
      refused typed; idle drain (notifications between invokes do not reach the next
      lease); idle TTL with injected clock; `shutdown` kills all; `@0.1.0` importer keeps
      per-invoke semantics and cannot link `reused`

## Phase 3: User Story 1 — A slow real server contributes on the second request (P1) 🎯 MVP

**Goal**: warm reuse end to end through the bundled guest.

**Independent Test**: quickstart Scenario 1.

- [x] T008 [US1] Update the guest `plugins/lsp-resolver/guest/src/lib.rs`: on `open`, if
      `sp::reused(session)` skip `initialize`/`initialized`; on any protocol failure that
      makes the session unusable (write/read error, read timeout, error reply) call
      `sp::close`; on success do NOT close (leave for invoke-end return); `invoke` status
      JSON gains `"surface": "workbench:resolver@0.1.0"` unchanged and
      `"subprocess": "workbench:subprocess@0.2.0"`; rebuild `guest.wasm` with
      `plugins/lsp-resolver/build.ps1`; update `plugin.toml` imports to `…@0.2.0`
- [x] T009 [US1] Extend `test/support/fake-lsp.ts`: modes `slow-init:<ms>`, `count`
      (initialize count echoed in the `workspace/symbol` result and appended to a sidecar
      file from argv), `ignore-eof`; keep `normal`/`crash`/`hang`
- [x] T010 [US1] `test/resolver-plugin.test.ts` new "pool" chapter over the live lane: two
      `PluginResolver.resolve` calls with `slow-init` > budget and an injected clock that
      jumps past `RESOLVER_RETRY_AFTER_MS` — first `[]` + one budget breach, second
      `resolver-lsp` edges, fake initialize count 1, `sessions.reused == 1`; a `crash`
      between calls ⇒ the second call respawns (`sessions.opened == 1`)

## Phase 4: User Story 2 — Every server process dies when it should (P1)

**Goal**: every ending is a test.

**Independent Test**: quickstart Scenario 2.

- [x] T011 [US2] `src/adapters/engine-ipc/engine-child.ts`: after `await Promise.all([...
      inFlight])` call `binding.sandboxServerPoolShutdown?.()` (guarded; the wasm-fallback
      lane lacks it)
- [x] T012 [US2] New `test/server-pool.test.ts`: spawn the real engine child through
      `EngineSupervisor`, warm a session with the `ignore-eof` fake (pid written to a
      sidecar), then (a) `supervisor.stop()` ⇒ fake pid gone within a bounded wait; (b)
      stdin closed ⇒ gone; (c) `describe.if(process.platform !== "darwin")` hard kill of the
      child process ⇒ gone, with a skip message naming the macOS gap on darwin
- [x] T013 [US2] Rust: `host_survives_every_kill_in_sequence`-style test extended so a warm
      session survives a clean run, dies on a fuel kill, and a clean run still works after

## Phase 5: User Story 3 — Stateless and compatible (P2)

**Goal**: nothing else moves.

**Independent Test**: quickstart Scenario 3.

- [x] T014 [US3] `src/tools/plugins.ts`: `SUBPROCESS_IMPORT = "…@0.2.0"`,
      `isSubprocessImport(name)`, `effectiveCapabilities` matches grants by
      `package/interface@major` and applies the blessed-tier rule to any subprocess version;
      update `test/sandbox-escape.test.ts` / `test/sandbox-host.test.ts` literals that name
      the manifest import to the current name, keeping the sideload refusals; add an
      `effectiveCapabilities` test: `@0.1.0` grant authorizes the `@0.2.0` manifest import
      for a blessed record, still refused for a sideload
- [x] T015 [US3] Run `bun test test/statelessness.test.ts`, `bun test
      test/plugin-tck.test.ts`, `bun test test/sandbox-escape.test.ts`, and
      `cargo test --test wit_contracts` (no bless) and record each via `ship_check`

## Phase 6: User Story 4 — The claim carries a number (P3)

**Goal**: cold vs warm for one real server.

**Independent Test**: quickstart Scenario 4.

- [x] T016 [US4] `test/resolver-plugin.test.ts` "real server" chapter:
      `describe.if(Bun.which("rust-analyzer") && Bun.which("cargo"))` with a skip message
      naming both binaries; fixture crate in a temp dir (lib with `pub fn greet()` and a
      second file calling it); spec `{ languageId: "rust", bin: rust-analyzer, args: [] }`
      through the test seam; `budgetMs` generous for the cold call; print
      `cold=<ms> warm=<ms> handshakes=<n>`; assert `sessions.reused == 1` on the warm call
      and warm < cold; the build unit records the printed numbers with `ship_record`

## Phase 7: Polish

- [x] T017 Update `wit/README.md` table row for `workbench:subprocess` (0.2.0, pooled
      sessions), `specs/007-resolver-server-pool/plan.md` Deviations, tick this file,
      and `skills/workbench-routing/SKILL.md` only if a routed surface changed (it should
      not; `bun run lint:skill` decides)
- [x] T018 Full gate sweep via `ship_check` with real commands (bun test, tsc, lint trio +
      lint:skill, cargo test), `ship_finalize`, `roadmap_complete_chunk`, commit
