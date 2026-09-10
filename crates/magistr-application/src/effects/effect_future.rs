use super::EffectError;
use alloc::boxed::Box;
use core::{future::Future, pin::Pin};

/// Runtime-independent asynchronous effect result.
pub type EffectFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, EffectError>> + Send + 'a>>;
