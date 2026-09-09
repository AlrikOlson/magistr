//! Headless renderer trait.

use std::io::Write;

use super::event::HeadlessEvent;

/// Trait for rendering headless events to an output stream.
///
/// Each implementation formats events differently for its target audience:
/// JSON for machines, plain text for humans, annotations for GitHub Actions.
pub trait HeadlessRenderer {
    /// Emit a single event to the writer.
    fn emit(&self, w: &mut dyn Write, event: HeadlessEvent) -> std::io::Result<()>;
}
