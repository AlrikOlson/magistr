//! Keyboard focus within the terminal skeleton.

/// The pane receiving navigation keys. Prompt editing and modal decisions
/// retain their own priority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pane {
    Rail,
    #[default]
    Stage,
    Inspector,
}

impl Pane {
    pub const fn cycle(self, backwards: bool) -> Self {
        match (self, backwards) {
            (Self::Rail, false) | (Self::Inspector, true) => Self::Stage,
            (Self::Stage, false) | (Self::Rail, true) => Self::Inspector,
            (Self::Inspector, false) | (Self::Stage, true) => Self::Rail,
        }
    }
}
