//! What an owner says about itself to a probe.

use serde::{Deserialize, Serialize};

use super::attach_kind::AttachKind;
use super::build_identity::BuildIdentity;
use super::owner_connection_record::OwnerConnectionRecord;
use super::owner_generation::OwnerGeneration;
use super::owner_identity::OwnerIdentity;

/// The line an owner writes after welcoming a `probe` attachment, before it
/// closes the connection: the generation it was admitted under, its
/// process, its build, the identity it serves and every session it has
/// welcomed and not yet seen end. The probe that asked is not among them.
///
/// A report is an observation of one moment, made by the owner itself.
/// It certifies that the owner was alive when it answered; it decides
/// nothing, admits nothing and is never retained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerReport {
    /// The generation the owner was admitted under.
    pub generation: OwnerGeneration,
    /// The owner process.
    pub owner_pid: u32,
    /// The owner's build; a probe of another build is refused instead.
    pub build: BuildIdentity,
    /// What the owner serves.
    pub identity: OwnerIdentity,
    /// Every welcomed session still running, in welcome order.
    pub connections: Vec<OwnerConnectionRecord>,
}

impl OwnerReport {
    /// How many live sessions attached for `kind`.
    #[must_use]
    pub fn attached(&self, kind: AttachKind) -> usize {
        self.connections
            .iter()
            .filter(|record| record.attach == kind)
            .count()
    }
}
