use std::{future::Future, pin::Pin};

use crate::contracts::WorkspaceError;

/// One boxed lane call. A lane error (worker down, sandbox lane absent) is a
/// typed, retryable condition distinct from any guest outcome.
pub type LaneFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, WorkspaceError>> + Send + 'a>>;
