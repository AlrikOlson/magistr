//! Press, repeat or release.

/// The event type of a key report. Legacy encodings only ever report a
/// press; the kitty protocol (with event types enabled) reports all three.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeyKind {
    /// The key went down.
    #[default]
    Press,
    /// The key is held and auto-repeating.
    Repeat,
    /// The key came up.
    Release,
}
