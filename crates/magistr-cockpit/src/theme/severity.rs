//! How badly one thing wants a person, as a colour band.

use magistr_theme::SemanticRole;

use super::icons::Icons;

/// Severity styling for cards: a tinted background and a colored icon.
/// Borders are not used to encode severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Red: something is stopped and needs a decision.
    Alert,
    /// Amber: something is finished and waiting to be read.
    Attention,
    /// Green: something landed, and there is nothing to do about it.
    Good,
    /// Grey: a measurement, carried for the record.
    Quiet,
}

impl Severity {
    /// Every band, **worst first** — the order the Brief deals in, so a
    /// sort is `Severity::ALL`'s own order rather than a comparator
    /// each caller writes. `Ord` agrees with it by declaration order.
    pub const ALL: [Self; 4] = [Self::Alert, Self::Attention, Self::Good, Self::Quiet];

    /// The signal role the band's hue comes from — `None` for
    /// [`Severity::Quiet`], which is secondary text and not a signal at
    /// all, and so is the one band a colour-vision remapping leaves
    /// alone.
    #[must_use]
    pub const fn role(self) -> Option<SemanticRole> {
        match self {
            Self::Alert => Some(SemanticRole::Error),
            Self::Attention => Some(SemanticRole::Warning),
            Self::Good => Some(SemanticRole::Success),
            Self::Quiet => None,
        }
    }

    /// The band's mark, drawn from the glyph vocabulary in force —
    /// the colored icon. Never a literal
    /// at a call site: the whole set lives in `theme/icons.rs` and a
    /// surface asks for it through `Pen.icons`.
    #[must_use]
    pub const fn mark(self, icons: &Icons) -> &'static str {
        match self {
            Self::Alert => icons.warning,
            Self::Attention => icons.file,
            Self::Good => icons.done,
            Self::Quiet => icons.blocked,
        }
    }

    /// The band's name, for a probe's message and a test's table.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Alert => "alert",
            Self::Attention => "attention",
            Self::Good => "good",
            Self::Quiet => "quiet",
        }
    }
}

#[cfg(test)]
#[path = "severity.tests.rs"]
mod tests;
