use magistr_cockpit::theme::ThemeMode;

/// One of the design's two media: **night** — the sky, whose medium is light — and
/// **day** — the chart, whose medium is ink. Day is not inverted night, which is why a
/// view is captured twice and the parity gate diffs each still against its own medium's
/// capture.
///
/// The medium names the capture file (`night-05.png` / `day-05.png`) and the palette
/// the still is rasterized in (Mocha / Latte) from one value, so the gate cannot diff a
/// Latte still against a night capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Medium {
    /// The sky: Catppuccin Mocha, `night-NN.png`.
    Night,
    /// The chart: Catppuccin Latte, `day-NN.png`.
    Day,
}

impl Medium {
    /// Both media, in the order the gate reports them.
    pub const ALL: [Self; 2] = [Self::Night, Self::Day];

    /// The capture file's prefix.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Night => "night",
            Self::Day => "day",
        }
    }

    /// The palette the still is rasterized in.
    #[must_use]
    pub const fn mode(self) -> ThemeMode {
        match self {
            Self::Night => ThemeMode::Dark,
            Self::Day => ThemeMode::Light,
        }
    }

    /// The medium a mask rect names in its `media` field, or `None` for
    /// a word that is neither.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.prefix() == word)
    }
}

impl std::fmt::Display for Medium {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.prefix())
    }
}

#[cfg(test)]
#[path = "medium.tests.rs"]
mod tests;
