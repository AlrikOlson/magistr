//! Why the cockpit is running without pixels.

/// The reason behind a [`super::Mode::MinimalText`] decision — it picks the
/// banner the user sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextReason {
    /// `MAGISTR_TUI_MODE=text` asked for it.
    Forced,
    /// A screen-reader session ([`Mode::SCREEN_READER_ENV`]): the
    /// cockpit is linear text (the terminal renderer's flat cockpit), so
    /// there is nothing to place pixels on.
    ///
    /// [`Mode::SCREEN_READER_ENV`]: super::Mode::SCREEN_READER_ENV
    ScreenReader,
    /// tmux is in the way and did not pass the graphics query through.
    TmuxNoPassthrough,
    /// The terminal answered neither the kitty query nor DA1 with sixel.
    NoGraphics,
}
