use super::{FileDisposition, FilePreview};

/// One ordered receipt, distinguishing approved content from its observed effect.
#[derive(Clone, Debug)]
pub struct FileResult {
    pub preview: FilePreview,
    pub disposition: FileDisposition,
}
