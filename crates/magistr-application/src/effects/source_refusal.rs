use alloc::string::String;
use core::fmt;

use super::EffectError;

/// Why a host holds no route for Magistr-managed source writes. A session
/// composed with one of these refuses every `fs/write_text_file` it is
/// asked for: nothing lands, nothing is journaled, no receipt is invented.
/// The host never falls back to a direct write and never attaches to
/// another owner to write through it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRefusal {
    /// A live owner elsewhere holds the workspace. This host admitted
    /// nothing: no second owner and no attachment for writing.
    OwnerElsewhere,
    /// The workspace could not be admitted for another reason, in the
    /// owner's own words: a marker that needs migration, durable data from
    /// a newer binary, a configuration the policy refused, a root or store
    /// that failed to open, or a service that refused to mint a session.
    NotAdmitted(String),
    /// The session was composed without source-write authority: the
    /// constrained judge, or any session resolved without a route.
    NoAuthority,
}

impl SourceRefusal {
    /// The stable wire word for this refusal.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::OwnerElsewhere => "owner_elsewhere",
            Self::NotAdmitted(_) => "not_admitted",
            Self::NoAuthority => "no_authority",
        }
    }

    /// The typed answer to a write asked of a host without a route. A
    /// missing owner is an unavailable capability; a session without
    /// authority is denied.
    #[must_use]
    pub fn effect_error(&self) -> EffectError {
        match self {
            Self::OwnerElsewhere | Self::NotAdmitted(_) => {
                EffectError::Unavailable(alloc::format!("{self}"))
            }
            Self::NoAuthority => EffectError::Denied(alloc::format!("{self}")),
        }
    }
}

impl fmt::Display for SourceRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerElsewhere => f.write_str(
                "source write refused: another owner holds the workspace and this host attaches to nothing for writing",
            ),
            Self::NotAdmitted(reason) => {
                write!(f, "source write refused: the workspace was not admitted ({reason})")
            }
            Self::NoAuthority => {
                f.write_str("source write refused: this session has no source-write authority")
            }
        }
    }
}

#[cfg(test)]
#[path = "source_refusal.tests.rs"]
mod tests;
