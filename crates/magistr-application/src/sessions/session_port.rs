use crate::{CommandContext, PortFuture};

use super::{SessionCommand, SessionResponse};

/// Effect boundary for a future shared session supervisor.
///
/// Implementations admit the command against the current revision and provider
/// capabilities, retain scoped identities and explicit provider bindings, and
/// distinguish acknowledged submission from terminal outcomes. Dropping a future
/// is not evidence that its provider operation was cancelled. An interrupted
/// operation whose effects cannot be established returns `UnknownOutcome`.
pub trait SessionPort: Send {
    fn execute<'a>(
        &'a mut self,
        context: &'a CommandContext,
        command: &'a SessionCommand,
    ) -> PortFuture<'a, SessionResponse>;
}
