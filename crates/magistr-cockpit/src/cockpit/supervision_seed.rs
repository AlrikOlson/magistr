//! The supervision rail's construction-time seed, as data.

use super::{Budget, CharterOffer, CharterRule};

/// What a host derived for the supervision rail before the session starts:
/// the charter rows, the delegation offers, the budget and the mode word.
/// Data rather than a seeded [`Model`], so a host can gather it on any
/// thread and the surface applies it on its own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SupervisionSeed {
    /// The charter rows, in order.
    pub charter: Vec<CharterRule>,
    /// The delegation offers the reader's record supports.
    pub offers: Vec<CharterOffer>,
    /// The budget the session runs under, when one binds.
    pub budget: Option<Budget>,
    /// The mode word the agent block names after the model.
    pub mode: String,
}
