//! Modifier keys held during a key or mouse event.

use std::ops::BitOr;

/// A set of modifier keys: shift, alt, ctrl, super.
///
/// The kitty keyboard protocol also reports hyper, meta and the lock keys;
/// those are dropped on decode so a key binding never depends on
/// caps-lock state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers(u8);

impl Modifiers {
    /// No modifier.
    pub const NONE: Self = Self(0);
    /// Shift.
    pub const SHIFT: Self = Self(1);
    /// Alt (option, meta on legacy terminals).
    pub const ALT: Self = Self(2);
    /// Control.
    pub const CTRL: Self = Self(4);
    /// Super (command, windows).
    pub const SUPER: Self = Self(8);

    /// The set encoded by the kitty modifier field (`1 + bits`; hyper,
    /// meta and lock bits are dropped). `0` and `1` both mean none.
    #[must_use]
    pub const fn from_kitty(field: u32) -> Self {
        let bits = field.saturating_sub(1) & 0x0f;
        // Masked to four bits above, so the narrowing is exact.
        Self(bits as u8)
    }

    /// The set encoded in an SGR mouse button value (shift 4, alt 8,
    /// ctrl 16).
    #[must_use]
    pub const fn from_sgr(button: u32) -> Self {
        let mut bits = 0;
        if button & 4 != 0 {
            bits |= Self::SHIFT.0;
        }
        if button & 8 != 0 {
            bits |= Self::ALT.0;
        }
        if button & 16 != 0 {
            bits |= Self::CTRL.0;
        }
        Self(bits)
    }

    /// Both sets held at once — `BitOr` in a `const` — so a chord
    /// with two modifiers can be a registry constant.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether every modifier in `other` is held.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether no modifier is held.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The raw bits (shift 1, alt 2, ctrl 4, super 8).
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

impl BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

#[cfg(test)]
#[path = "modifiers.tests.rs"]
mod tests;
