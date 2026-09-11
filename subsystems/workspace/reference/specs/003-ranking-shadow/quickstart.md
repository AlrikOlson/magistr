# Quickstart: Ranking-Signal Plugins in Shadow Mode

Runnable validation scenarios proving the feature end to end. Prerequisites: `bun install`,
a built napi artifact (`bun run build:core` or the committed release artifact), and the
committed `plugins/ranking-visibility/guest.wasm`.

## Scenario 1 — Baseline unchanged (SC-001, US1)

```sh
bun test test/ranking.test.ts        # golden ranking suite: must pass UNMODIFIED
bun run eval:rank                    # default run: baseline hash + metrics unchanged
```

Expected: both exit 0 with no golden-file drift — with no ranking plugin installed the
pipeline is bit-for-bit pre-feature.

## Scenario 2 — Shadow by default; promotion applies; demotion reverts (SC-002, US2)

```sh
bun test test/ranking-shadow.test.ts
```

The suite drives the boot lane against a fixture workspace with the visibility signal
installed and asserts, in order: (a) served orderings identical to baseline while
shadow-log entries accumulate; (b) after promotion + cache warm, ordering changes exactly
match applying the logged features through `RANKING_WEIGHTS.plugin`; (c) demotion restores
baseline; (d) cold cache ⇒ zero contribution; (e) unhealthy/removed plugin ⇒ baseline,
no error (FR-009); (f) promotion refused for an unknown signal; (g) componentHash change
resets promotion to shadow.

## Scenario 3 — MRR delta before promotion (SC-003, US3)

```sh
bun run eval:rank --signal plugins/ranking-visibility
```

Expected: one run prints before/after MRR, Hit@1, Hit@5 over `eval/ranking-corpus.jsonl`
(first-party-only vs. with `ranking-visibility/exported` applied at its table weight). A
zero delta is a valid, visible result. Record the pair in the commit that lands the weight.

## Scenario 4 — TCK unmodified, capabilities empty (SC-004, US4)

```sh
bun test test/plugin-tck.test.ts     # includes the ranking-visibility chapter
bun test test/sandbox-escape.test.ts # ranking-lane chapter: no import reachable
```

Expected: TCK passes over `plugins/ranking-visibility/guest.wasm` unmodified; effective
capability set resolves empty; the escape chapter shows the lane links no host imports.

## Scenario 5 — No sandbox on the response path (SC-005)

```sh
bun test test/ranking-shadow.test.ts -t "response path"
```

Expected: the pin asserts the search response resolves before any `contribute` invocation
is observed (instrumented sandbox stub), in both shadow and promoted modes.

## Full gate sweep (Constitution / ship checks)

```sh
bun test && bunx tsc --noEmit && bun run lint && bun run lint:imports \
  && bun run codegen:check && cargo test --manifest-path crates/workbench-core/Cargo.toml
```
