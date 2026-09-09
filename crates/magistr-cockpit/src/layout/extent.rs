//! The size of a node's content, scrolled or not.

/// Columns and rows a node's content covers — at least the node's own
/// size, more when children overflow a viewport. The scrollable range is
/// `content - rect` on each axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Extent {
    /// Columns the content covers.
    pub width: u16,
    /// Rows the content covers.
    pub height: u16,
}
