//! Pixels or plain text: decided once at startup, never mid-session.

use super::text_reason::TextReason;

/// How the cockpit renders for this session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Pixel widgets render on this surface.
    Graphics {
        /// Whether the surface also honours the cockpit's pointer-shape
        /// request — the hand over a control, the default elsewhere. A
        /// kitty terminal does (its OSC 22); a sixel terminal would print
        /// the string and gets the hover colour alone; a window shapes
        /// its own cursor.
        pointer_shapes: bool,
    },
    /// Everything is cells; pixel regions do not render and a banner names
    /// a surface that would show them.
    MinimalText(TextReason),
}

impl Mode {
    /// The variable a screen-reader user sets to get the flat cockpit
    /// (`--screen-reader` exports it). Spelled here once: the terminal's
    /// environment reads it to decide [`TextReason::ScreenReader`], and
    /// the theme reads it too, because a flat session must never animate.
    pub const SCREEN_READER_ENV: &'static str = "MAGISTR_TUI_SCREEN_READER";

    /// Whether pixel widgets render.
    #[must_use]
    pub const fn has_pixels(self) -> bool {
        matches!(self, Self::Graphics { .. })
    }

    /// Whether the surface honours the pointer-shape request.
    #[must_use]
    pub const fn has_pointer_shapes(self) -> bool {
        matches!(
            self,
            Self::Graphics {
                pointer_shapes: true
            }
        )
    }

    /// The one-line banner for a plain-text session; `None` with pixels.
    #[must_use]
    pub const fn banner(self) -> Option<&'static str> {
        match self {
            Self::Graphics { .. } => None,
            Self::MinimalText(TextReason::Forced) => Some(
                "Plain-text mode (MAGISTR_TUI_MODE=text): pixel widgets are off. Unset it in ghostty, kitty, WezTerm or iTerm2 for the pixel cockpit.",
            ),
            Self::MinimalText(TextReason::ScreenReader) => Some(
                "Screen-reader mode: the cockpit is linear text — box drawing, spinners, motion and pixel widgets are off, and settled transcript lines are never redrawn.",
            ),
            Self::MinimalText(TextReason::TmuxNoPassthrough) => Some(
                "Plain-text mode: tmux is not passing graphics through. Set `allow-passthrough on` (tmux 3.3+) inside ghostty, kitty, WezTerm or iTerm2 for the pixel cockpit.",
            ),
            Self::MinimalText(TextReason::NoGraphics) => Some(
                "Plain-text mode: this terminal has no graphics protocol. Use ghostty, kitty, WezTerm or iTerm2 (3.5+) for the pixel cockpit.",
            ),
        }
    }
}

#[cfg(test)]
#[path = "mode.tests.rs"]
mod tests;
