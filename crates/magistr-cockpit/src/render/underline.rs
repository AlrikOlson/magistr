//! The shape an underline is drawn in — SGR 4's colon-parameterized styles.

/// How an underlined cell's rule is drawn.
///
/// The values are kitty's `SGR 4:x` sub-parameters, which VTE, WezTerm,
/// foot, Ghostty and Alacritty all adopted: `4:1` straight, `4:2`
/// double, `4:3` curly, `4:4` dotted, `4:5` dashed. There is deliberately
/// no "none" variant — an unstyled cell simply does not carry
/// [`Attrs::UNDERLINE`](super::Attrs::UNDERLINE), so
/// [`Attrs::underline`](super::Attrs::underline) answers `Option` and
/// the absent case cannot be spelled two ways.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Underline {
    /// One rule under the baseline (SGR 4, or `4:1`).
    #[default]
    Single,
    /// Two rules (`4:2`).
    Double,
    /// A wave — how an editor marks a diagnostic (`4:3`).
    Curly,
    /// A dotted rule (`4:4`).
    Dotted,
    /// A dashed rule (`4:5`).
    Dashed,
}

impl Underline {
    /// Every style, in code order — the sweep the round-trip and the
    /// rasterizer tests walk.
    pub const ALL: [Underline; 5] = [
        Underline::Single,
        Underline::Double,
        Underline::Curly,
        Underline::Dotted,
        Underline::Dashed,
    ];

    /// The style's `SGR 4:x` sub-parameter.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Underline::Single => 1,
            Underline::Double => 2,
            Underline::Curly => 3,
            Underline::Dotted => 4,
            Underline::Dashed => 5,
        }
    }

    /// The style a `code` names, when it names one.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Underline> {
        match code {
            1 => Some(Underline::Single),
            2 => Some(Underline::Double),
            3 => Some(Underline::Curly),
            4 => Some(Underline::Dotted),
            5 => Some(Underline::Dashed),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "underline.tests.rs"]
mod tests;
