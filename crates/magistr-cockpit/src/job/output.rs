//! Background results produced by the retained application services.
use crate::cockpit::GalaxyData;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobOutput {
    Text(String),
    Galaxy(Box<GalaxyData>),
    Failed(String),
}
impl JobOutput {
    #[must_use]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}
