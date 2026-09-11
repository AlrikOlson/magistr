//! `workbench-core` — the single native crate (SDD §3).
//!
//! Wraps tree-sitter (twelve bundled grammar crates), the `grep` crate
//! (ripgrep's engine as a library) and ast-grep (structural match/rewrite)
//! behind one FFI boundary. Exposed to the engine child via napi-rs; the same
//! crate builds to `wasm32-wasip1` as the fallback when the native addon
//! fails to load. The `bindings` module is the only napi-aware code; `engine`
//! and `langs` are plain Rust and unit-tested directly.

pub mod engine;
pub mod indexer;
#[cfg(feature = "lang-pack")]
pub mod lang_registry;
pub mod langs;
#[cfg(feature = "sandbox")]
pub mod sandbox;

/// Version of the crate, surfaced so the engine child can report what it loaded.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// `no-bindings` (Cargo.toml): the integration-test link aid — the same cut
// `cfg(test)` already makes for unit tests, reachable from a test binary.
#[cfg(all(not(test), not(feature = "no-bindings")))]
mod bindings;

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_is_reported() {
        assert!(!VERSION.is_empty());
    }
}
