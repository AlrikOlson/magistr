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

## Policy compatibility fixtures

The `config.json`, `jail.json`, `responses.json` and `content.json` vectors
extend this corpus for `arch-14c1-workspace-policy`. Each has an ordinary Rust
integration test and a matching `*-reference.ts` runner that imports the
pinned implementation. The required `workspace-reference` gate runs all four
reference runners; root Rust and `workspace-native` gates run their native
counterparts. No fixture runner starts a workspace server.

- Configuration fixtures compare layered effective settings, command admission,
  provenance and policy refusal observations. Companion tests cover command
  detection, malformed layers and native empty-command refusal.
- Jail fixtures compare read/write admission and protected path behavior.
  Native companions cover symlink-before-parent traversal, dangling escapes,
  retargeted configuration aliases, additive subtree protection (`/**` supported;
  unsupported glob forms refused),
  key creation/rotation, malformed material, private creation modes and
  concurrent initialization. Known unsafe reference cases are explicit native
  corrections, not parity expectations.
- Response fixtures compare version negotiation, all 24 error/recovery pairs
  and required payloads, ordered rows, truncation manifests, optional notes,
  redaction and port signature versions. The reference runner also validates
  envelope schemas. Token counters describe accounting; query selection is
  implemented by later native query units.
- Content fixtures compare NUL/invalid-UTF8 classification, raw byte retention
  including BOMs and SHA-256 values. Text/binary variants compare by bytes.

Run the native policy consumers with:

```sh
cargo test --locked -p magistr-workspace --test config_conformance --test jail_conformance --test responses_conformance --test content_conformance
```

These tests certify policy/value boundaries only. They do not establish atomic
filesystem confinement, confirmation-grant verification, complete tool parity,
workspace ownership, source certification or production protocol integration.
