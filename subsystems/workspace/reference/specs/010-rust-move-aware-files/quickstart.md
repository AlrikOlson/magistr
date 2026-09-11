# Quickstart Validation: Rust Move-Aware Files

Prerequisites: `bun install`; for the compile-proof scenarios, `cargo` on PATH (`cargo
--version` exits 0). Commands from the repo root.

## Scenario 1 — Rename in place (US1 / SC-001)

```
bun test test/rust-move.test.ts -t "rename"
bun test test/files-tool.test.ts -t "rust"
```

Expected: with `src/a.rs` declaring `pub mod b;`, moving `src/a/b.rs` → `src/a/bee.rs` answers
`CONFIRM_REQUIRED` whose diff rewrites the declaration to `pub mod bee;`, rewrites
`crate::a::b::…` in `src/lib.rs`, `self::b::…` and uniform `b::…` in `src/a.rs`, and
`super::b::…` in a sibling module; the toolchain-gated test then applies it and `cargo check`
exits 0 (skips with a reason when cargo is absent).

## Scenario 2 — Move under another parent (US2 / SC-002)

```
bun test test/rust-move.test.ts -t "move"
```

Expected: moving `src/a/b.rs` → `src/c/b.rs` removes `pub mod b;` from `src/a.rs`, adds it to
`src/c.rs`, rewrites `self::b::helper()` in `src/a.rs` to `crate::c::b::helper()`, rewrites
the moved file's `super::sibling()` to `crate::a::sibling()`, and `cargo check` exits 0. With
no `src/c.rs`/`src/c/mod.rs`, the old declaration is still removed and the note names the
missing parent file and the declaration it needs.

## Scenario 3 — Manifest facts (US3 / SC-003)

```
bun test test/rust-move.test.ts -t "manifest"
```

Expected: a `[[bin]] path = "src/tools/cli.rs"` entry follows the moved file and the manifest
stays well-formed; a manifest without such an entry is not in the diff; `[workspace] members`
never changes; an auto-discovered target's move notes the table that would pin it.

## Scenario 4 — Honest notes and the negatives (US4 / SC-005, SC-006)

```
bun test test/rust-move.test.ts -t "note"
bun test test/files-tool.test.ts
```

Expected: brace-group members are reported with file:line; a non-leaf module move attempts no
Rust rewrite and lists the children; an undiscoverable declaration attempts none and says so;
the six non-Rust fixtures pass unchanged; the Go note no longer mentions Rust.

## Scenario 5 — Gates (SC-007)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check && bun run lint:skill
cargo test --manifest-path crates/workbench-core/Cargo.toml
```

Expected: all exit 0 (server-verified via `ship_check`).
