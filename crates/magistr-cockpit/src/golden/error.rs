//! How a golden comparison ends other than with a match.

use std::fmt;
use std::path::PathBuf;

/// Why [`Golden::check`](super::Golden::check) did not match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GoldenError {
    /// No golden existed: the frame was written as the new golden, which
    /// must be looked at and committed. Fails so the blessing is a
    /// deliberate act, never a side effect of a test run.
    Blessed {
        /// The golden just written.
        golden: PathBuf,
    },
    /// The frame differs from the golden. The frame was written beside it
    /// as `<name>.new.png` so both can be opened side by side; re-bless
    /// with `MAGISTR_GOLDEN_UPDATE=1` once the new image is the intended
    /// one.
    Mismatch {
        /// The checked-in image.
        golden: PathBuf,
        /// The image the frame rendered to.
        candidate: PathBuf,
        /// Pixels that differ (every pixel when the size changed).
        differing: usize,
        /// The frame's size differs from the golden's.
        size_changed: bool,
    },
    /// A golden or candidate could not be read or written.
    Io {
        /// The file involved.
        path: PathBuf,
        /// What the filesystem or codec said.
        error: String,
    },
}

impl fmt::Display for GoldenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blessed { golden } => write!(
                f,
                "golden was missing — wrote {}; inspect it, commit it and re-run",
                golden.display()
            ),
            Self::Mismatch {
                golden,
                candidate,
                differing,
                size_changed,
            } => {
                let what = if *size_changed {
                    "the frame's size changed".to_owned()
                } else {
                    format!("{differing} pixel(s) differ")
                };
                write!(
                    f,
                    "screenshot diverged from {golden}: {what} — compare against {candidate}, \
                     then re-bless with MAGISTR_GOLDEN_UPDATE=1 if the new image is right",
                    golden = golden.display(),
                    candidate = candidate.display()
                )
            }
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for GoldenError {}
