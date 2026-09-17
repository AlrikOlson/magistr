# Rust Conventions

## Toolchain

- Edition 2024, stable toolchain.
- `cargo check --all-targets` before commit. `cargo clippy -- -D warnings` treats all warnings as errors.
- `cargo fmt` enforced — no style debates.

## Error Handling

- No `unwrap()` in library code. Use `?` for propagation, `expect("context")` only when invariants are truly guaranteed.
- `thiserror` for library error types, `color-eyre` / `anyhow` for application-level errors.
- Return `Result<T, E>` from fallible functions. Never use `panic!` for recoverable errors.
- Wrap errors with context: `.map_err(|e| ...)` or `.context("what failed")`.

## Naming

- `snake_case` for modules, functions, variables, and methods.
- `PascalCase` for types, enums, traits, and type aliases.
- `SCREAMING_SNAKE_CASE` for constants and statics.
- Prefix boolean functions/methods with `is_`, `has_`, `can_`, `should_`.

## Patterns

- Builder pattern for structs with many optional fields.
- Newtype wrappers (`struct UserId(u64)`) for domain types that deserve type safety.
- Accept `impl Into<T>` or `AsRef<T>` for flexible API boundaries.
- Mark return values with `#[must_use]` when ignoring them is almost certainly a bug.
- Prefer iterators and combinators over manual loops. Chain `.map()`, `.filter()`, `.collect()`.
- Use `enum` with variants over boolean flags or stringly-typed dispatch.

## File Structure

- **Cohesive modules, not item counts.** A module holds the public types and behavior that change together (one decision, one contract). Several `pub` items may share a file when it is a facade internal: its parent declares it `mod name;` (never `pub mod`) and re-exports the surface with `pub use name::…`, it opens with a `//!` doc naming what the items decide together and why they change together, and it has its companion `{name}.tests.rs`. A file reached through a public module path exports at most one item. `pub mod`, `pub use` and impl blocks never count.
- **Tests in separate files.** Unit tests go in `{name}.tests.rs`, NOT inline `#[cfg(test)] mod tests {}` blocks. The test file uses `use super::*;` to access the parent module's internals.
- **Fitness over structure.** The checks that carry this policy are behavior and boundary tests: the companion suites, the dependency-direction rule, and the `domain_boundary` / `application_boundary` suites. Enforced by `scripts/lint-structure.sh` on the owned crates; `.magistr/lint-structure-allowlist.txt` is a grandfathered baseline that is pruned, never extended. Imported subsystems keep their own per-file ratchets.

## Testing

- Unit tests in `{name}.tests.rs` files alongside the module they test.
- Integration tests in `tests/` directory.
- Use `assert_eq!`, `assert_ne!`, `assert!(cond, "message")`. Prefer `assert_matches!` for enum variants.
- Test error paths, not just happy paths.
- Use `tempfile` for filesystem tests — never write to fixed paths.

## Anti-Patterns

- No `clone()` to silence the borrow checker — fix the ownership model instead.
- No `Box<dyn Any>` — use concrete types or proper trait objects.
- No `unsafe` without a `// SAFETY:` comment justifying the invariant.
- No wildcard imports (`use foo::*`) except in test modules and preludes.
- No raw `println!` — use structured output, logging, or the TUI renderer.
- No `#[allow(clippy::...)]` without a comment explaining why the lint is wrong here.

## Dependencies

- Minimize dependency count. Prefer std-lib solutions when reasonable.
- Pin major versions in `Cargo.toml`. Use `cargo update` deliberately, not blindly.
- Audit new deps: check maintenance status, transitive dependency count, and unsafe usage.
