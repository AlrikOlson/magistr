//! Whether the cockpit moves.

/// How much decorative motion the screen carries: the full layer —
/// eased fills, the streaming shimmer, spinners, tab and dock
/// transitions — or none of it, every surface frozen at its final
/// frame.
///
/// There is no cross-application standard for this the way `NO_COLOR`
/// is for colour, so [`MotionMode::detect`] honours the cockpit's own
/// `MAGISTR_MOTION` first and the two conventions the `--reduce-motion`
/// flag already sets (`REDUCE_MOTION`, `NO_ANIMATIONS`) after it. The
/// default is the full layer; reduced motion is the user's own ask.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MotionMode {
    /// Everything animates on the runtime's tick.
    #[default]
    Full,
    /// Nothing decorative moves: fills land at once, the streaming
    /// block is still, in-flight marks hold one glyph, transitions are
    /// cuts. The runtime never starts its tick for motion.
    Reduced,
}

impl MotionMode {
    /// The environment variable that decides the mode: `full` or
    /// `reduced`. Read by `Theme::from_os`, the theme's one environment
    /// read.
    pub const ENV: &'static str = "MAGISTR_MOTION";

    /// The conventions `--reduce-motion` and `--accessible` set: any
    /// non-empty value in either asks for [`MotionMode::Reduced`].
    pub const REDUCE_ENVS: [&'static str; 2] = ["REDUCE_MOTION", "NO_ANIMATIONS"];

    /// The mode `value` names, or `None` when it names neither.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "full" | "on" | "motion" => Some(Self::Full),
            "reduced" | "reduce" | "off" | "none" | "no" => Some(Self::Reduced),
            _ => None,
        }
    }

    /// Decide the mode: a parsable `env` value wins; otherwise any
    /// non-empty `reduce` convention asks for reduced motion; with
    /// neither, the full layer.
    #[must_use]
    pub fn detect<'a>(env: Option<&str>, reduce: impl IntoIterator<Item = &'a str>) -> Self {
        if let Some(mode) = env.and_then(Self::parse) {
            return mode;
        }
        let reduced = reduce
            .into_iter()
            .any(|value| !value.trim().is_empty() && value.trim() != "0");
        if reduced { Self::Reduced } else { Self::Full }
    }

    /// Whether decorative motion is on.
    #[must_use]
    pub const fn is_full(self) -> bool {
        matches!(self, Self::Full)
    }
}

#[cfg(test)]
#[path = "motion_mode.tests.rs"]
mod tests;
