#!/usr/bin/env bash
# Development-only algorithm comparisons. Native contracts own the public API.
# Never launch a server against the real checkout.
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT/subsystems/workspace/reference"
unset WORKBENCH_MULTI_CLIENT
bun install --frozen-lockfile
# The second "--" belongs to napi; bun run would consume it.
bunx napi build --manifest-path crates/workbench-core/Cargo.toml \
    --platform --release --esm --output-dir crates/workbench-core/artifacts -- --locked
bun run scripts/smoke-core.ts native
bun ../conformance/reference.ts
bun ../conformance/config-reference.ts
bun ../conformance/jail-reference.ts
bun ../conformance/responses-reference.ts
bun ../conformance/content-reference.ts
bun ../conformance/journal-reference.ts
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --features workbench-core/no-bindings -- -D warnings
# no-bindings is used only for reference Rust test linking, never native proof.
cargo test --locked --workspace --features workbench-core/no-bindings
# Standalone guest packages retain separate locks and are checked as WASI code.
for guest in plugins/*/guest/Cargo.toml; do
    cargo fmt --manifest-path "$guest" -- --check
    cargo clippy --locked --manifest-path "$guest" --target wasm32-wasip1 -- -D warnings
done
python3 "$PROJECT_ROOT/scripts/workspace-runner-build-check.py"
bun run codegen:check
bun run typecheck
bun run lint
bun run lint:imports
bun run lint:skill
bun test
# Compare complete native query envelopes after the retained native addon exists.
cargo test --manifest-path "$PROJECT_ROOT/Cargo.toml" -p magistr-cli --test workspace_queries_conformance -- --ignored
# Cross-runtime journal append/read uses only disposable SQLite databases.
cargo test --manifest-path "$PROJECT_ROOT/Cargo.toml" -p magistr-workspace --test journal_conformance -- --ignored

