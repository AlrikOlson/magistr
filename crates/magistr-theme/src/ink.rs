//! The colour a *consumer* should paint a palette slot with.
//!
//! A [`Palette`] is a colour identity, not a set of guaranteed
//! foregrounds — [`Palette::validate`] says so out loud, and most
//! built-ins fail some of the floors it declares (Catppuccin Latte
//! misses five of them). The cockpit is immune because every `Pen` runs
//! through [`math::ensure_contrast`] before it paints. This module is
//! the same guarantee for the other consumer: the GUI, which reads the
//! palette as CSS custom properties and cannot correct per surface the
//! way a pen can.
//!
//! The split is between *grounds* and *ink*. A ground — `base` and the
//! three surfaces — is emitted exactly as the palette names it, because
//! a nudged background is a different theme rather than a more legible
//! one. Everything else is ink, and owes the floor `Palette::validate`
//! declares for it on the palette's own base.

use crate::color::ThemeColor;
use crate::mapping::PaletteField;
use crate::math;
use crate::palette::Palette;

/// What a palette slot owes the reader.
enum Contract {
    /// A background: the GUI paints behind things with it, so it is
    /// emitted unchanged.
    Ground,
    /// A foreground owing `ratio` against the palette's base — and,
    /// when `on_panels`, against `surface0` as well.
    Ink { ratio: f64, on_panels: bool },
}

/// WCAG AA for normal text. The floor `Palette::validate` puts on its
/// text pairs.
const TEXT: f64 = 4.5;

/// WCAG AA for large text and UI elements. The floor
/// `Palette::validate` puts on `overlay0`, and the one
/// `Palette::accent_violations` sweeps every accent at.
const UI: f64 = 3.0;

/// The colour a consumer should paint `field` with on this palette:
/// the palette's own colour for a ground, and the slot nudged toward
/// legibility for ink.
///
/// A slot that already clears its floor is returned untouched, so a
/// palette that passes its own validator — Catppuccin Mocha does — comes
/// back byte-identical.
///
/// The ground is assumed to be the palette's `base`. That is the best a
/// stylesheet can do: unlike a pen, it does not know which surface a
/// given rule will be drawn over. Text is additionally corrected against
/// `surface0`, the second ground `Palette::validate` names for it.
#[must_use]
pub fn ink(field: PaletteField, palette: &Palette) -> ThemeColor {
    let raw = field.resolve(palette);
    match contract(field) {
        Contract::Ground => raw,
        Contract::Ink { ratio, on_panels } => {
            let c = math::ensure_contrast(raw, palette.base, ratio);
            if on_panels {
                math::ensure_contrast(c, palette.surface0, ratio)
            } else {
                c
            }
        }
    }
}

/// What each slot owes, taken from `Palette::validate`'s own
/// declarations so there is one floor and not two.
///
/// `overlay1` is the one extension: the validator names no pair for it,
/// and it gets `overlay0`'s floor by symmetry — both are dim slots the
/// GUI draws separators and secondary text with.
fn contract(field: PaletteField) -> Contract {
    match field {
        // Grounds.
        PaletteField::Base
        | PaletteField::Surface0
        | PaletteField::Surface1
        | PaletteField::Surface2 => Contract::Ground,
        // Text — validate() checks these on base *and* on surface0.
        PaletteField::Text | PaletteField::Subtext0 | PaletteField::Subtext1 => Contract::Ink {
            ratio: TEXT,
            on_panels: true,
        },
        // The four signal accents validate() checks as text on base.
        PaletteField::Blue | PaletteField::Green | PaletteField::Red | PaletteField::Yellow => {
            Contract::Ink {
                ratio: TEXT,
                on_panels: false,
            }
        }
        // Dim slots and decorative accents: the large-text floor.
        PaletteField::Overlay0
        | PaletteField::Overlay1
        | PaletteField::Sky
        | PaletteField::Peach
        | PaletteField::Mauve
        | PaletteField::Lavender => Contract::Ink {
            ratio: UI,
            on_panels: false,
        },
    }
}

#[cfg(test)]
#[path = "ink.tests.rs"]
mod tests;
