//! Semantic color token system — maps UI intent to palette colors.
//!
//! The semantic layer decouples widget code from raw palette colors. Themes
//! define a `ThemeMapping` that says "primary accent uses the `blue` palette
//! slot" — custom themes can override this to use any palette color for any
//! semantic role.

use serde::Deserialize;

use crate::color::ThemeColor;
use crate::palette::Palette;

// ---------------------------------------------------------------------------
// PaletteField — identifies a color slot within a Palette
// ---------------------------------------------------------------------------

/// Identifies one of the 17 color slots in a `Palette`.
///
/// Used by `ThemeMapping` to specify which palette color serves a semantic role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaletteField {
    Base,
    Surface0,
    Surface1,
    Surface2,
    Overlay0,
    Overlay1,
    Text,
    Subtext0,
    Subtext1,
    Blue,
    Sky,
    Green,
    Red,
    Yellow,
    Peach,
    Mauve,
    Lavender,
}

impl PaletteField {
    /// All palette fields in declaration order.
    pub const ALL: [PaletteField; 17] = [
        Self::Base,
        Self::Surface0,
        Self::Surface1,
        Self::Surface2,
        Self::Overlay0,
        Self::Overlay1,
        Self::Text,
        Self::Subtext0,
        Self::Subtext1,
        Self::Blue,
        Self::Sky,
        Self::Green,
        Self::Red,
        Self::Yellow,
        Self::Peach,
        Self::Mauve,
        Self::Lavender,
    ];

    /// Resolve this field to an actual color from a palette.
    pub fn resolve(self, palette: &Palette) -> ThemeColor {
        match self {
            Self::Base => palette.base,
            Self::Surface0 => palette.surface0,
            Self::Surface1 => palette.surface1,
            Self::Surface2 => palette.surface2,
            Self::Overlay0 => palette.overlay0,
            Self::Overlay1 => palette.overlay1,
            Self::Text => palette.text,
            Self::Subtext0 => palette.subtext0,
            Self::Subtext1 => palette.subtext1,
            Self::Blue => palette.blue,
            Self::Sky => palette.sky,
            Self::Green => palette.green,
            Self::Red => palette.red,
            Self::Yellow => palette.yellow,
            Self::Peach => palette.peach,
            Self::Mauve => palette.mauve,
            Self::Lavender => palette.lavender,
        }
    }

    /// Parse a palette field name from a string (case-insensitive).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "base" => Some(Self::Base),
            "surface0" => Some(Self::Surface0),
            "surface1" => Some(Self::Surface1),
            "surface2" => Some(Self::Surface2),
            "overlay0" => Some(Self::Overlay0),
            "overlay1" => Some(Self::Overlay1),
            "text" => Some(Self::Text),
            "subtext0" => Some(Self::Subtext0),
            "subtext1" => Some(Self::Subtext1),
            "blue" => Some(Self::Blue),
            "sky" => Some(Self::Sky),
            "green" => Some(Self::Green),
            "red" => Some(Self::Red),
            "yellow" => Some(Self::Yellow),
            "peach" => Some(Self::Peach),
            "mauve" => Some(Self::Mauve),
            "lavender" => Some(Self::Lavender),
            _ => None,
        }
    }

    /// The string name for this field (lowercase, matches TOML key).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Surface0 => "surface0",
            Self::Surface1 => "surface1",
            Self::Surface2 => "surface2",
            Self::Overlay0 => "overlay0",
            Self::Overlay1 => "overlay1",
            Self::Text => "text",
            Self::Subtext0 => "subtext0",
            Self::Subtext1 => "subtext1",
            Self::Blue => "blue",
            Self::Sky => "sky",
            Self::Green => "green",
            Self::Red => "red",
            Self::Yellow => "yellow",
            Self::Peach => "peach",
            Self::Mauve => "mauve",
            Self::Lavender => "lavender",
        }
    }
}

// ---------------------------------------------------------------------------
// SemanticRole — UI intent independent of any specific color
// ---------------------------------------------------------------------------

/// Semantic color roles that define UI intent rather than specific colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticRole {
    /// Primary accent — headers, active items, focused borders.
    Primary,
    /// Secondary — labels, badges, metadata.
    Secondary,
    /// Surface — panel backgrounds, raised areas.
    Surface,
    /// On-surface — text that sits on surface-colored backgrounds.
    OnSurface,
    /// Error — failures, destructive actions, gate failures.
    Error,
    /// Warning — attention needed, budget alerts, pending review.
    Warning,
    /// Success — pass states, completions, file saved.
    Success,
    /// Info — informational, neutral states, thinking, searching.
    Info,
    /// Muted — dim text, help text, separators, timestamps.
    Muted,
    /// Accent — decorative, titles, branding, secondary emphasis.
    Accent,
    /// Highlight — search matches, selection, hover states.
    Highlight,
}

impl SemanticRole {
    /// All semantic roles in declaration order.
    pub const ALL: [SemanticRole; 11] = [
        Self::Primary,
        Self::Secondary,
        Self::Surface,
        Self::OnSurface,
        Self::Error,
        Self::Warning,
        Self::Success,
        Self::Info,
        Self::Muted,
        Self::Accent,
        Self::Highlight,
    ];

    /// The string name for this role (lowercase, matches TOML key).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Surface => "surface",
            Self::OnSurface => "on_surface",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Success => "success",
            Self::Info => "info",
            Self::Muted => "muted",
            Self::Accent => "accent",
            Self::Highlight => "highlight",
        }
    }

    /// Parse a semantic role name from a string (case-insensitive).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "primary" => Some(Self::Primary),
            "secondary" => Some(Self::Secondary),
            "surface" => Some(Self::Surface),
            "on_surface" | "onsurface" => Some(Self::OnSurface),
            "error" => Some(Self::Error),
            "warning" => Some(Self::Warning),
            "success" => Some(Self::Success),
            "info" => Some(Self::Info),
            "muted" => Some(Self::Muted),
            "accent" => Some(Self::Accent),
            "highlight" => Some(Self::Highlight),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// ThemeMapping — maps semantic roles to palette fields
// ---------------------------------------------------------------------------

/// Maps each `SemanticRole` to a `PaletteField`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeMapping {
    pub primary: PaletteField,
    pub secondary: PaletteField,
    pub surface: PaletteField,
    pub on_surface: PaletteField,
    pub error: PaletteField,
    pub warning: PaletteField,
    pub success: PaletteField,
    pub info: PaletteField,
    pub muted: PaletteField,
    pub accent: PaletteField,
    pub highlight: PaletteField,
}

impl ThemeMapping {
    /// Resolve a semantic role to the corresponding `PaletteField`.
    pub fn field_for(&self, role: SemanticRole) -> PaletteField {
        match role {
            SemanticRole::Primary => self.primary,
            SemanticRole::Secondary => self.secondary,
            SemanticRole::Surface => self.surface,
            SemanticRole::OnSurface => self.on_surface,
            SemanticRole::Error => self.error,
            SemanticRole::Warning => self.warning,
            SemanticRole::Success => self.success,
            SemanticRole::Info => self.info,
            SemanticRole::Muted => self.muted,
            SemanticRole::Accent => self.accent,
            SemanticRole::Highlight => self.highlight,
        }
    }

    /// Resolve a semantic role to an actual color from a palette.
    pub fn resolve(&self, role: SemanticRole, palette: &Palette) -> ThemeColor {
        self.field_for(role).resolve(palette)
    }

    /// Apply overrides from a TOML mapping.
    ///
    /// Only fields present in `toml` are overridden; others keep their current values.
    pub fn apply_toml_overrides(&mut self, toml: &TomlThemeMapping) {
        if let Some(ref v) = toml.primary
            && let Some(f) = PaletteField::from_name(v)
        {
            self.primary = f;
        }
        if let Some(ref v) = toml.secondary
            && let Some(f) = PaletteField::from_name(v)
        {
            self.secondary = f;
        }
        if let Some(ref v) = toml.surface
            && let Some(f) = PaletteField::from_name(v)
        {
            self.surface = f;
        }
        if let Some(ref v) = toml.on_surface
            && let Some(f) = PaletteField::from_name(v)
        {
            self.on_surface = f;
        }
        if let Some(ref v) = toml.error
            && let Some(f) = PaletteField::from_name(v)
        {
            self.error = f;
        }
        if let Some(ref v) = toml.warning
            && let Some(f) = PaletteField::from_name(v)
        {
            self.warning = f;
        }
        if let Some(ref v) = toml.success
            && let Some(f) = PaletteField::from_name(v)
        {
            self.success = f;
        }
        if let Some(ref v) = toml.info
            && let Some(f) = PaletteField::from_name(v)
        {
            self.info = f;
        }
        if let Some(ref v) = toml.muted
            && let Some(f) = PaletteField::from_name(v)
        {
            self.muted = f;
        }
        if let Some(ref v) = toml.accent
            && let Some(f) = PaletteField::from_name(v)
        {
            self.accent = f;
        }
        if let Some(ref v) = toml.highlight
            && let Some(f) = PaletteField::from_name(v)
        {
            self.highlight = f;
        }
    }
}

impl Default for ThemeMapping {
    fn default() -> Self {
        Self {
            primary: PaletteField::Blue,
            secondary: PaletteField::Subtext0,
            surface: PaletteField::Surface0,
            on_surface: PaletteField::Text,
            error: PaletteField::Red,
            warning: PaletteField::Yellow,
            success: PaletteField::Green,
            info: PaletteField::Sky,
            muted: PaletteField::Overlay0,
            accent: PaletteField::Lavender,
            highlight: PaletteField::Surface2,
        }
    }
}

// ---------------------------------------------------------------------------
// TomlThemeMapping — optional [mapping] section in theme TOML files
// ---------------------------------------------------------------------------

/// TOML-friendly semantic mapping with optional string fields.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TomlThemeMapping {
    pub primary: Option<String>,
    pub secondary: Option<String>,
    pub surface: Option<String>,
    pub on_surface: Option<String>,
    pub error: Option<String>,
    pub warning: Option<String>,
    pub success: Option<String>,
    pub info: Option<String>,
    pub muted: Option<String>,
    pub accent: Option<String>,
    pub highlight: Option<String>,
}

impl TomlThemeMapping {
    /// Returns `true` if no overrides are specified.
    pub fn is_empty(&self) -> bool {
        self.primary.is_none()
            && self.secondary.is_none()
            && self.surface.is_none()
            && self.on_surface.is_none()
            && self.error.is_none()
            && self.warning.is_none()
            && self.success.is_none()
            && self.info.is_none()
            && self.muted.is_none()
            && self.accent.is_none()
            && self.highlight.is_none()
    }

    /// Build a `ThemeMapping` from defaults with these overrides applied.
    pub fn into_mapping(self) -> ThemeMapping {
        let mut mapping = ThemeMapping::default();
        mapping.apply_toml_overrides(&self);
        mapping
    }
}

#[cfg(test)]
#[path = "mapping.tests.rs"]
mod tests;
