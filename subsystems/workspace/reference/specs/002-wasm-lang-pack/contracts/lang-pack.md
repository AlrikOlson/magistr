# Contract: workbench:lang-pack@0.1.0 (consumed unchanged)

**Feature**: 002-wasm-lang-pack | **Source of truth**: `wit/lang-pack.wit`,
blessed in `wit/baseline.json` (enforced by
`crates/workbench-core/tests/wit_contracts.rs`).

This feature consumes the interface as blessed. **No WIT vocabulary changes
are in scope**; the contract gates must pass with an untouched baseline.

## Guest obligations (the pack)

- Export the `lang-pack` interface of world `lang-pack-provider`:
  `info`, `grammar-wasm`, `tag-queries`, `classify-file`.
- Declare zero capabilities; import nothing beyond the interface types.
  The host instantiates with an empty linker surface (FR-010).
- `info().name` is the lowercase language id; `extensions` claim no bundled
  extension (a claim that does is ignored with a note, never an error).
- `info().name` MUST equal the grammar's own language name: tree-sitter's
  WasmStore resolves the exported `tree_sitter_<name>` symbol by the load
  name, so a mismatched id is rejected at registration (observable, typed).
- `grammar-wasm()` returns a tree-sitter grammar compiled to WASM (built by
  the tree-sitter CLI); `tag-queries()` returns `.scm` query text whose
  captures map to the indexer's symbol/import kinds.
- `classify-file` answers only where the pack knows better; `none` defers.
- Pass the TCK unmodified (m5-gate criterion).

## Host obligations

- Instantiate the carrier only in the Wasmtime sandbox lane (fuel-limited,
  default-deny), read the four exports, cache per component hash.
- Execute grammar bytes only in tree-sitter's WasmStore for
  sideload/verified tiers; native lane only at blessed tier where the child
  carries the native grammar (R5). Lane is observable in the inventory.
- Route pack-language edits through the same parse-verify choke point as
  bundled languages (Constitution III).
- Degrade every pack failure to lexical-only service for that language;
  never let a pack failure surface on unrelated queries (FR-009).
- Surface every shadowed claim, conflict, or refusal as an observable note
  from the existing taxonomy (FR-012).

## Verification

- `cargo test` WIT contract gate: six-package census, baseline byte-equal.
- TCK chapter over the pack component, unmodified kit.
- Tier-flip suite: same fixtures, both lanes, identical `Extraction`.
- Sandbox-escape suite: the pack (any tier) is refused any capability it
  did not declare — which is all of them.
