//! What one bounded probe of an endpoint established.

use std::time::Duration;

use super::attach_refusal::AttachRefusal;
use super::owner_report::OwnerReport;

/// The outcome of one probe handshake. Only an answer from the owner's own
/// process — a report or a typed refusal — certifies that an owner is
/// alive; a socket file, a durable generation counter or an accepted
/// connection that never greets certifies nothing, and each is named as
/// what it is rather than read as a sign of life.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerLiveness {
    /// The owner welcomed the probe and reported itself.
    Live(Box<OwnerReport>),
    /// A live owner answered the handshake with a refusal: it exists, but
    /// this binary cannot attach to it (another build, another identity).
    Refused(AttachRefusal),
    /// The connection was accepted but no greeting arrived within the
    /// deadline: something listens, nothing is known about it.
    Silent {
        /// How long the probe waited.
        deadline: Duration,
    },
    /// No endpoint at the path: no owner was ever there, or its socket has
    /// been unlinked.
    Absent,
    /// An endpoint exists at the path but nothing answered on it — a stale
    /// socket file, a hang-up before the greeting, or a peer that is not an
    /// owner at all.
    Unanswered {
        /// The connection or protocol failure, in the transport's words.
        reason: String,
    },
}

impl OwnerLiveness {
    /// Whether an owner process answered the handshake. A refusal counts:
    /// only a live owner refuses.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        matches!(self, Self::Live(_) | Self::Refused(_))
    }

    /// The report, when the owner gave one.
    #[must_use]
    pub fn report(&self) -> Option<&OwnerReport> {
        match self {
            Self::Live(report) => Some(report),
            _ => None,
        }
    }
}
