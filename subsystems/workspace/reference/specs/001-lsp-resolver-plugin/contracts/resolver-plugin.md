# Contracts: LSP Resolver Plugin

The WIT files under `wit/` are the source of truth for plugin-facing contracts, and
`schemas/` for envelope shape. This file records what this feature ADDS to each
contract surface; nothing existing changes shape.

## 1. `wit/resolver.wit` — unchanged export surface

`workbench:resolver@0.1.0`, world `resolver-provider`, exports
`resolve-references(symbol) -> list<source-location>`, `rename`, `diagnostics`.
The bundled plugin implements this world as-is. Contract gate:
`crates/workbench-core/tests/wit_contracts.rs` against `wit/baseline.json` (no bless
needed for this package).

## 2. NEW: `wit/subprocess.wit` — `workbench:subprocess@0.1.0`

The narrow host capability (research D2/D3). Shape to be authored in the build unit,
bound by these contract rules:

- Server addressing is by host-curated spec id / language id — **no argv, no binary
  path, no env crosses the boundary from guest to host**.
- Transport is stdio framing only: `write-frame(bytes)` / `read-frame() -> bytes`
  style, host-buffered, bounded.
- Every function is fallible with typed error variants (spawn-refused, dead,
  timeout); no errno leakage.
- Import name of this interface in a plugin world IS the grant name; host requires
  inventory tier `blessed` in addition to the grant (data-model rule).
- Change interfaces, current hosts/guests and `wit/baseline.json` together.
  No additive-only or major-version preservation constraint applies before
  Magistr's public release.

## 3. `src/ports/resolver.ts` — v2 additive

- `PORT_VERSIONS.resolver: 1 → 2` with the required "what changed" comment
  (versions.ts house style).
- Resolve results carry per-edge `provenance` + `confidence` (data-model vocabulary).
- Existing v1 call shape untouched; `test/wit-contracts.test.ts` crosswalk updated
  additively (source-location ↔ SourceLocation mapping is already pinned there).

## 4. Envelope / references lane — additive provenance value

- New provenance string `resolver-lsp` joins the existing per-hit vocabulary in
  references mode; no schema major (values are open strings today; the exact new
  value is pinned by test, SC-003).
- Resolver unavailability is fidelity degradation, never an error envelope, for any
  request the symbolic lane can answer (FR-007).

## 5. Failure taxonomy

- Reuses `PLUGIN_CAP_DENIED` (un-granted/un-blessed subprocess import, refused
  pre-instantiation) and `PLUGIN_CRASHED` (guest trap/fuel/memory), both existing.
- Any new code (e.g. a subprocess-specific refusal) lands in
  `schemas/error-taxonomy.json` with exactly one recovery class
  (one-code-one-recovery is a schema test).

## 6. TCK

`test/plugin-tck.test.ts` + `src/tools/tck.ts` run over the bundled resolver plugin
**unmodified** — the M5 gate criterion and spec SC-002. Any TCK change required to
pass is by definition an SDK defect to fix, not a TCK patch.
