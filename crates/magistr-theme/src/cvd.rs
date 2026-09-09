//! Color Vision Deficiency (CVD) support — remaps semantic colors for accessibility.
//!
//! Three CVD modes remap the theme's semantic color assignments so that
//! critical status signals (pass/fail/warn/info) remain visually distinct
//! for users with common forms of color blindness.

use crate::mapping::{PaletteField, SemanticRole, ThemeMapping};

/// Color vision deficiency mode.
///
/// Each variant remaps the theme's semantic roles to avoid colors that are
/// indistinguishable for that type of color blindness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CvdMode {
    /// Red-green color blindness (most common, ~6% of males).
    Deuteranopia,
    /// Red-blind — red appears dark/black.
    Protanopia,
    /// Blue-yellow color blindness (rare, ~0.01%).
    Tritanopia,
}

impl CvdMode {
    /// Parse a CVD mode from a string (case-insensitive).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "deuteranopia" => Some(Self::Deuteranopia),
            "protanopia" => Some(Self::Protanopia),
            "tritanopia" => Some(Self::Tritanopia),
            _ => None,
        }
    }

    /// The string name for this mode (lowercase).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deuteranopia => "deuteranopia",
            Self::Protanopia => "protanopia",
            Self::Tritanopia => "tritanopia",
        }
    }

    /// Human-readable label for display in UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Deuteranopia => "Deuteranopia",
            Self::Protanopia => "Protanopia",
            Self::Tritanopia => "Tritanopia",
        }
    }

    /// Cycle to the next CVD mode.
    pub fn next(self) -> Self {
        match self {
            Self::Deuteranopia => Self::Protanopia,
            Self::Protanopia => Self::Tritanopia,
            Self::Tritanopia => Self::Deuteranopia,
        }
    }

    /// Apply CVD-safe remappings to a `ThemeMapping`.
    ///
    /// | Role    | Default   | Deuteranopia     | Protanopia       | Tritanopia       |
    /// |---------|-----------|------------------|------------------|------------------|
    /// | Error   | Red       | Peach (orange)   | Peach (orange)   | Red (unchanged)  |
    /// | Success | Green     | Blue             | Blue             | Green (unchanged)|
    /// | Warning | Yellow    | Yellow (unchanged)| Yellow (unchanged)| Peach (orange)  |
    /// | Info    | Sky       | Lavender         | Lavender         | Mauve            |
    pub fn apply_to_mapping(self, mapping: &ThemeMapping) -> ThemeMapping {
        let mut m = mapping.clone();
        match self {
            Self::Deuteranopia => {
                m.error = PaletteField::Peach;
                m.success = PaletteField::Blue;
                m.info = PaletteField::Lavender;
            }
            Self::Protanopia => {
                m.error = PaletteField::Peach;
                m.success = PaletteField::Blue;
                m.info = PaletteField::Lavender;
            }
            Self::Tritanopia => {
                m.warning = PaletteField::Peach;
                m.info = PaletteField::Mauve;
            }
        }
        m
    }
}

/// Return a distinguishing icon for a semantic signal role.
///
/// Non-signal roles return `None`.
pub fn signal_icon(role: SemanticRole) -> Option<&'static str> {
    match role {
        SemanticRole::Error => Some("\u{2717}"),   // ✗
        SemanticRole::Warning => Some("\u{26A0}"), // ⚠
        SemanticRole::Success => Some("\u{2713}"), // ✓
        SemanticRole::Info => Some("\u{2139}"),    // ℹ
        _ => None,
    }
}

/// Signal icon set for convenient access.
pub struct SignalIcons {
    pub error: &'static str,
    pub warning: &'static str,
    pub success: &'static str,
    pub info: &'static str,
}

impl SignalIcons {
    /// The standard signal icon set.
    pub const STANDARD: Self = Self {
        error: "\u{2717}",
        warning: "\u{26A0}",
        success: "\u{2713}",
        info: "\u{2139}",
    };

    /// ASCII-safe fallback icons.
    pub const ASCII: Self = Self {
        error: "X",
        warning: "!",
        success: "V",
        info: "i",
    };
}

/// Audit a `ThemeMapping` for CVD safety.
///
/// Returns `true` if the four critical signal roles all map to distinct palette fields.
#[must_use]
pub fn is_cvd_safe(mapping: &ThemeMapping) -> bool {
    let signals = [
        mapping.error,
        mapping.warning,
        mapping.success,
        mapping.info,
    ];
    for i in 0..signals.len() {
        for j in (i + 1)..signals.len() {
            if signals[i] == signals[j] {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
#[path = "cvd.tests.rs"]
mod tests;
