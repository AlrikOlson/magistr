use alloc::sync::Arc;
use core::fmt;

use super::{SourceRefusal, SourceWriter};

/// The one route a Magistr-managed source write can take: the admitted
/// workspace's pipeline, or a typed refusal. There is no third state — a
/// host either owns the checkout for its run and writes through it, or it
/// holds a reason and refuses. A port composed with a refusal advertises
/// no write capability, so a cooperating agent runs its own workspace tools
/// instead; those effects are external to this contract and are observed,
/// never routed.
#[derive(Clone)]
pub enum SourceRoute {
    /// This host admitted the workspace; authorized writes go through it.
    Admitted(Arc<dyn SourceWriter>),
    /// This host holds no writer; every write is refused with the reason.
    Refused(SourceRefusal),
}

impl SourceRoute {
    /// Whether a write asked of this route can land.
    #[must_use]
    pub const fn is_admitted(&self) -> bool {
        matches!(self, Self::Admitted(_))
    }

    /// The refusal, when the route is one.
    #[must_use]
    pub const fn refusal(&self) -> Option<&SourceRefusal> {
        match self {
            Self::Admitted(_) => None,
            Self::Refused(refusal) => Some(refusal),
        }
    }
}

impl fmt::Debug for SourceRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admitted(_) => f.write_str("SourceRoute::Admitted(..)"),
            Self::Refused(refusal) => f
                .debug_tuple("SourceRoute::Refused")
                .field(refusal)
                .finish(),
        }
    }
}
