# Data Model: First-Party WASM Language Pack

**Feature**: 002-wasm-lang-pack | **Date**: 2026-08-22

## Entities

### PackLanguage (engine child, `lang_registry.rs`)

One registered pack-provided language.

| Field | Type | Rules |
|---|---|---|
| `id` | string | lowercase language name from `language-info.name`; MUST NOT collide with a bundled id (refused with note, FR-005/FR-012) |
| `extensions` | string[] | claimed extensions (no dot); bundled extensions are stripped from the claim with a note; inter-pack conflicts resolve by install order with a note |
| `lane` | `native \| wasm` | chosen once at registration (R5); observable |
| `grammar` | native `LanguageFn` or WasmStore-loaded `Language` | per lane |
| `queries` | compiled tag queries | compiled once at registration; compile failure ⇒ `queries: none` degradation (extraction off, parse-verify still on) |
| `source` | plugin label + component sha256 | ties health/breaker records to the registry entry |

State transitions: `registered → unhealthy` (grammar load or invocation
failure trips the existing breaker; entry removed from resolution until the
component hash changes) ; `registered → removed` (uninstall; next re-index
drops its symbols, FR-009 / edge "uninstalled after indexed").

### DynamicLanguageRegistry (engine child)

Rebuilt at engine start from installed packs (cache-class data,
Constitution V — never migrated, always re-derivable).

- Resolution order: bundled table first, registry second (R7).
- Lookup keys: language id; extension.
- Invariant: no entry may shadow a bundled id or extension — enforced at
  registration, not at lookup, so the hot path gains no checks.

### PackRecord (host, `plugins.ts` — existing shape, one addition)

The existing installed-plugin record (manifest, tier, grants, health)
gains a surfaced `lane` (from the engine's registration answer) in the map
inventory. No stored schema change: lane is derived, not persisted.

### Carrier exports (the WIT, consumed unchanged)

`info() → language-info { name, extensions }`;
`grammar-wasm() → list<u8>`; `tag-queries() → string`;
`classify-file(path) → option<file-class>`.
Read once per load in the sandbox lane, zero capabilities, results cached
against the component hash.

## Validation rules (from FRs)

- Registration refuses: bundled-id collision, empty extensions, grammar
  bytes that fail WasmStore load, oversized grammar (cap documented with the
  lane) — each with an observable note (FR-012).
- Extraction equality: for any source text, native-lane and wasm-lane
  extraction MUST be identical (FR-008) — pinned by fixture test, not
  assumed.
- Degradation: any pack failure mode leaves lexical service intact
  (FR-009); no pack state is consulted for bundled-language files (FR-005).
