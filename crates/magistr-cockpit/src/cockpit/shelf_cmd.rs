//! What the cockpit asks for a shipped chunk's deliverables.

use super::chapter::Chapter;

/// A shelf request from the cockpit, carried by
/// [`Cmd::Shelf`](super::Cmd::Shelf): the entry point that owns the
/// project directory serves it as a job — reading the store, git and the
/// session database off the loop thread — and posts the answer back as
/// [`JobOutput::Shelf`](crate::job::JobOutput::Shelf). The cockpit never
/// gathers a shelf itself: it knows no git and opens no database.
///
/// The two writing requests carry the [`Chapter`]: the cockpit is the
/// one that holds the roadmap the chapter is derived from, and the
/// document the page publishes and the bundle carries has to be the one
/// the Report view composed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ShelfCmd {
    /// A Done chunk was opened on a window: collect its deliverables.
    Collect {
        /// The shipped chunk.
        chunk: String,
    },
    /// \[Export bundle\]: write the chapter and its receipts as files.
    Export {
        /// The shipped chunk.
        chunk: String,
        /// The chapter the bundle carries.
        chapter: Chapter,
    },
    /// \[Share page\]: write the chapter as one self-contained page.
    Share {
        /// The shipped chunk.
        chunk: String,
        /// The chapter the page publishes.
        chapter: Chapter,
    },
}
