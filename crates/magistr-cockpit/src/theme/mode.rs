//! Dark or light: the one thing the palette choice depends on.

use std::fmt;

use magistr_theme::{Palette, ThemeColor, catppuccin_latte, catppuccin_mocha, math};

use crate::render::Color;

/// Whether the cockpit draws for a dark or a light terminal background.
///
/// Dark is the default: a terminal that never answers OSC 11, or answers
/// with something other than an RGB colour, gives no evidence of a light
/// background.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemeMode {
    /// A dark background: Catppuccin Mocha.
    #[default]
    Dark,
    /// A light background: Catppuccin Latte.
    Light,
}

impl ThemeMode {
    /// A background whose WCAG relative luminance exceeds this is light.
    const LIGHT_ABOVE: f64 = 0.5;

    /// `MAGISTR_THEME`'s vocabulary: `dark` or `light`, case-insensitive,
    /// surrounding whitespace ignored. Anything else is nothing — there is
    /// no picker, so a palette name is not an answer.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.eq_ignore_ascii_case("dark") {
            Some(Self::Dark)
        } else if value.eq_ignore_ascii_case("light") {
            Some(Self::Light)
        } else {
            None
        }
    }

    /// The mode a terminal background calls for: light when its WCAG 2.1
    /// relative luminance (linearised sRGB, `0.2126 R + 0.7152 G + 0.0722 B`)
    /// exceeds `0.5`, the threshold magistr-theme itself uses to derive
    /// surfaces from a base colour.
    #[must_use]
    pub fn for_background(background: Color) -> Self {
        match background {
            Color::Rgb(r, g, b)
                if math::relative_luminance(ThemeColor::new(r, g, b)) > Self::LIGHT_ABOVE =>
            {
                Self::Light
            }
            _ => Self::Dark,
        }
    }

    /// The palette this mode draws with — both are magistr-theme's.
    #[must_use]
    pub fn palette(self) -> Palette {
        match self {
            Self::Dark => catppuccin_mocha(),
            Self::Light => catppuccin_latte(),
        }
    }

    /// The name [`ThemeMode::parse`] accepts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

impl fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "mode.tests.rs"]
mod tests;
