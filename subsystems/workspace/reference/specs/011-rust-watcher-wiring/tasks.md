# Tasks: Watcher Wiring

**Input**: Design documents from `specs/011-rust-watcher-wiring/` (plan.md, research.md
R1–R10, data-model.md, quickstart.md). Substrate pinned in think:164/165.

**Tests**: included — every success criterion is pinned by a test or a recorded number
(Constitution VI).

**Organization**: phases map to the spec's user stories; each story is independently testable.
Execute in order; do not re-derive decisions recorded in research.md.

## Phase 1: Foundational — the bounded bridge (blocking)

- [x] T001 `src/adapters/fswatch/watcher.ts`: extend `bindWatcherToIndex(watcher, index, root,
      options?)` per data-model.md — `maxInFlight` (default 8), a per-path arrival-ordered queue
      with coalescing (newer replaces queued; removal supersedes refresh), a drain loop that
      keeps ≤ maxInFlight calls in flight, `ignore` (default: the write engine's
      `.workbench-tmp-` suffix) on top of the `.git/`/`.workbench/` filter, `onEvent`/`onSettled`
      hooks, and an `idle()` promise on the returned subscription (R4). Existing callers unchanged.
- [x] T002 `test/fswatch.test.ts`: pins over the recording index + manual watcher — N=50 events
      with maxInFlight=3 never exceed 3 in flight and all complete; two events for one path drain
      as one refresh; removal after a queued refresh yields only the removal; temp-file events
      are ignored; the existing routes/degrades/filters test still passes (SC-002 bound).

## Phase 2: Foundational — mode + status (blocking)

- [x] T003 `src/config/schema.ts`: add `"watcher.mode"` (enum auto|native|poll|off, default
      auto, not capped) (R2); confirm the config tests that enumerate keys/trace order still
      pass or extend them deliberately.
- [x] T004 New `src/protocol/watcher.ts`: `selectWatcherMode(setting, detection)` →
      `{ mode, adapter: "fs.watch" | "polling" | null }`; `startWatcher({ root, stateDir, index,
      mode, detection, now })` → `{ close(): Promise<void> } | null` that builds the adapter,
      binds it through the bridge with status hooks, writes `.workbench/watcher.json` at start,
      throttled (≥ 1 s) on events, and at stop with `stoppedAtMs` (R3); never throws into the
      caller (a failure to subscribe is recorded in the file and returns null).
- [x] T005 `src/main.ts` `composeSearch`: resolve `workspace.config.effective["watcher.mode"]`,
      call `startWatcher`, and make `stop()` close it BEFORE `supervisor.stop()` (R1).

## Phase 3: User Story 1 — external edits reach the index (P1) 🎯 MVP

- [x] T006 [US1] `test/watcher-wiring.test.ts` (new): mode selection pins — auto+local ⇒
      fs.watch, auto+network ⇒ polling, native/poll forced, off ⇒ null (SC-006); status record
      written at start and updated at stop; `watcher.mode=off` ⇒ no record, no subscription.
- [x] T007 [US1] Lag measurement (R5): over the fuzzer's rig (EngineSupervisor + IpcIndexAdapter,
      stub core) with `startWatcher`, write a NEW file from a spawned process, poll `runSearch`
      symbol mode until the symbol appears; record ms for fs.watch and polling via `ship_record`;
      the off control does not see it; deletion removes it (SC-001).

## Phase 4: User Story 2 — bursts and idle (P1)

- [x] T008 [US2] Burst (R4/FR-009): write 2000 files at once over the real rig; assert all
      indexed, zero `ENGINE_BUSY` observed (hook or supervisor counter), record settle time
      (SC-002).
- [x] T009 [US2] Idle (R6/FR-010): 2000-file tree; `process.cpuUsage()` deltas over a 1.5 s idle
      window for none / fs.watch / polling; record all three; assert native ≤ none + 25 ms
      (SC-003).

## Phase 5: User Story 3 — doctor (P2)

- [x] T010 [US3] `src/doctor/probes.ts` `probeEnvironment`: replace the fixed line with the
      record reader — live (pid alive) / last-session / not-active / unreadable — detail per
      data-model.md; `test/doctor.test.ts` pins all four states (SC-004).

## Phase 6: User Story 4 — fuzzer (P2)

- [x] T011 [US4] `test/freshness-fuzz.test.ts`: a spawned-process mutator variant (`bun -e`
      writing the same oracle-encoded content) and a checkout-style burst phase, both adapters;
      oracles unchanged (SC-005).

## Phase 7: Polish

- [x] T012 Update `plan.md` Deviations (incl. R8 SDD placement), tick this file, file
      follow-ups (map watcher-health line is already filed as chunk:map-watcher-health).
- [x] T013 Server-verified gate sweep via `ship_check` (bun test, tsc, lint quartet, cargo
      test), `ship_finalize`, `roadmap_complete_chunk`, commit.
