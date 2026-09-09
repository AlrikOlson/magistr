//! One key report.

use super::key_code::KeyCode;
use super::key_kind::KeyKind;
use super::modifiers::Modifiers;

/// A key with the modifiers held and the event type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    /// Which key.
    pub code: KeyCode,
    /// Modifier keys held.
    pub mods: Modifiers,
    /// Press, repeat or release.
    pub kind: KeyKind,
}

impl KeyEvent {
    /// A press of `code` with no modifiers.
    #[must_use]
    pub const fn new(code: KeyCode) -> Self {
        Self {
            code,
            mods: Modifiers::NONE,
            kind: KeyKind::Press,
        }
    }

    /// The same key with `mods` held.
    #[must_use]
    pub const fn with(mut self, mods: Modifiers) -> Self {
        self.mods = mods;
        self
    }

    /// The same key as a `kind` event.
    #[must_use]
    pub const fn kind(mut self, kind: KeyKind) -> Self {
        self.kind = kind;
        self
    }

    /// Whether this is a press (what a binding normally answers).
    #[must_use]
    pub fn is_press(self) -> bool {
        self.kind == KeyKind::Press
    }

    /// Whether this is a release — what a held key's binding answers
    /// (the kitty keyboard protocol's event types on a terminal, the
    /// window's own key-up).
    #[must_use]
    pub fn is_release(self) -> bool {
        self.kind == KeyKind::Release
    }
}
