# Native consumer fixtures

`engine.json` records the inputs and observable outputs of the pinned Workbench
`engine.rs` parse/search/rewrite cases and `indexer.rs::rust_symbols_and_imports`.
The WAT files are verbatim fixture strings from the pinned `sandbox.rs` suite.
They test the adopted crate through `use magistr_workspace`, as ordinary Rust
integration binaries, with no addon runtime or test-only linking feature.

Run `cargo test -p magistr-workspace --test native_consumer --test native_sandbox`
for the consumer contracts. The crate's complete test suite also retains the
upstream grammar, indexer, sandbox, WIT compatibility and language-pack tests.
Guest termination, capability denial and malformed input are asserted as data.

After installing and building the reference with its lockfiles, run
`cd subsystems/workspace/reference && bun ../conformance/reference.ts` to execute
the same fixtures through `loadVerifiedCore({ force: "native" })`. Both runners
assert the same observable values, normalizing camelCase addon fields to the
Rust fixture names and matching error categories across the FFI boundary.

These fixtures preserve behavior at the native boundary. They do not claim
conformance for the TypeScript MCP orchestration, loader or service lifecycle,
which remain retained reference source in this adoption chunk.
