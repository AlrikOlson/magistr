//! Dark or light, decided once: [`Theme::detect`] on the terminal's OSC 11
//! reply with `MAGISTR_THEME=light|dark` as the override, the palette
//! ([`ThemeMode::palette`], both from magistr-theme — Catppuccin Mocha and
//! Latte) and the [`Pen`] that turns it into the styles the painter uses.
//! No picker: the cockpit follows the terminal.
//!
//! Beside the palette the theme carries the three other choices the
//! screen is drawn from: the [`IconMode`] deciding which [`Icons`]
//! vocabulary the glyphs come from, the `CvdMode` the [`Pen`] remaps its
//! signal colours for, and the [`MotionMode`] deciding whether anything
//! moves at all.

pub mod decision;
pub mod icon_mode;
pub mod icons;
pub mod mode;
pub mod motion_mode;
pub mod pen;
pub mod severity;

pub use decision::Theme;
pub use icon_mode::IconMode;
pub use icons::Icons;
pub use mode::ThemeMode;
pub use motion_mode::MotionMode;
pub use pen::Pen;
pub use severity::Severity;
