//! magistr-theme — rendering-agnostic theme definitions for Magistr.
//!
//! This crate defines color palettes, semantic mappings, TOML parsing,
//! and CSS generation shared between the desktop and command-line consumers frontends.
//! It has zero rendering dependencies — no ratatui, no web framework.
//!
//! # Usage
//!
//! ```rust
//! use magistr_theme::{catppuccin_mocha, ThemeMapping, css};
//!
//! let palette = catppuccin_mocha();
//! let mapping = ThemeMapping::default();
//!
//! // Generate CSS custom properties for GUI
//! let css_props = css::palette_to_css(&palette, &mapping);
//!
//! // Access palette colors directly
//! let primary_color = mapping.resolve(magistr_theme::SemanticRole::Primary, &palette);
//! let hex = primary_color.to_hex(); // "#89b4fa"
//! ```

pub mod ansi;
pub mod color;
pub mod css;
pub mod cvd;
pub mod ink;
pub mod mapping;
pub mod math;
pub mod palette;
pub mod xterm;

// Re-export primary types for convenience.
pub use ansi::ansi16;
pub use color::ThemeColor;
pub use cvd::CvdMode;
pub use ink::ink;
pub use mapping::{PaletteField, SemanticRole, ThemeMapping, TomlThemeMapping};
pub use math::ContrastViolation;
pub use palette::{
    MinimalTomlPalette, OwnedPalette, Palette, TomlPalette, all_builtin_palettes, catppuccin_latte,
    catppuccin_mocha, cvd_optimized, dracula, gruvbox_dark, gruvbox_light, kanagawa, nord,
    one_dark, rose_pine, solarized_dark, solarized_light, tokyo_night,
};
pub use xterm::xterm256;
