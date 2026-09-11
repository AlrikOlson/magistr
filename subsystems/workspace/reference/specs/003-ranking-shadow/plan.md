# Implementation Plan: Ranking-Signal Plugins in Shadow Mode

**Branch**: `003-ranking-shadow` | **Date**: 2026-08-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/003-ranking-shadow/spec.md`

## Summary

Plumb the blessed `workbench:ranking@0.1.0` interface end to end: a new typed sandbox lane
(batched `contribute` over hit-contexts), a shadow scorer that runs strictly off the search
response path and appends to a size-capped local shadow log, an in-memory feature cache that
lets promoted signals influence the stage-5 scorer through a new `RANKING_WEIGHTS.plugin`
entry (first-party combiner preserved, eval-hash-gated), a persisted per-signal promotion
record orthogonal to the blessing tier, a signal mode for the offline eval harness printing
before/after MRR/Hit@1/Hit@5, and a first-party zero-capability `ranking-visibility` plugin
(no_std carrier recipe) that passes the TCK unmodified. Key research decisions: R1-R7 in
[research.md](research.md); pinned in think:128.

## Technical Context

**Language/Version**: TypeScript (Bun) host; Rust (crates/workbench-core, wasmtime 48
component sandbox); no_std Rust guest for the first-party plugin.

**Primary Dependencies**: existing sandbox host (`sandbox.rs` typed invoke lanes), napi
bindings (`bindings.rs` → `src/adapters/core/loader.ts`), engine-child IPC, blessed WIT
baseline (`wit/ranking.wit` + `wit/baseline.json`, consumed unchanged), plugin boot-lane
precedent (`src/protocol/{resolver,lang-pack,runner}-plugin.ts`).

**Storage**: `.workbench/ranking-promotions.json` (per-signal promotion records,
componentHash-keyed like `plugin-health.json`); `.workbench/ranking-shadow.jsonl`
(size-capped shadow log, oldest-first eviction); in-memory per-process feature cache.

**Testing**: bun test (golden ranking suite unchanged, new shadow/promotion suites, TCK
chapter, escape-suite chapter), cargo test (sandbox lane), `bun run eval:rank` signal mode,
tsc, biome, lint:imports, codegen:check.

**Target Platform**: unchanged (the MCP server's existing platforms).

**Project Type**: single project — host TS + core Rust crate + committed plugin artifact.

**Performance Goals**: zero added latency on the search response path (no sandbox
invocation, pinned by test); shadow scoring is post-response fire-and-forget with the
existing per-plugin fuel/memory policy caps.

**Constraints**: Constitution I (degrade, never block: cold cache/unhealthy plugin ⇒
first-party baseline); Constitution II (ResponseEnvelope unchanged); Constitution V (WIT
baseline byte-equal); Constitution VI (feature weights live in `RANKING_WEIGHTS`, so the
eval weights-hash gate covers them; the shipping change records before/after MRR).

**Scale/Scope**: one first-party signal; the lane, log, cache, promotion store, and eval
mode are generic over any number of installed signals.

## Constitution Check

*GATE: passed pre-Phase-0; re-evaluated post-design — still passing.*

- **I. Match Grep's Cost Structure — PASS.** No sandbox call on any search response path
  (shadow: post-response fire-and-forget; promoted: in-memory cache consult, miss ⇒ zero
  contribution). Plugin failure trips the existing disk-backed breaker; search never errors
  on a ranking-plugin fault. SC-005 pins this by test.
- **II. Rank, Don't Dump — PASS.** Output shape, envelope, budgets, and manifests are
  untouched; only stage-5 scores move, and only through the first-party table.
- **III. Verified Writes — N/A.** No workspace mutation. `.workbench/` state files are
  internal server state (precedent: `plugin-health.json`), not agent-visible writes.
- **IV. Narrow, Typed Escape Hatches — N/A.** No new escape hatches.
- **V. Versioned Contracts, Sandboxed Extensions — PASS.** `workbench:ranking@0.1.0`
  consumed as blessed; baseline byte-equal (WIT census test unchanged); plugin runs in the
  existing component sandbox under the existing capability lattice (empty set here).
- **VI. No Number, No Merge — PASS (load-bearing).** Plugin feature weights are new keys in
  `RANKING_WEIGHTS`, so `scripts/eval-ranking.ts`'s weights-hash gate refuses any change
  that lands without re-recorded MRR. The change shipping the visibility signal's weight
  records its before/after pair (SC-003).

## Project Structure

### Documentation (this feature)

```text
specs/003-ranking-shadow/
├── plan.md              # This file
├── research.md          # Phase 0 output (R1-R7)
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── ranking.md       # workbench:ranking@0.1.0 consumed unchanged
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
plugins/ranking-visibility/          # first-party signal: plugin.toml (zero-cap),
│                                    # guest/ (no_std carrier recipe), guest.wasm (committed)
wit/ranking.wit                      # UNCHANGED (blessed)
crates/workbench-core/src/
├── sandbox.rs                       # + invoke_ranking_contribute (batched typed lane)
└── bindings.rs                      # + rankingContribute napi surface
src/adapters/core/loader.ts          # + RankingGuest* wire types
src/adapters/engine-ipc/
├── engine-child.ts                  # + sandbox/rankingContribute method
└── index.ts                         # + child client surface
src/protocol/
├── ranking-plugin.ts                # boot lane: discovery, breaker, shadow scorer,
│                                    # feature cache, promotion store, shadow log
└── server.ts                        # promotion/demotion + status surface wiring
src/tools/
├── ranking.ts                       # RANKING_WEIGHTS.plugin + pluginFeatures param
│                                    # (pure: caller passes a plain lookup, no I/O)
└── search.ts                        # cache consult into rankGroups; post-response hook
scripts/eval-ranking.ts              # + signal mode (before/after MRR/Hit@k)
test/
├── ranking-shadow.test.ts           # shadow/promotion/degradation suites
├── plugin-tck.test.ts               # + visibility signal chapter
└── sandbox-escape.test.ts           # + chapter for the ranking lane
```

**Structure Decision**: mirrors the three shipped plugin lanes exactly (guest → typed
sandbox lane → napi → engine-child → protocol boot lane → main.ts composition); the only
novel modules are the shadow scorer/log/cache/promotion store, all inside
`src/protocol/ranking-plugin.ts` beside its siblings.

## Complexity Tracking

No constitution violations to justify.

## Deviations

1. **Promotion surface (T014)**: no tool-call mutation verb was added. Promotion is a HUMAN
   decision (§8.3: "until the user promotes it"), and a tool argument would let an agent
   self-promote a signal — so the delivered surface is the user-editable
   `.workbench/ranking-promotions.json` plus the runtime's promote/demote verbs (used by
   the suites and any future CLI), with per-signal state reported through
   `workbench_status` notes (the SDD §5.2 schema-free channel) instead of new schema fields.
2. **Zero-delta pin (T017)**: no separate test pins the zero-delta case; the recorded
   `eval:rank --signal plugins/ranking-visibility` run is the evidence — before MRR 0.9375 /
   Hit@1 0.875 / Hit@5 1, after identical. Honest negative: the 16-query corpus contains no
   tie that the `exported` feature breaks, so the signal's corpus delta is zero. The mode
   itself (and its delta reporting) is exercised end to end over the real component.
3. **Lane hardening (beyond plan)**: `invoke_ranking_contribute` is a define-nothing carrier
   lane — the subprocess interface is not linkable even when granted (lang-pack precedent),
   pinned by the new escape-suite chapter. The plan only promised grant-gating.
4. **Weights-hash re-record (predicted, think:130 H2)**: adding the `plugin` table to
   `RANKING_WEIGHTS` changed the hash; the baseline was re-recorded via the sanctioned
   `--record` flow with identical metrics (no features flow in the default run).
5. **Sequencing**: T007's contribution hygiene landed with the module skeleton during the
   weights phase rather than the boot-lane phase — content as planned, order not.
