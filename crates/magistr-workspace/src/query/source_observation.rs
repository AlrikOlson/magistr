use crate::{contracts::Freshness, file_content::FileContent};

/// One bounded live observation; its hash identifies the bytes actually read.
#[derive(Debug)]
pub struct SourceObservation {
    pub path: String,
    pub content: FileContent,
    pub content_hash: String,
    pub freshness: Freshness,
}
