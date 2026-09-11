# gleam.wasm — the committed grammar artifact

`gleam.wasm` is the tree-sitter gleam grammar compiled to WASM, served by
the carrier guest's `grammar-wasm()` export and executed by the engine
child's WasmStore lane (specs/002-wasm-lang-pack/research.md R2, R6).

Built out of band — the repository's gates deliberately require no
C-to-wasm toolchain (same precedent as the committed guest components):

```sh
git clone --depth 1 https://github.com/gleam-lang/tree-sitter-gleam
cd tree-sitter-gleam
bunx --package tree-sitter-cli tree-sitter build --wasm
# → tree-sitter-gleam.wasm; commit as plugins/lang-pack-gleam/grammar/gleam.wasm
```

The tree-sitter CLI (0.26.x) downloads its own wasi-sdk on first use; no
emscripten. The grammar's external scanner is C (`src/scanner.c`) — a C++
scanner would not build to wasm (research R3).

Provenance of this artifact: gleam-lang/tree-sitter-gleam @ the crates.io
`tree-sitter-gleam 1.0.0` generation, built with tree-sitter-cli 0.26.12.
The native promotion lane links the same grammar generation via the
`tree-sitter-gleam` crate, so the tier-flip equality suite compares like
with like (research R5).
