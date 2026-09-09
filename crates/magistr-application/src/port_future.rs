use alloc::boxed::Box;
use core::{future::Future, pin::Pin};

use crate::ApplicationError;

/// Runtime-independent, dyn-compatible asynchronous port response. The host polls
/// it. Dropping it does not prove that an admitted effect was cancelled.
pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ApplicationError>> + Send + 'a>>;
