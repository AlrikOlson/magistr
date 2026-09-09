//! Color palette definitions — Palette, OwnedPalette, TomlPalette, and built-in palettes.
//!
//! Framework-agnostic: uses `ThemeColor` instead of any rendering library's color type.

use serde::Deserialize;

use crate::color::ThemeColor;
use crate::math;

// ---------------------------------------------------------------------------
// Palette — 16-color palette that defines a theme's color identity
// ---------------------------------------------------------------------------

/// A 16-color palette from which all theme styles are derived.
///
/// Every built-in theme is defined as a `Palette`. Custom themes can be
/// loaded from TOML files with the same structure.
///
/// This type uses `&'static str` for the name — for user-loaded themes,
/// use `OwnedPalette` instead.
#[derive(Debug, Clone)]
pub struct Palette {
    /// Theme display name (e.g. "Catppuccin Mocha")
    pub name: &'static str,
    /// Main background
    pub base: ThemeColor,
    /// Raised surface (panels, header/footer bg)
    pub surface0: ThemeColor,
    /// Higher surface (borders, separators)
    pub surface1: ThemeColor,
    /// Highest surface (reserved for layered UI)
    pub surface2: ThemeColor,
    /// Overlay background (modals, floating elements)
    pub overlay0: ThemeColor,
    /// Higher overlay background
    pub overlay1: ThemeColor,
    /// Primary text
    pub text: ThemeColor,
    /// Secondary text (labels, metadata)
    pub subtext0: ThemeColor,
    /// Tertiary text
    pub subtext1: ThemeColor,
    // --- 8 accent colors ---
    /// Primary accent (headers, focus)
    pub blue: ThemeColor,
    /// Active/running status
    pub sky: ThemeColor,
    /// Success indicators
    pub green: ThemeColor,
    /// Error indicators
    pub red: ThemeColor,
    /// Warning indicators
    pub yellow: ThemeColor,
    /// Phase labels, badges
    pub peach: ThemeColor,
    /// Tool calls, special actions
    pub mauve: ThemeColor,
    /// Titles, decorative accent
    pub lavender: ThemeColor,
}

impl Palette {
    /// Validate WCAG AA contrast ratios for critical text/background pairs.
    ///
    /// Returns a list of violations (empty = all pass).
    ///
    /// This is a report about the palette, not a promise to a consumer:
    /// Catppuccin Latte fails five of these pairs and is still exactly
    /// the palette upstream ships. A consumer that needs legibility
    /// takes it rather than trusting the colours — the cockpit through
    /// its pens, the GUI's stylesheet through [`crate::ink`], both of
    /// which correct against the floors declared here.
    pub fn validate(&self) -> Vec<math::ContrastViolation> {
        let mut violations = Vec::new();

        // Normal text (4.5:1 required for WCAG AA)
        let text_pairs: &[(&str, ThemeColor, ThemeColor)] = &[
            ("text on base", self.text, self.base),
            ("text on surface0", self.text, self.surface0),
            ("subtext0 on base", self.subtext0, self.base),
            ("subtext1 on base", self.subtext1, self.base),
            ("green on base", self.green, self.base),
            ("red on base", self.red, self.base),
            ("yellow on base", self.yellow, self.base),
            ("blue on base", self.blue, self.base),
        ];

        for &(name, fg, bg) in text_pairs {
            if let Some(v) = math::validate_contrast(fg, bg, 4.5, name) {
                violations.push(v);
            }
        }

        // Large text / UI elements (3.0:1 required for WCAG AA)
        let ui_pairs: &[(&str, ThemeColor, ThemeColor)] = &[
            ("overlay0 on base", self.overlay0, self.base),
            ("focus ring (blue) on base", self.blue, self.base),
            ("focus ring (blue) on surface0", self.blue, self.surface0),
        ];

        for &(name, fg, bg) in ui_pairs {
            if let Some(v) = math::validate_contrast(fg, bg, 3.0, name) {
                violations.push(v);
            }
        }

        violations.extend(self.accent_violations());
        violations
    }

    /// Every accent that fails WCAG AA large-text contrast (3:1) on a
    /// background it can be drawn over: the palette's base and its first
    /// two surfaces.
    ///
    /// This is a report, not a promise. Most built-in palettes fail some
    /// of these pairs — Catppuccin Latte's lavender is 2.06:1 on
    /// surface0, its peach 2.64:1 on base — because the eight accents are
    /// a colour identity, not a set of guaranteed foregrounds. A surface
    /// that must draw an accent on a background legibly runs it through
    /// [`math::ensure_contrast`] rather than trusting the palette; the
    /// cockpit's pens do exactly that, which is also what keeps the CVD
    /// remappings legible (they only ever move a signal role onto another
    /// one of these eight accents, so this sweep covers them all).
    #[must_use]
    pub fn accent_violations(&self) -> Vec<math::ContrastViolation> {
        let mut violations = Vec::new();
        let accents: &[(&str, ThemeColor)] = &[
            ("blue", self.blue),
            ("sky", self.sky),
            ("green", self.green),
            ("red", self.red),
            ("yellow", self.yellow),
            ("peach", self.peach),
            ("mauve", self.mauve),
            ("lavender", self.lavender),
        ];
        let grounds: &[(&str, ThemeColor)] = &[
            ("base", self.base),
            ("surface0", self.surface0),
            ("surface1", self.surface1),
        ];
        for &(accent, fg) in accents {
            for &(ground, bg) in grounds {
                let name = format!("accent {accent} on {ground}");
                if let Some(v) = math::validate_contrast(fg, bg, 3.0, &name) {
                    violations.push(v);
                }
            }
        }
        violations
    }

    /// Export this palette as a TOML string suitable for `~/.config/magistr/theme.toml`.
    pub fn to_toml_string(&self) -> String {
        OwnedPalette::from_ref(self).to_toml_string()
    }
}

// ---------------------------------------------------------------------------
// OwnedPalette — owned variant for user-loaded themes
// ---------------------------------------------------------------------------

/// Owned palette with a `String` name — used for user-loaded themes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedPalette {
    pub name: String,
    pub base: ThemeColor,
    pub surface0: ThemeColor,
    pub surface1: ThemeColor,
    pub surface2: ThemeColor,
    pub overlay0: ThemeColor,
    pub overlay1: ThemeColor,
    pub text: ThemeColor,
    pub subtext0: ThemeColor,
    pub subtext1: ThemeColor,
    pub blue: ThemeColor,
    pub sky: ThemeColor,
    pub green: ThemeColor,
    pub red: ThemeColor,
    pub yellow: ThemeColor,
    pub peach: ThemeColor,
    pub mauve: ThemeColor,
    pub lavender: ThemeColor,
}

impl OwnedPalette {
    /// Create an `OwnedPalette` from a `Palette` (copies all colors).
    pub fn from_ref(p: &Palette) -> Self {
        Self {
            name: p.name.to_string(),
            base: p.base,
            surface0: p.surface0,
            surface1: p.surface1,
            surface2: p.surface2,
            overlay0: p.overlay0,
            overlay1: p.overlay1,
            text: p.text,
            subtext0: p.subtext0,
            subtext1: p.subtext1,
            blue: p.blue,
            sky: p.sky,
            green: p.green,
            red: p.red,
            yellow: p.yellow,
            peach: p.peach,
            mauve: p.mauve,
            lavender: p.lavender,
        }
    }

    /// Borrow as a `Palette` reference.
    ///
    /// The returned palette has a static-lifetime name pointing to `"custom"`.
    pub fn as_ref_palette(&self) -> Palette {
        Palette {
            name: "custom",
            base: self.base,
            surface0: self.surface0,
            surface1: self.surface1,
            surface2: self.surface2,
            overlay0: self.overlay0,
            overlay1: self.overlay1,
            text: self.text,
            subtext0: self.subtext0,
            subtext1: self.subtext1,
            blue: self.blue,
            sky: self.sky,
            green: self.green,
            red: self.red,
            yellow: self.yellow,
            peach: self.peach,
            mauve: self.mauve,
            lavender: self.lavender,
        }
    }

    /// Apply color overrides from a `MinimalTomlPalette`.
    ///
    /// Only specified (non-None) fields are overridden. The `name` is always
    /// taken from the override. Used for theme inheritance.
    pub fn apply_overrides(&mut self, overrides: &MinimalTomlPalette) {
        self.name = overrides.name.clone();

        if let Some(c) = ThemeColor::from_hex(&overrides.base) {
            self.base = c;
        }
        if let Some(c) = ThemeColor::from_hex(&overrides.text) {
            self.text = c;
        }
        if let Some(c) = ThemeColor::from_hex(&overrides.blue) {
            self.blue = c;
        }
        if let Some(c) = ThemeColor::from_hex(&overrides.green) {
            self.green = c;
        }
        if let Some(c) = ThemeColor::from_hex(&overrides.red) {
            self.red = c;
        }
        if let Some(c) = ThemeColor::from_hex(&overrides.yellow) {
            self.yellow = c;
        }
        if let Some(c) = ThemeColor::from_hex(&overrides.mauve) {
            self.mauve = c;
        }
        if let Some(c) = parse_optional(&overrides.surface0) {
            self.surface0 = c;
        }
        if let Some(c) = parse_optional(&overrides.surface1) {
            self.surface1 = c;
        }
        if let Some(c) = parse_optional(&overrides.surface2) {
            self.surface2 = c;
        }
        if let Some(c) = parse_optional(&overrides.overlay0) {
            self.overlay0 = c;
        }
        if let Some(c) = parse_optional(&overrides.overlay1) {
            self.overlay1 = c;
        }
        if let Some(c) = parse_optional(&overrides.subtext0) {
            self.subtext0 = c;
        }
        if let Some(c) = parse_optional(&overrides.subtext1) {
            self.subtext1 = c;
        }
        if let Some(c) = parse_optional(&overrides.sky) {
            self.sky = c;
        }
        if let Some(c) = parse_optional(&overrides.peach) {
            self.peach = c;
        }
        if let Some(c) = parse_optional(&overrides.lavender) {
            self.lavender = c;
        }
    }

    /// Export this palette as a TOML string suitable for `~/.config/magistr/theme.toml`.
    pub fn to_toml_string(&self) -> String {
        let mut out = String::with_capacity(512);
        out.push_str(&format!("name = \"{}\"\n", self.name));
        out.push_str(&format!("base = \"{}\"\n", self.base));
        out.push_str(&format!("surface0 = \"{}\"\n", self.surface0));
        out.push_str(&format!("surface1 = \"{}\"\n", self.surface1));
        out.push_str(&format!("surface2 = \"{}\"\n", self.surface2));
        out.push_str(&format!("overlay0 = \"{}\"\n", self.overlay0));
        out.push_str(&format!("overlay1 = \"{}\"\n", self.overlay1));
        out.push_str(&format!("text = \"{}\"\n", self.text));
        out.push_str(&format!("subtext0 = \"{}\"\n", self.subtext0));
        out.push_str(&format!("subtext1 = \"{}\"\n", self.subtext1));
        out.push_str(&format!("blue = \"{}\"\n", self.blue));
        out.push_str(&format!("sky = \"{}\"\n", self.sky));
        out.push_str(&format!("green = \"{}\"\n", self.green));
        out.push_str(&format!("red = \"{}\"\n", self.red));
        out.push_str(&format!("yellow = \"{}\"\n", self.yellow));
        out.push_str(&format!("peach = \"{}\"\n", self.peach));
        out.push_str(&format!("mauve = \"{}\"\n", self.mauve));
        out.push_str(&format!("lavender = \"{}\"\n", self.lavender));
        out
    }
}

// ---------------------------------------------------------------------------
// TOML parsing types
// ---------------------------------------------------------------------------

/// TOML-friendly palette with hex color strings (full 16-color format).
///
/// Requires all 16 color fields. For a minimal format, use `MinimalTomlPalette`.
#[derive(Debug, Deserialize)]
pub struct TomlPalette {
    pub name: String,
    pub base: String,
    pub surface0: String,
    pub surface1: String,
    pub surface2: String,
    pub overlay0: String,
    pub overlay1: String,
    pub text: String,
    pub subtext0: String,
    pub subtext1: String,
    pub blue: String,
    pub sky: String,
    pub green: String,
    pub red: String,
    pub yellow: String,
    pub peach: String,
    pub mauve: String,
    pub lavender: String,
}

impl TomlPalette {
    /// Convert to an `OwnedPalette`.
    ///
    /// Returns `None` if any hex color string is invalid.
    pub fn into_palette(self) -> Option<OwnedPalette> {
        Some(OwnedPalette {
            name: self.name,
            base: ThemeColor::from_hex(&self.base)?,
            surface0: ThemeColor::from_hex(&self.surface0)?,
            surface1: ThemeColor::from_hex(&self.surface1)?,
            surface2: ThemeColor::from_hex(&self.surface2)?,
            overlay0: ThemeColor::from_hex(&self.overlay0)?,
            overlay1: ThemeColor::from_hex(&self.overlay1)?,
            text: ThemeColor::from_hex(&self.text)?,
            subtext0: ThemeColor::from_hex(&self.subtext0)?,
            subtext1: ThemeColor::from_hex(&self.subtext1)?,
            blue: ThemeColor::from_hex(&self.blue)?,
            sky: ThemeColor::from_hex(&self.sky)?,
            green: ThemeColor::from_hex(&self.green)?,
            red: ThemeColor::from_hex(&self.red)?,
            yellow: ThemeColor::from_hex(&self.yellow)?,
            peach: ThemeColor::from_hex(&self.peach)?,
            mauve: ThemeColor::from_hex(&self.mauve)?,
            lavender: ThemeColor::from_hex(&self.lavender)?,
        })
    }
}

/// Minimal TOML palette requiring only ~7 base colors.
///
/// Optional fields are auto-derived from base colors using `lighten()`/`darken()`.
/// Light vs dark detection uses `relative_luminance(base)`.
///
/// Required: `name`, `base`, `text`, `blue`, `green`, `red`, `yellow`, `mauve`
/// Optional: `surface0`..`surface2`, `overlay0`..`overlay1`, `subtext0`..`subtext1`,
///           `sky`, `peach`, `lavender`
#[derive(Debug, Deserialize)]
pub struct MinimalTomlPalette {
    pub name: String,
    // Required base colors
    pub base: String,
    pub text: String,
    pub blue: String,
    pub green: String,
    pub red: String,
    pub yellow: String,
    pub mauve: String,
    // Optional — derived if absent
    pub surface0: Option<String>,
    pub surface1: Option<String>,
    pub surface2: Option<String>,
    pub overlay0: Option<String>,
    pub overlay1: Option<String>,
    pub subtext0: Option<String>,
    pub subtext1: Option<String>,
    pub sky: Option<String>,
    pub peach: Option<String>,
    pub lavender: Option<String>,
}

impl MinimalTomlPalette {
    /// Convert to an `OwnedPalette`, deriving missing colors.
    ///
    /// Returns `None` if any specified hex color is invalid.
    pub fn into_palette(self) -> Option<OwnedPalette> {
        let base = ThemeColor::from_hex(&self.base)?;
        let text = ThemeColor::from_hex(&self.text)?;
        let blue = ThemeColor::from_hex(&self.blue)?;
        let green = ThemeColor::from_hex(&self.green)?;
        let red = ThemeColor::from_hex(&self.red)?;
        let yellow = ThemeColor::from_hex(&self.yellow)?;
        let mauve = ThemeColor::from_hex(&self.mauve)?;

        let light = is_light_base(base);

        let surface0 =
            parse_optional(&self.surface0).unwrap_or_else(|| derive_surface(base, light, 0.05));
        let surface1 =
            parse_optional(&self.surface1).unwrap_or_else(|| derive_surface(base, light, 0.10));
        let surface2 =
            parse_optional(&self.surface2).unwrap_or_else(|| derive_surface(base, light, 0.15));
        let overlay0 =
            parse_optional(&self.overlay0).unwrap_or_else(|| derive_surface(base, light, 0.20));
        let overlay1 =
            parse_optional(&self.overlay1).unwrap_or_else(|| derive_surface(base, light, 0.25));

        let subtext0 = parse_optional(&self.subtext0).unwrap_or_else(|| {
            if light {
                math::lighten(text, 0.15)
            } else {
                math::darken(text, 0.15)
            }
        });
        let subtext1 = parse_optional(&self.subtext1).unwrap_or_else(|| {
            if light {
                math::lighten(text, 0.30)
            } else {
                math::darken(text, 0.30)
            }
        });

        let sky = parse_optional(&self.sky).unwrap_or_else(|| math::lighten(blue, 0.15));
        let peach = parse_optional(&self.peach).unwrap_or_else(|| math::lerp(red, yellow, 0.5));
        let lavender = parse_optional(&self.lavender).unwrap_or_else(|| math::lighten(mauve, 0.20));

        Some(OwnedPalette {
            name: self.name,
            base,
            surface0,
            surface1,
            surface2,
            overlay0,
            overlay1,
            text,
            subtext0,
            subtext1,
            blue,
            sky,
            green,
            red,
            yellow,
            peach,
            mauve,
            lavender,
        })
    }
}

/// Returns `true` if the base color is light (luminance > 0.5).
fn is_light_base(base: ThemeColor) -> bool {
    math::relative_luminance(base) > 0.5
}

/// Derive a surface color from base by stepping away from it.
fn derive_surface(base: ThemeColor, light: bool, amount: f64) -> ThemeColor {
    if light {
        math::darken(base, amount)
    } else {
        math::lighten(base, amount)
    }
}

/// Parse an optional hex color string, returning `None` if absent or invalid.
fn parse_optional(hex: &Option<String>) -> Option<ThemeColor> {
    hex.as_ref().and_then(|s| ThemeColor::from_hex(s))
}

// ---------------------------------------------------------------------------
// Built-in palettes
// ---------------------------------------------------------------------------

/// Catppuccin Mocha — warm dark theme (default).
pub fn catppuccin_mocha() -> Palette {
    Palette {
        name: "catppuccin-mocha",
        base: ThemeColor::new(30, 30, 46),
        surface0: ThemeColor::new(49, 50, 68),
        surface1: ThemeColor::new(69, 71, 90),
        surface2: ThemeColor::new(88, 91, 112),
        overlay0: ThemeColor::new(108, 112, 134),
        overlay1: ThemeColor::new(127, 132, 156),
        text: ThemeColor::new(205, 214, 244),
        subtext0: ThemeColor::new(166, 173, 200),
        subtext1: ThemeColor::new(186, 194, 222),
        blue: ThemeColor::new(137, 180, 250),
        sky: ThemeColor::new(137, 220, 235),
        green: ThemeColor::new(166, 227, 161),
        red: ThemeColor::new(243, 139, 168),
        yellow: ThemeColor::new(249, 226, 175),
        peach: ThemeColor::new(250, 179, 135),
        mauve: ThemeColor::new(203, 166, 247),
        lavender: ThemeColor::new(180, 190, 254),
    }
}

/// Catppuccin Latte — light theme for bright terminals.
pub fn catppuccin_latte() -> Palette {
    Palette {
        name: "catppuccin-latte",
        base: ThemeColor::new(239, 241, 245),
        surface0: ThemeColor::new(204, 208, 218),
        surface1: ThemeColor::new(188, 192, 204),
        surface2: ThemeColor::new(172, 176, 190),
        overlay0: ThemeColor::new(156, 160, 176),
        overlay1: ThemeColor::new(140, 143, 161),
        text: ThemeColor::new(76, 79, 105),
        subtext0: ThemeColor::new(108, 111, 133),
        subtext1: ThemeColor::new(92, 95, 119),
        blue: ThemeColor::new(30, 102, 245),
        sky: ThemeColor::new(4, 165, 229),
        green: ThemeColor::new(64, 160, 43),
        red: ThemeColor::new(210, 15, 57),
        yellow: ThemeColor::new(223, 142, 29),
        peach: ThemeColor::new(254, 100, 11),
        mauve: ThemeColor::new(136, 57, 239),
        lavender: ThemeColor::new(114, 135, 253),
    }
}

/// Tokyo Night — deep navy/purple palette.
pub fn tokyo_night() -> Palette {
    Palette {
        name: "tokyo-night",
        base: ThemeColor::new(26, 27, 38),
        surface0: ThemeColor::new(36, 40, 59),
        surface1: ThemeColor::new(55, 59, 82),
        surface2: ThemeColor::new(65, 72, 104),
        overlay0: ThemeColor::new(86, 95, 137),
        overlay1: ThemeColor::new(96, 106, 150),
        text: ThemeColor::new(192, 202, 245),
        subtext0: ThemeColor::new(169, 177, 214),
        subtext1: ThemeColor::new(139, 147, 185),
        blue: ThemeColor::new(122, 162, 247),
        sky: ThemeColor::new(125, 207, 255),
        green: ThemeColor::new(158, 206, 106),
        red: ThemeColor::new(247, 118, 142),
        yellow: ThemeColor::new(224, 175, 104),
        peach: ThemeColor::new(255, 158, 100),
        mauve: ThemeColor::new(187, 154, 247),
        lavender: ThemeColor::new(193, 168, 255),
    }
}

/// Dracula — classic high-contrast dark with purples, pinks, greens.
pub fn dracula() -> Palette {
    Palette {
        name: "dracula",
        base: ThemeColor::new(40, 42, 54),
        surface0: ThemeColor::new(52, 54, 71),
        surface1: ThemeColor::new(68, 71, 90),
        surface2: ThemeColor::new(85, 89, 110),
        overlay0: ThemeColor::new(98, 114, 164),
        overlay1: ThemeColor::new(119, 133, 179),
        text: ThemeColor::new(248, 248, 242),
        subtext0: ThemeColor::new(210, 210, 220),
        subtext1: ThemeColor::new(189, 147, 249),
        blue: ThemeColor::new(139, 233, 253),
        sky: ThemeColor::new(139, 233, 253),
        green: ThemeColor::new(80, 250, 123),
        red: ThemeColor::new(255, 85, 85),
        yellow: ThemeColor::new(241, 250, 140),
        peach: ThemeColor::new(255, 184, 108),
        mauve: ThemeColor::new(189, 147, 249),
        lavender: ThemeColor::new(202, 169, 255),
    }
}

/// Nord — cool, polar-inspired blues with gentle contrast.
pub fn nord() -> Palette {
    Palette {
        name: "nord",
        base: ThemeColor::new(46, 52, 64),
        surface0: ThemeColor::new(59, 66, 82),
        surface1: ThemeColor::new(67, 76, 94),
        surface2: ThemeColor::new(76, 86, 106),
        overlay0: ThemeColor::new(113, 128, 147),
        overlay1: ThemeColor::new(143, 157, 174),
        text: ThemeColor::new(236, 239, 244),
        subtext0: ThemeColor::new(229, 233, 240),
        subtext1: ThemeColor::new(216, 222, 233),
        blue: ThemeColor::new(136, 192, 208),
        sky: ThemeColor::new(143, 188, 187),
        green: ThemeColor::new(163, 190, 140),
        red: ThemeColor::new(191, 97, 106),
        yellow: ThemeColor::new(235, 203, 139),
        peach: ThemeColor::new(208, 135, 112),
        mauve: ThemeColor::new(180, 142, 173),
        lavender: ThemeColor::new(129, 161, 193),
    }
}

/// Gruvbox Dark — warm, retro earth tones with strong legibility.
pub fn gruvbox_dark() -> Palette {
    Palette {
        name: "gruvbox-dark",
        base: ThemeColor::new(40, 40, 40),
        surface0: ThemeColor::new(60, 56, 54),
        surface1: ThemeColor::new(80, 73, 69),
        surface2: ThemeColor::new(102, 92, 84),
        overlay0: ThemeColor::new(124, 111, 100),
        overlay1: ThemeColor::new(146, 131, 116),
        text: ThemeColor::new(235, 219, 178),
        subtext0: ThemeColor::new(213, 196, 161),
        subtext1: ThemeColor::new(189, 174, 147),
        blue: ThemeColor::new(131, 165, 152),
        sky: ThemeColor::new(69, 133, 136),
        green: ThemeColor::new(184, 187, 38),
        red: ThemeColor::new(251, 73, 52),
        yellow: ThemeColor::new(250, 189, 47),
        peach: ThemeColor::new(254, 128, 25),
        mauve: ThemeColor::new(211, 134, 155),
        lavender: ThemeColor::new(177, 98, 134),
    }
}

/// Gruvbox Light — light variant for bright terminal backgrounds.
pub fn gruvbox_light() -> Palette {
    Palette {
        name: "gruvbox-light",
        base: ThemeColor::new(251, 241, 199),
        surface0: ThemeColor::new(242, 229, 188),
        surface1: ThemeColor::new(213, 196, 161),
        surface2: ThemeColor::new(189, 174, 147),
        overlay0: ThemeColor::new(168, 153, 132),
        overlay1: ThemeColor::new(146, 131, 116),
        text: ThemeColor::new(60, 56, 54),
        subtext0: ThemeColor::new(80, 73, 69),
        subtext1: ThemeColor::new(102, 92, 84),
        blue: ThemeColor::new(7, 102, 120),
        sky: ThemeColor::new(69, 133, 136),
        green: ThemeColor::new(121, 116, 14),
        red: ThemeColor::new(204, 36, 29),
        yellow: ThemeColor::new(181, 118, 20),
        peach: ThemeColor::new(175, 58, 3),
        mauve: ThemeColor::new(177, 98, 134),
        lavender: ThemeColor::new(143, 63, 113),
    }
}

/// Solarized Dark — Ethan Schoonover's precision-engineered palette.
pub fn solarized_dark() -> Palette {
    Palette {
        name: "solarized-dark",
        base: ThemeColor::new(0, 43, 54),
        surface0: ThemeColor::new(7, 54, 66),
        surface1: ThemeColor::new(88, 110, 117),
        surface2: ThemeColor::new(101, 123, 131),
        overlay0: ThemeColor::new(131, 148, 150),
        overlay1: ThemeColor::new(147, 161, 161),
        text: ThemeColor::new(253, 246, 227),
        subtext0: ThemeColor::new(238, 232, 213),
        subtext1: ThemeColor::new(147, 161, 161),
        blue: ThemeColor::new(38, 139, 210),
        sky: ThemeColor::new(42, 161, 152),
        green: ThemeColor::new(133, 153, 0),
        red: ThemeColor::new(220, 50, 47),
        yellow: ThemeColor::new(181, 137, 0),
        peach: ThemeColor::new(203, 75, 22),
        mauve: ThemeColor::new(108, 113, 196),
        lavender: ThemeColor::new(211, 54, 130),
    }
}

/// Solarized Light — light variant with identical accent colors.
pub fn solarized_light() -> Palette {
    Palette {
        name: "solarized-light",
        base: ThemeColor::new(253, 246, 227),
        surface0: ThemeColor::new(238, 232, 213),
        surface1: ThemeColor::new(147, 161, 161),
        surface2: ThemeColor::new(131, 148, 150),
        overlay0: ThemeColor::new(101, 123, 131),
        overlay1: ThemeColor::new(88, 110, 117),
        text: ThemeColor::new(0, 43, 54),
        subtext0: ThemeColor::new(7, 54, 66),
        subtext1: ThemeColor::new(88, 110, 117),
        blue: ThemeColor::new(38, 139, 210),
        sky: ThemeColor::new(42, 161, 152),
        green: ThemeColor::new(133, 153, 0),
        red: ThemeColor::new(220, 50, 47),
        yellow: ThemeColor::new(181, 137, 0),
        peach: ThemeColor::new(203, 75, 22),
        mauve: ThemeColor::new(108, 113, 196),
        lavender: ThemeColor::new(211, 54, 130),
    }
}

/// One Dark — Atom-inspired with vibrant but balanced accents.
pub fn one_dark() -> Palette {
    Palette {
        name: "one-dark",
        base: ThemeColor::new(40, 44, 52),
        surface0: ThemeColor::new(49, 53, 63),
        surface1: ThemeColor::new(62, 68, 81),
        surface2: ThemeColor::new(76, 82, 99),
        overlay0: ThemeColor::new(92, 99, 112),
        overlay1: ThemeColor::new(114, 121, 135),
        text: ThemeColor::new(171, 178, 191),
        subtext0: ThemeColor::new(150, 158, 172),
        subtext1: ThemeColor::new(130, 137, 151),
        blue: ThemeColor::new(97, 175, 239),
        sky: ThemeColor::new(86, 182, 194),
        green: ThemeColor::new(152, 195, 121),
        red: ThemeColor::new(224, 108, 117),
        yellow: ThemeColor::new(229, 192, 123),
        peach: ThemeColor::new(209, 154, 102),
        mauve: ThemeColor::new(198, 120, 221),
        lavender: ThemeColor::new(170, 142, 230),
    }
}

/// Rosé Pine — muted, elegant palette with natural tones.
pub fn rose_pine() -> Palette {
    Palette {
        name: "rose-pine",
        base: ThemeColor::new(25, 23, 36),
        surface0: ThemeColor::new(30, 28, 43),
        surface1: ThemeColor::new(38, 35, 58),
        surface2: ThemeColor::new(57, 53, 82),
        overlay0: ThemeColor::new(110, 106, 134),
        overlay1: ThemeColor::new(144, 140, 170),
        text: ThemeColor::new(224, 222, 244),
        subtext0: ThemeColor::new(144, 140, 170),
        subtext1: ThemeColor::new(110, 106, 134),
        blue: ThemeColor::new(49, 116, 143),
        sky: ThemeColor::new(156, 207, 216),
        green: ThemeColor::new(156, 207, 216),
        red: ThemeColor::new(235, 111, 146),
        yellow: ThemeColor::new(246, 193, 119),
        peach: ThemeColor::new(234, 154, 151),
        mauve: ThemeColor::new(196, 167, 231),
        lavender: ThemeColor::new(235, 188, 186),
    }
}

/// Kanagawa — Japanese ink-wash inspired, subtle and distinctive.
pub fn kanagawa() -> Palette {
    Palette {
        name: "kanagawa",
        base: ThemeColor::new(31, 31, 40),
        surface0: ThemeColor::new(42, 42, 55),
        surface1: ThemeColor::new(54, 54, 70),
        surface2: ThemeColor::new(84, 84, 109),
        overlay0: ThemeColor::new(114, 113, 105),
        overlay1: ThemeColor::new(146, 142, 135),
        text: ThemeColor::new(220, 215, 186),
        subtext0: ThemeColor::new(200, 195, 168),
        subtext1: ThemeColor::new(169, 164, 142),
        blue: ThemeColor::new(126, 156, 216),
        sky: ThemeColor::new(127, 180, 202),
        green: ThemeColor::new(152, 187, 108),
        red: ThemeColor::new(195, 64, 67),
        yellow: ThemeColor::new(226, 194, 110),
        peach: ThemeColor::new(255, 169, 98),
        mauve: ThemeColor::new(148, 130, 196),
        lavender: ThemeColor::new(210, 126, 153),
    }
}

/// CVD-Optimized — curated colors that remain distinguishable across all CVD types.
///
/// Based on the Okabe-Ito palette.
pub fn cvd_optimized() -> Palette {
    Palette {
        name: "cvd-optimized",
        base: ThemeColor::new(30, 30, 35),
        surface0: ThemeColor::new(42, 42, 48),
        surface1: ThemeColor::new(58, 58, 65),
        surface2: ThemeColor::new(80, 80, 88),
        overlay0: ThemeColor::new(110, 110, 120),
        overlay1: ThemeColor::new(140, 140, 150),
        text: ThemeColor::new(230, 230, 230),
        subtext0: ThemeColor::new(190, 190, 195),
        subtext1: ThemeColor::new(150, 150, 158),
        blue: ThemeColor::new(80, 158, 220),
        sky: ThemeColor::new(86, 180, 233),
        green: ThemeColor::new(0, 178, 130),
        red: ThemeColor::new(240, 120, 40),
        yellow: ThemeColor::new(240, 228, 66),
        peach: ThemeColor::new(240, 175, 30),
        mauve: ThemeColor::new(210, 140, 180),
        lavender: ThemeColor::new(170, 170, 255),
    }
}

/// Returns all built-in palettes.
pub fn all_builtin_palettes() -> Vec<Palette> {
    vec![
        catppuccin_mocha(),
        catppuccin_latte(),
        tokyo_night(),
        dracula(),
        nord(),
        gruvbox_dark(),
        gruvbox_light(),
        solarized_dark(),
        solarized_light(),
        one_dark(),
        rose_pine(),
        kanagawa(),
        cvd_optimized(),
    ]
}

#[cfg(test)]
#[path = "palette.tests.rs"]
mod tests;
