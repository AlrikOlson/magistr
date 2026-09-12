/// What to start. The variant is the consumer's admission decision: a
/// configured gate line and a caller's check command are shell lines, a
/// workspace declared command is the argv its policy admitted. The
/// mechanism never converts one into the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// One shell line, run through the platform shell (`sh -c` on Unix,
    /// `cmd /C` on Windows) so `&&`, pipes and quoting work as written.
    Shell(String),
    /// One program and its literal arguments, no shell anywhere.
    Argv(Vec<String>),
}
