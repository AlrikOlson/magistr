//! Which kind of screen the cockpit is being drawn on.

/// The screen a cockpit frame is arranged for.
///
/// The cockpit has one [`view`](super::view::view), one `update` and one
/// widget vocabulary. What differs between a terminal and a desktop
/// window is not *what* is on screen but *where* it goes: a terminal is
/// 80 columns of cells read top to bottom, a window is a resizable
/// rectangle whose bottom edge is where a desktop user already looks for
/// a status bar.
///
/// So `view` reads this off the model and may arrange the **same widget
/// ids** differently. That is the whole contract, and the reason it is
/// worth stating: `update` folds the same messages, the hit registry
/// answers with the same ids, the accessibility tree keeps the same
/// names, and every terminal golden is untouched — because a terminal
/// model is [`Surface::Terminal`] and nothing about that arrangement
/// moved.
///
/// A new arrangement therefore costs a branch in `view` and nothing
/// else. Inventing a widget id for the native surface alone would cost a
/// hit-registry entry and an accessibility node with no label, which is
/// exactly the drift this type exists to avoid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Surface {
    /// A terminal: the cell grid, arranged as the cockpit always was.
    #[default]
    Terminal,
    /// A desktop window: the native GUI's own arrangement.
    Native,
}

impl Surface {
    /// Whether this is the desktop window.
    ///
    /// The question `view` asks. Named rather than matched at each site
    /// so a third surface — a web canvas, say — is a new variant and a
    /// new predicate, not a `match` arm to find in a 2400-line file.
    #[must_use]
    pub const fn is_native(self) -> bool {
        matches!(self, Self::Native)
    }
}

#[cfg(test)]
#[path = "surface.tests.rs"]
mod tests;
