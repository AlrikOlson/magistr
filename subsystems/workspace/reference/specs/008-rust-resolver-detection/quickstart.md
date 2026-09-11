# Quickstart Validation: Rust in the Resolver Detection Table

Prerequisites: `bun install`; for the gated scenario, `rustup which rust-analyzer` must
succeed (the component installed for the workspace toolchain). Commands from the repo root.

## Scenario 1 — Detection unit matrix (US1/US2/US3 / SC-001)

```
bun test test/resolver-plugin.test.ts -t "rust detection"
```

Expected, over an injected resolver and temp workspaces:
- `Cargo.toml` + resolver returns a path ⇒ exactly one `{ languageId: "rust", bin, args }` entry.
- no `Cargo.toml` ⇒ no Rust entry (even if the resolver would return a path).
- resolver returns `null` (proxy without component) ⇒ no Rust entry, no spawn.
- a TypeScript project that also has a `Cargo.toml` ⇒ both entries.

## Scenario 2 — Real fixture references + the number (US1 / SC-002 / SC-004)

```
bun test test/resolver-plugin.test.ts -t "rust fixture"
```

Expected with rustup + rust-analyzer: a temp fixture crate (trait impl method, generic fn,
macro-generated item) is detected by the REAL `detectLanguageServers`; a warm references
request returns `resolver-lsp`/`high` edges at the real sites; cold vs warm latency is printed
and recorded. Without rust-analyzer: skipped with a message naming rustup + rust-analyzer.

## Scenario 3 — Absent rust-analyzer is byte-identical tree-sitter (US2 / SC-003)

```
bun test test/resolver-plugin.test.ts -t "no regression"
```

Expected: references over the same fixture with no resolver configured equal the answer with a
Rust entry whose resolver is absent/unhealthy — the pre-feature product, preserved.

## Full gate sweep (required before completion)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check && bun run lint:skill
cargo test --manifest-path crates/workbench-core/Cargo.toml
```

Expected: all exit 0, recorded server-side via `ship_check`.
