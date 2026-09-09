//! Which glyph vocabulary the cockpit draws with.

use super::icons::Icons;

/// The icon vocabulary in force: the Nerd Font set, or the safe Unicode
/// set every terminal font can draw.
///
/// Nerd Font glyphs live in the Private Use Area, so a terminal whose
/// font lacks them draws tofu boxes rather than falling back to
/// something readable. There is no escape sequence that asks a terminal
/// whether its font has them ([`IconMode::detect`]'s only honest
/// signals are the user's own answer and the handful of terminals that
/// bundle a symbol fallback), so the default is [`IconMode::Unicode`]:
/// the set that is always legible, and the one the screenshot gate's
/// embedded face rasterizes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum IconMode {
    /// Nerd Font icons — a patched font, or a terminal that bundles the
    /// symbols as a fallback face.
    Nerd,
    /// Geometric shapes and dingbats: what any monospace font draws.
    #[default]
    Unicode,
}

impl IconMode {
    /// The environment variable that decides the mode: `nerd` or
    /// `unicode`. Read by `Theme::from_os`, the theme's one environment
    /// read.
    pub const ENV: &'static str = "MAGISTR_ICONS";

    /// The environment variable naming the terminal, which
    /// [`IconMode::detect`] matches against the ones that bundle the
    /// symbols.
    pub const TERM_ENV: &'static str = "TERM_PROGRAM";

    /// The terminals that ship a Nerd Font symbol fallback, so their
    /// users get the icons without patching a font. Matched against
    /// `TERM_PROGRAM`, case-insensitively.
    const BUNDLED: [&'static str; 2] = ["wezterm", "ghostty"];

    /// The mode `value` names, or `None` when it names neither.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "nerd" | "nerdfont" | "nerd-font" => Some(Self::Nerd),
            "unicode" | "plain" => Some(Self::Unicode),
            _ => None,
        }
    }

    /// Decide the mode: a parsable `env` value wins; otherwise a
    /// `term_program` that bundles the symbols gets them; with neither,
    /// the Unicode set.
    #[must_use]
    pub fn detect(env: Option<&str>, term_program: Option<&str>) -> Self {
        if let Some(mode) = env.and_then(Self::parse) {
            return mode;
        }
        let bundled = term_program.is_some_and(|program| {
            let program = program.trim().to_ascii_lowercase();
            Self::BUNDLED.contains(&program.as_str())
        });
        if bundled { Self::Nerd } else { Self::Unicode }
    }

    /// The glyphs this mode draws.
    #[must_use]
    pub const fn icons(self) -> &'static Icons {
        match self {
            Self::Nerd => &Icons::NERD,
            Self::Unicode => &Icons::UNICODE,
        }
    }
}

#[cfg(test)]
#[path = "icon_mode.tests.rs"]
mod tests;
