//! Embedded proportional fonts for the native interface.
//!
//! Monospace fonts are shared through [`Faces`](magistr_cockpit::fonts::Faces).

/// Hanken Grotesk regular, semibold, and italic faces, licensed under OFL 1.1.
/// See `fonts/HankenGrotesk-OFL.txt` for the license.
///
/// Embedded static fonts keep rendering independent of the host's installed fonts.
/// Weights 400 and 500 use regular; weight 600 uses semibold. Italic uses its own face.
pub struct UiFaces;

impl UiFaces {
    /// Hanken Grotesk Regular — weights 400 and 500 (`ui/xs` through `ui/md`).
    pub const REGULAR: &'static [u8] = include_bytes!("../fonts/HankenGrotesk-Regular.ttf");
    /// Hanken Grotesk SemiBold — weight 600 (`ui/lg` and `ui/xl`), and what a
    /// `BOLD` attribute resolves to on a proportional run.
    pub const SEMIBOLD: &'static [u8] = include_bytes!("../fonts/HankenGrotesk-SemiBold.ttf");
    /// Hanken Grotesk Italic.
    pub const ITALIC: &'static [u8] = include_bytes!("../fonts/HankenGrotesk-Italic.ttf");
}
