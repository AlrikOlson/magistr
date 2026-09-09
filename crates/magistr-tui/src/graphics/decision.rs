//! The probe's verdict: which protocol pixels travel by, or why they do
//! not — decided once at startup, never mid-session.

use magistr_cockpit::capability::{Mode, TextReason};

use super::env::Env;
use super::probe_reply::ProbeReply;
use super::protocol::Protocol;

/// What the startup probe decided for this terminal. The runtime keeps
/// the [`Protocol`] for its compositor and hands the core only the
/// [`Mode`] it translates to — `Protocol`, `Passthrough` and `Kitty`
/// never cross into magistr-cockpit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Decision {
    /// Pixel widgets are placed through `Protocol`.
    Graphics(Protocol),
    /// Everything is cells; the reason picks the banner the user sees.
    MinimalText(TextReason),
}

impl Decision {
    /// The decision: a
    /// screen-reader session is linear text and wins over everything;
    /// then `MAGISTR_TUI_MODE=text`; a kitty
    /// reply means kitty (wrapped for tmux when tmux is in the way); tmux
    /// without a reply means its passthrough is off, so plain text; sixel
    /// only outside tmux; otherwise plain text.
    #[must_use]
    pub fn for_terminal(env: &Env, reply: &ProbeReply) -> Self {
        if env.screen_reader() {
            Self::MinimalText(TextReason::ScreenReader)
        } else if env.forces_text() {
            Self::MinimalText(TextReason::Forced)
        } else if reply.kitty {
            Self::Graphics(Protocol::Kitty(env.passthrough()))
        } else if env.tmux() {
            Self::MinimalText(TextReason::TmuxNoPassthrough)
        } else if reply.sixel {
            Self::Graphics(Protocol::Sixel)
        } else {
            Self::MinimalText(TextReason::NoGraphics)
        }
    }

    /// The capability the core is told. A terminal that answered the
    /// kitty graphics query speaks the pointer-shape protocol too, and
    /// every other terminal — sixel included — would print the OSC 22
    /// string, so only kitty gets the hand.
    #[must_use]
    pub const fn mode(self) -> Mode {
        match self {
            Self::Graphics(Protocol::Kitty(_)) => Mode::Graphics {
                pointer_shapes: true,
            },
            Self::Graphics(Protocol::Sixel) => Mode::Graphics {
                pointer_shapes: false,
            },
            Self::MinimalText(reason) => Mode::MinimalText(reason),
        }
    }
}

#[cfg(test)]
#[path = "decision.tests.rs"]
mod tests;
