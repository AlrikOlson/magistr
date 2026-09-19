//! The supervision rail's construction-time seed, as data.

use super::{Budget, CharterOffer, CharterRule, Model};

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

impl SupervisionSeed {
    /// Give `model`'s rail this seed.
    #[must_use]
    pub fn seed(self, mut model: Model) -> Model {
        let rail = model.rail_mut();
        rail.set_charter(self.charter);
        rail.set_offers(self.offers);
        rail.set_budget(self.budget);
        rail.set_mode(self.mode);
        model
    }
}

#[cfg(test)]
#[path = "supervision_seed.tests.rs"]
mod tests;
