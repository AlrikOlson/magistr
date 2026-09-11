# Tasks: Ranking-Signal Plugins in Shadow Mode

**Input**: Design documents from `/specs/003-ranking-shadow/`
**Prerequisites**: plan.md, research.md (R1-R7), data-model.md, contracts/ranking.md, quickstart.md

**Organization**: grouped by user story (US1 features-only leash, US2 shadow/promotion, US3
MRR delta, US4 first-party plugin conformance). Tests are integral here (Constitution: golden
suites, TCK, pinned behavior), so test tasks are included per story.

## Phase 1: Setup — the first-party signal artifact

- [x] T001 Create plugin skeleton `plugins/ranking-visibility/` — `plugin.toml` (zero
      capabilities, limits per lattice), `guest/` no_std carrier-recipe crate targeting the
      `ranking-provider` world with `wit/deps/ranking/ranking.wit` copied unchanged, and
      `README.md` documenting the one feature `exported` per contracts/ranking.md.
- [x] T002 Implement the guest's `contribute` in `plugins/ranking-visibility/guest/src/lib.rs`
      (hand-rolled export/pub/public declaration detection over the excerpt, declaration
      kind-class only, values 0.0/1.0), componentize, and commit
      `plugins/ranking-visibility/guest.wasm`.

## Phase 2: Foundational — the typed sandbox lane (blocks all stories)

- [x] T003 Add `invoke_ranking_contribute` to `crates/workbench-core/src/sandbox.rs`:
      instantiate once, call `contribute` per hit-context batch member (Val API lowering,
      post_return between calls), returning per-hit feature lists; unit tests over the
      committed guest in `crates/workbench-core/src/sandbox.rs` `#[cfg(test)]` or
      `tests/ranking_lane.rs`.
- [x] T004 Expose the lane as `rankingContribute` napi in
      `crates/workbench-core/src/bindings.rs` and add `RankingGuest*` wire types to
      `src/adapters/core/loader.ts` (Option::None lowers as undefined — `?: T | null`).
- [x] T005 Plumb the engine-child method `sandbox/rankingContribute` in
      `src/adapters/engine-ipc/engine-child.ts` and the client surface in
      `src/adapters/engine-ipc/index.ts`.

## Phase 3: US1 — features only, first-party combiner preserved (P1)

**Goal**: plugin values reach scores only through `RANKING_WEIGHTS.plugin`; baseline
bit-for-bit unchanged without plugins.
**Independent test**: golden ranking suite passes unmodified; applying/removing a weight
reproduces/restores orderings exactly.

- [x] T006 [US1] Add `plugin` weights to `RANKING_WEIGHTS` and an optional pure
      `pluginFeatures` lookup parameter threaded through `orderGroups`/`rankGroups` in
      `src/tools/ranking.ts` (no I/O — caller supplies a plain map; absent ⇒ identical
      code path to today).
- [x] T007 [P] [US1] Host-side contribution hygiene in `src/protocol/ranking-plugin.ts`:
      namespace prefixing (`<plugin>/<feature>`), finite-value validation (drop |v| > 1e6,
      NaN/Inf), malformed-batch health accounting (data-model.md FeatureContribution).
- [x] T008 [US1] Pin US1 in `test/ranking-shadow.test.ts`: golden suite untouched (SC-001),
      weighted-feature application is exactly `Σ weight·value` on stage-5 scores, weight
      removal restores baseline order; `bun run eval:rank` default run unchanged.

## Phase 4: US2 — shadow by default, explicit promotion (P2)

**Goal**: install ⇒ shadow (logged, not applied); promote ⇒ applied via warm cache;
demote/unhealthy/hash-change ⇒ baseline.
**Independent test**: quickstart Scenario 2 sequence (a)-(g).

- [x] T009 [P] [US2] Promotion store `.workbench/ranking-promotions.json` in
      `src/protocol/ranking-plugin.ts`: componentHash-keyed records, absent ⇒ shadow,
      hash-change auto-demotes (data-model.md PromotionRecord transitions).
- [x] T010 [P] [US2] Shadow log writer in `src/protocol/ranking-plugin.ts`:
      `.workbench/ranking-shadow.jsonl`, 5 MB cap with oldest-entry eviction, queryHash not
      raw query, swallowed append failures (data-model.md ShadowLogEntry).
- [x] T011 [P] [US2] In-memory feature cache in `src/protocol/ranking-plugin.ts`: HitKey
      (file, line, column, excerptHash, kindClass, fileClass) → namespaced contributions,
      LRU capacity 50k, synchronous read path only.
- [x] T012 [US2] Boot lane in `src/protocol/ranking-plugin.ts`: `ranking-` package
      discovery, manifest parse, jail-checked componentInfo, disk-backed breaker reuse,
      effectiveCapabilities (must resolve empty for zero-cap), post-response shadow scorer
      (fire-and-forget batched lane call → cache + log), RankingSignalStatus observability.
- [x] T013 [US2] Wire the seam: `src/tools/search.ts` consults the cache into a
      `pluginFeatures` map for promoted healthy signals before `rankGroups` and fires the
      shadow scorer after the response is enveloped; compose in `src/main.ts` after
      pluginHostDeps (fire-and-forget boot, resolver/lang-pack precedent).
- [x] T014 [US2] Promotion/demotion + status surface: expose promote/demote verbs and
      per-signal state (tier, promoted, healthy, cache stats) through the server's existing
      status/config surface in `src/protocol/server.ts` (refuse unknown signal).
- [x] T015 [US2] US2 suite in `test/ranking-shadow.test.ts`: scenarios (a)-(g) from
      quickstart Scenario 2 plus the SC-005 response-path pin (instrumented sandbox stub —
      response resolves before any contribute call, both modes).

## Phase 5: US3 — MRR delta before promotion (P3)

**Goal**: one offline run prints before/after MRR/Hit@1/Hit@5 for a signal.
**Independent test**: quickstart Scenario 3.

- [x] T016 [US3] Add `--signal <plugin-dir>` mode to `scripts/eval-ranking.ts`: load the
      component through loader.ts, compute contributions for all corpus candidate hits,
      rank corpus twice (baseline vs. +features at table weight), print the before/after
      triple; default no-flag run byte-identical (baseline gate untouched).
- [x] T017 [US3] Run the signal mode over `plugins/ranking-visibility`, record the
      before/after pair (goes in the shipping commit message per Constitution VI), and pin
      the zero-delta-is-visible case in `test/ranking-shadow.test.ts` or a script test.

## Phase 6: US4 — TCK unmodified, capabilities empty (P4)

**Goal**: the shipped artifact passes conformance a third party would face.
**Independent test**: quickstart Scenario 4.

- [x] T018 [US4] Add the ranking-visibility chapter to `test/plugin-tck.test.ts`
      (fuzz-style excerpt inputs never trap, purity, empty effective capability set) and
      the ranking-lane chapter to `test/sandbox-escape.test.ts` (no host import linkable,
      granted or not).

## Phase 7: Polish & cross-cutting

- [x] T019 Sync `specs/003-ranking-shadow/` with reality: tick tasks, record deviations in
      plan.md Deviations, align quickstart commands to real test filter names.
- [x] T020 Full server-verified gate sweep (bun test, tsc, biome, lint:imports,
      codegen:check, cargo test incl. WIT census byte-equal) and file any out-of-scope
      discoveries as backlog chunks (never into this diff).

## Dependencies

- Phase 1 → Phase 2 (T003's tests need the committed guest) → Phase 3 → Phase 4 → Phase 5/6 → Phase 7.
- US3 (T016-T017) depends on US1 (weights) + Phase 2 (lane), not on US2.
- US4 (T018) depends only on Phase 1 artifact + Phase 2 lane; can run parallel to US2/US3.
- Within Phase 4: T009/T010/T011 are [P] (distinct modules, no cross-deps); T012 needs all
  three; T013 needs T012; T014 needs T009+T012; T015 needs T013+T014.

## Parallel example

After T008: `T009 ∥ T010 ∥ T011` then T012→T013→T014→T015, with `T016 ∥ T018` alongside
Phase 4 once the lane (T005) and weights (T006) exist.

## Implementation strategy

MVP = Phase 1-3 (US1): the leash exists and baseline is provably unchanged. Increment 2 =
US2 (the shadow/promotion product). Increment 3 = US3 + US4 (the number and the gate
obligation). Ship gates run at Phase 7 regardless of increments.
