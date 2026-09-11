# Implementation Plan: LSP Resolver Plugin on the Public SDK

**Branch**: `001-lsp-resolver-plugin` | **Date**: 2026-08-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/001-lsp-resolver-plugin/spec.md` (chunk:m5-lsp-resolver-plugin)

## Summary

Rebuild the LSP resolver as the first first-party consumer of the public plugin SDK:
a bundled, blessed plugin exporting the existing `workbench:resolver@0.1.0` world,
speaking stdio to project-detected language servers through a new narrow subprocess
host capability, lazily booted after Tier-1 indexing, circuit-breakered onto the
existing disk-backed plugin-health mechanism, with per-edge confidence provenance so
envelopes never overclaim. The missing infrastructure this feature forces into
existence — exactly as the SDD planned — is the component host-call bridge
(`src/tools/plugins.ts` header: "The bridge lands with the first first-party
consumer (M5)").

## Technical Context

**Language/Version**: TypeScript (Bun runtime, tsgo strict) for the host; Rust
(workbench-core crate, Wasmtime) for the component lane; the plugin guest compiled to
a WASM component targeting `workbench:resolver@0.1.0`.

**Primary Dependencies**: Wasmtime component host in `crates/workbench-core/src/sandbox.rs`;
plugin manifest/grant/health host in `src/tools/plugins.ts` (chunk:m4-capability-manifests);
`wit/resolver.wit` (`resolver-provider` world); `ResolverPort`
(`src/ports/resolver.ts`, PORT_VERSIONS.resolver = 1); search references lane
(`src/tools/search.ts`, provenance vocabulary `tier1-*`); TCK (`src/tools/tck.ts`,
`test/plugin-tck.test.ts`).

**Storage**: `.workbench/plugins.json` (inventory + grants + tier),
`.workbench/plugin-health.json` (circuit breaker, disk-backed, stateless server).
No new stores.

**Testing**: `bun test` (host + plugin behavior, fake language server), `cargo test`
(component lane + WIT contract gate), TCK unmodified over the resolver plugin,
sandbox-escape suite extended with the subprocess-capability deny case.

**Target Platform**: same as the product — Bun server on the developer machine;
guest is platform-neutral WASM.

**Project Type**: single project (existing layout: `src/` host, `crates/` native,
`wit/` contracts, `examples/plugins/` + bundled plugin assets, `test/`).

**Performance Goals**: references p95 < 50ms at 1M LOC with resolver healthy (existing
release gate, SC-006); zero added latency on any answer from lazy boot (SC-004);
bundled-path behavior unchanged when resolver absent (SC-001).

**Constraints**: no general shell execution at any trust tier (Constitution IV) — the
subprocess capability must not let a guest name an arbitrary command; server process
never opens outbound network connections (Constitution V); no in-memory session state
(statelessness suite); every failure taxonomy-coded (Constitution II).

**Scale/Scope**: one bundled plugin, one new host capability, one WIT package
addition, resolver port v1→v2 additive bump, provenance plumb-through in the
references lane. Language-server detection intentionally minimal (one real detection
heuristic + a test-controlled fake server); breadth is out of scope (spec Assumptions).

## Constitution Check

*GATE: evaluated before Phase 0; re-checked after Phase 1 design.*

| Principle | Verdict | Notes |
|---|---|---|
| I. Match grep's cost structure | PASS | The feature is the degradation guarantee: resolver absent/unhealthy → identical pre-feature product, no waits, no outage (US2, SC-001). Lazy boot forbids added cold-start cost. |
| II. Rank, don't dump | PASS | Provenance rides the existing envelope/confidence vocabulary; all new failures are taxonomy-coded (`PLUGIN_CAP_DENIED`, `PLUGIN_CRASHED` already exist; any new code lands in the published taxonomy with one recovery class). |
| III. Verified writes | PASS | Resolver proposes text only (`rename` returns edits); the HOST applies through the §7.5 write-engine choke point — stated in `wit/resolver.wit` itself. No new write path. |
| IV. Narrow, typed escape hatches | PASS (design-constrained) | The subprocess capability is NOT general execution: guests request a host-curated, detection-resolved server spec by language id — never a command line. Host resolves binary+args; grant is reserved to tier `blessed`; denied pre-instantiation otherwise. See research.md D2. |
| V. Versioned contracts, sandboxed extensions | PASS | New WIT package + resolver port v2 are additive with independent stamps behind the `wit_contracts.rs` baseline gate; plugin runs in the existing Wasmtime sandbox; first-party-on-public-SDK is the feature's whole point. |
| VI. No number, no merge | PASS | No ranking-weight change. Perf gates measured (SC-004/SC-006); TCK + sandbox-escape are release-blocking and both extended here. |

No violations → Complexity Tracking left empty.

## Project Structure

### Documentation (this feature)

```text
specs/001-lsp-resolver-plugin/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── resolver-plugin.md   # Contract deltas (points at wit/ as source of truth)
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
wit/
├── resolver.wit                 # existing — the plugin's export surface (unchanged)
└── subprocess.wit               # NEW — the narrow host capability package

crates/workbench-core/src/
└── sandbox.rs                   # component lane: host-call bridge + subprocess cap

src/ports/
├── resolver.ts                  # v1→v2 additive: provenance on results
└── versions.ts                  # PORT_VERSIONS.resolver bump

src/protocol/
└── resolver-plugin.ts           # NEW — ResolverPort lane backed by the plugin:
                                 # lazy boot, circuit breaker, jail admission,
                                 # detection table. Protocol layer, NOT adapters:
                                 # the import-graph lint forbids adapters → tools/jail,
                                 # and this lane needs both (amended during build)

src/tools/
├── plugins.ts                   # host-call bridge deps + subprocess grant plumbing
└── search.ts                    # references lane: resolver-upgraded provenance

plugins/lsp-resolver/            # NEW — the bundled first-party plugin
├── plugin.toml                  # declares workbench:resolver export + subprocess import
└── (component build artifacts + guest source)

test/
├── resolver-plugin.test.ts      # NEW — US1/US2/US3 acceptance + edge cases
├── plugin-tck.test.ts           # runs over the resolver plugin UNMODIFIED
└── sandbox-escape.test.ts       # + subprocess-cap deny chapter
```

**Structure Decision**: single-project layout, extending the existing seams only:
one new WIT package, one new adapter directory, one new bundled-plugin directory.
The dependency rule (protocol → tools → ports ← adapters) is untouched and stays
enforced by `lint:imports`.

## Complexity Tracking

No constitution violations to justify.
