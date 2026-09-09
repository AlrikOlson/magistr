use crate::{CommandContext, PortFuture};

use super::{PermissionCommand, PermissionResponse};

/// Effect boundary for pending-request correlation and the shared project policy.
///
/// Implementations verify that the selected option was offered on the live
/// decision, apply policy Deny before any approval, and admit the expected
/// revision before answering. Noninteractive Ask must refuse unless an explicit
/// compatibility policy was selected. This contract introduces no automatic
/// approval default and no provider response channel.
pub trait PermissionPort: Send {
    fn resolve<'a>(
        &'a mut self,
        context: &'a CommandContext,
        command: &'a PermissionCommand,
    ) -> PortFuture<'a, PermissionResponse>;
}
