//! A terminal color: the terminal's default, one of the 256 indexed colors,
//! or 24-bit RGB.

/// A terminal foreground or background color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Color {
    /// The terminal's own default color (SGR 39 / 49).
    #[default]
    Default,
    /// One of the 256 indexed palette colors (SGR 38;5;n / 48;5;n).
    Indexed(u8),
    /// A 24-bit truecolor value (SGR 38;2;r;g;b / 48;2;r;g;b).
    Rgb(u8, u8, u8),
}
