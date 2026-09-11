use std::{future::Future, pin::Pin};

/// One boxed executor call; executors are object-safe so tests script them.
pub type RunFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
