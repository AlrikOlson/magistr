# Owned workspace plugin fixtures

These four guest components exercise Magistr's current WIT contracts. They are rebuilt from the adjacent locked Rust sources with the `magistr` namespace; the pinned upstream binaries are not used by native tests. The six public WIT definitions live in `../../wit`.

Source provenance: Workbench revision `2a2430bbcd5dcbed00920f92c6f3e6928f387d95`, retained under `subsystems/workspace/reference/plugins` in the Magistr repository. Changes are the package namespace, current subprocess capability and Magistr publisher metadata. The Gleam grammar artifact is unchanged from that import. See the repository's `subsystems/workspace/THIRD_PARTY_NOTICES.md` and `PROVENANCE.md` for attribution and limits: upstream guest manifests omit their own license declarations. This copy does not invent a license grant for those sources. Dependency and grammar notices are retained in `LICENSES` alongside this file.

To rebuild from the Magistr checkout, install the `wasm32-wasip1` Rust target, then run for each of `lang-pack-gleam`, `lsp-resolver`, `ranking-visibility`, and `runner-go`:

```sh
cargo build --locked --release --target wasm32-wasip1 \
  --manifest-path crates/magistr-workspace/fixtures/plugins/PLUGIN/guest/Cargo.toml \
  --target-dir target/plugin-fixtures
cargo run --locked --release --manifest-path subsystems/workspace/reference/Cargo.toml \
  -p componentize -- target/plugin-fixtures/wasm32-wasip1/release/PLUGIN_guest.wasm \
  crates/magistr-workspace/fixtures/plugins/PLUGIN/guest.wasm
```

Replace hyphens with underscores only in the intermediate `PLUGIN_guest.wasm` name. `artifacts.json` records the committed component and grammar SHA-256 values. Normal builds and extracted-package tests consume these checked-in artifacts and require neither the reference checkout nor a guest toolchain.
