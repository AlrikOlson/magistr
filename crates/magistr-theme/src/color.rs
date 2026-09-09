//! Framework-agnostic RGB color type and hex parsing utilities.

/// A simple RGB color — no dependency on any rendering framework.
///
/// This is the shared color representation used in palette definitions.
/// Frontend crates convert to their native color types:
/// - TUI: `ratatui::style::Color::Rgb(c.r, c.g, c.b)`
/// - GUI: CSS hex string via `to_hex()`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThemeColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ThemeColor {
    /// Create a new RGB color.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Format as a CSS hex string (e.g. `#89b4fa`).
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Parse a hex color string (e.g. `"#89b4fa"` or `"89b4fa"`).
    ///
    /// Returns `None` if the string is not a valid 6-digit hex color.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Self { r, g, b })
    }
}

impl std::fmt::Display for ThemeColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[cfg(test)]
#[path = "color.tests.rs"]
mod tests;
