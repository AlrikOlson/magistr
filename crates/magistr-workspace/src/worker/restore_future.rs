use std::{future::Future, pin::Pin};

use super::WorkerError;

/// One boxed restore step over a not-yet-admitted worker incarnation.
pub type RestoreFuture<'a> = Pin<Box<dyn Future<Output = Result<(), WorkerError>> + Send + 'a>>;
