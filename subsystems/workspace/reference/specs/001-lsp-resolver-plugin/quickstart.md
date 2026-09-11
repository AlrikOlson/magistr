# Quickstart Validation: LSP Resolver Plugin

Runnable scenarios proving the feature end to end. Prerequisites: `bun install`,
Rust toolchain per `rust-toolchain.toml`. All commands from the repo root.

## Scenario 1 — Degradation floor (US2 / SC-001): the pre-feature product survives

```
bun test test/resolver-plugin.test.ts -t "adapter"
```

Expected: with the resolver (a) absent, (b) crashing on every request, (c) hanging
past its timeout, every tool request in the scenario answers ok at tree-sitter
fidelity; no request waits on the resolver; provenance on all reference edges is a
`tier1-*` value.

## Scenario 2 — Plugin resolves through the public SDK (US1)

```
bun test test/resolver-plugin.test.ts -t "live lane"
```

Expected: with the fake language server registered through the detection test seam
and the plugin healthy, a references request returns edges labeled `resolver-lsp` /
`high`. The plugin's manifest declares only public WIT imports; the test asserts the
effective capability set is exactly manifest ∩ grants.

## Scenario 3 — TCK unmodified (SC-002, M5 gate criterion)

```
bun test test/plugin-tck.test.ts
```

Expected: full pass over `plugins/lsp-resolver` with zero TCK modifications in the
diff (reviewable: `git diff --stat` touches no `tck` file).

## Scenario 4 — Provenance honesty (US3 / SC-003)

```
bun test test/resolver-plugin.test.ts -t "provenance merge"
```

Expected: resolver healthy → `resolver-lsp` edges; resolver unhealthy → only
`tier1-*` edges; mixed answers label per-edge; no symbolic edge ever carries
`resolver-lsp`.

## Scenario 5 — Capability is blessed-only (FR-004, escape suite)

```
bun test test/sandbox-escape.test.ts -t "subprocess"
```

Expected: a non-blessed plugin declaring the subprocess import answers
`PLUGIN_CAP_DENIED` with `fuelUsed 0` (refused before instantiation) even when a
grant record exists; the blessed resolver with the grant proceeds.

## Scenario 6 — Contract gates

```
cargo test --test wit_contracts
bun test test/wit-contracts.test.ts
```

Expected: `workbench:subprocess@0.1.0` present and blessed in `wit/baseline.json`;
`workbench:resolver` unchanged at 0.1.0; the TS↔WIT crosswalk green with the port at
v2.

## Full gate sweep (required before completion)

```
bun test
bunx tsgo --noEmit
bunx biome check .
bun run lint:imports
bun run codegen:check
cargo test
```

Expected: all exit 0. These are the same gates every chunk ships behind.
