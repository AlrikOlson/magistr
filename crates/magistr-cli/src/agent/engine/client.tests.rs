use super::*;
use crate::agent::engine::local::LocalEngineClient;

/// Verify that `LocalEngineClient` implements `EngineClient`.
///
/// This is a compile-time check — if `LocalEngineClient` doesn't satisfy
/// the trait bounds, this test won't compile.
fn _assert_engine_client_impl<T: EngineClient>() {}

#[test]
fn local_client_implements_trait() {
    _assert_engine_client_impl::<LocalEngineClient>();
}
