//! Text attributes as a small bit set.

use std::ops::{BitOr, BitOrAssign};

use super::underline::Underline;

/// The six one-bit flags live in bits 0..=5.
const FLAGS: u16 = 0b0011_1111;
/// The underline style's field starts here.
const STYLE_SHIFT: u32 = 6;
/// Three bits, wide enough for every [`Underline`] code (1..=5).
const STYLE_MASK: u16 = 0b111 << STYLE_SHIFT;

/// A set of SGR text attributes.
///
/// Combine with `|`: `Attrs::BOLD | Attrs::UNDERLINE`.
///
/// Sixteen bits: the six one-bit flags (SGR 1/2/3/4/7/9) in the low bits
/// and a three-bit underline-style field above them. Every styled
/// underline constant — [`Attrs::UNDERLINE_CURLY`] and its three
/// siblings — carries the plain [`Attrs::UNDERLINE`] bit *as well as*
/// its style code, which is what makes the fallback for a terminal that
/// predates `SGR 4:x` structural rather than a decision at every call
/// site: such a terminal is sent the bit and never the style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Attrs(u16);

impl Attrs {
    /// No attributes.
    pub const NONE: Attrs = Attrs(0);
    /// SGR 1.
    pub const BOLD: Attrs = Attrs(1);
    /// SGR 2.
    pub const DIM: Attrs = Attrs(1 << 1);
    /// SGR 3.
    pub const ITALIC: Attrs = Attrs(1 << 2);
    /// SGR 4 — the plain rule.
    pub const UNDERLINE: Attrs = Attrs(1 << 3);
    /// SGR 7.
    pub const REVERSE: Attrs = Attrs(1 << 4);
    /// SGR 9.
    pub const STRIKETHROUGH: Attrs = Attrs(1 << 5);
    /// SGR 4:2 — underlined twice.
    pub const UNDERLINE_DOUBLE: Attrs = Attrs::styled(Underline::Double);
    /// SGR 4:3 — the wave an editor marks a diagnostic with.
    pub const UNDERLINE_CURLY: Attrs = Attrs::styled(Underline::Curly);
    /// SGR 4:4 — a dotted rule.
    pub const UNDERLINE_DOTTED: Attrs = Attrs::styled(Underline::Dotted);
    /// SGR 4:5 — a dashed rule.
    pub const UNDERLINE_DASHED: Attrs = Attrs::styled(Underline::Dashed);

    /// The underline bit plus `style`'s code in the style field.
    const fn styled(style: Underline) -> Attrs {
        Attrs(Self::UNDERLINE.0 | ((style.code() as u16) << STYLE_SHIFT))
    }

    /// True when every attribute in `other` is set in `self`.
    #[must_use]
    pub const fn contains(self, other: Attrs) -> bool {
        self.0 & other.0 == other.0
    }

    /// True when no attribute is set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// These attributes with every flag of `other` cleared. Clearing
    /// the underline bit takes its style with it, since a style with
    /// no rule is not a thing a cell can wear.
    #[must_use]
    pub const fn without(self, other: Attrs) -> Attrs {
        let cleared = self.0 & !(other.0 & FLAGS);
        if other.0 & Self::UNDERLINE.0 != 0 {
            Attrs(cleared & !STYLE_MASK)
        } else {
            Attrs(cleared)
        }
    }

    /// The underline this cell wears, when it wears one. A set
    /// [`Attrs::UNDERLINE`] bit with an empty style field is
    /// [`Underline::Single`], so the plain underline every call site
    /// already asked for keeps meaning exactly what it did.
    #[must_use]
    pub const fn underline(self) -> Option<Underline> {
        if !self.contains(Attrs::UNDERLINE) {
            return None;
        }
        match Underline::from_code(((self.0 & STYLE_MASK) >> STYLE_SHIFT) as u8) {
            Some(style) => Some(style),
            None => Some(Underline::Single),
        }
    }
}

impl BitOr for Attrs {
    type Output = Attrs;

    /// The flags unioned; the underline style taken from `rhs` when it
    /// names one, so laying a style on replaces the style already there
    /// rather than or-ing two codes into a third.
    fn bitor(self, rhs: Attrs) -> Attrs {
        let style = if rhs.0 & STYLE_MASK == 0 {
            self.0 & STYLE_MASK
        } else {
            rhs.0 & STYLE_MASK
        };
        Attrs(((self.0 | rhs.0) & FLAGS) | style)
    }
}

impl BitOrAssign for Attrs {
    fn bitor_assign(&mut self, rhs: Attrs) {
        *self = *self | rhs;
    }
}

#[cfg(test)]
#[path = "attrs.tests.rs"]
mod tests;
