//! Row height and padding, as a segmented select.

/// How much air the rows carry — an enumerable text choice, so its
/// control is segmented pills.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Density {
    /// The default density.
    #[default]
    Comfortable,
    /// Tighter rows.
    Compact,
}

impl Density {
    /// The two segments, in order.
    pub const ALL: [Self; 2] = [Self::Comfortable, Self::Compact];

    /// The segment's word.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Comfortable => "Comfortable",
            Self::Compact => "Compact",
        }
    }

    /// The density at `index` in [`Density::ALL`], when there is one.
    #[must_use]
    pub fn at(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }
}
