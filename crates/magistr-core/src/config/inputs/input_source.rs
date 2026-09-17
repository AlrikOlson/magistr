/// Where a captured setting came from. Names the source, never the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    /// Nothing in the captured environment named it; the built-in default
    /// (or "unlocated") applies.
    Default,
    /// The named environment variable, read once at capture.
    Environment(&'static str),
    /// An explicit host or builder override applied after capture.
    Explicit,
}
