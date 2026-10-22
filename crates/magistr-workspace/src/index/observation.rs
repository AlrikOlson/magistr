use super::{IndexError, cache::IndexedFile};
use crate::{jail::JailPolicy, source_languages::SourceLanguages};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path, time::UNIX_EPOCH};

/// One bounded optimistic read. Changed metadata invalidates it.
pub(super) struct Observation {
    pub file: IndexedFile,
}
impl Observation {
    pub fn read(
        root: &Path,
        path: &str,
        max_bytes: usize,
        languages: &SourceLanguages,
    ) -> Result<Self, IndexError> {
        let jail = JailPolicy::new(root).map_err(|e| IndexError::Unavailable(format!("{e:?}")))?;
        let absolute = jail
            .check_read(path)
            .map_err(|e| IndexError::Unavailable(format!("{e:?}")))?;
        let spelling = root.join(path);
        if std::fs::symlink_metadata(&spelling)?
            .file_type()
            .is_symlink()
            || absolute != spelling
        {
            return Err(IndexError::Unavailable(
                "index excludes symlink inputs".into(),
            ));
        }
        let mut input = crate::source_open::open_source(root, &absolute)
            .map_err(|error| IndexError::Unavailable(error.to_string()))?;
        let before = input.metadata()?;
        if !before.is_file() {
            return Err(IndexError::Unavailable(
                "index input is no longer a file".into(),
            ));
        }
        let mut bytes = Vec::new();
        if before.len() <= max_bytes as u64 {
            input
                .by_ref()
                .take(max_bytes as u64 + 1)
                .read_to_end(&mut bytes)?;
        }
        let after = input.metadata()?;
        if before.len() != after.len()
            || before.modified().ok() != after.modified().ok()
            || mode(&before) != mode(&after)
        {
            return Err(IndexError::Unavailable(
                "file changed during index read".into(),
            ));
        }
        let oversized = before.len() > max_bytes as u64 || bytes.len() > max_bytes;
        let hash = if oversized {
            String::new()
        } else {
            hex::encode(Sha256::digest(&bytes))
        };
        let content = if oversized || bytes.iter().take(8192).any(|b| *b == 0) {
            None
        } else {
            String::from_utf8(bytes).ok()
        };
        Ok(Self {
            file: IndexedFile {
                path: path.into(),
                language: languages.language(path),
                size: before.len(),
                mtime_ms: before
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
                    .unwrap_or(0),
                mode: mode(&before),
                content_hash: hash,
                junk: crate::indexer::is_junk(path),
                content,
                symbols: Vec::new(),
                imports: Vec::new(),
                test_ranges: Vec::new(),
            },
        })
    }
    pub fn same(&self, other: &IndexedFile) -> bool {
        !self.file.content_hash.is_empty()
            && self.file.content_hash == other.content_hash
            && self.file.mtime_ms == other.mtime_ms
            && self.file.mode == other.mode
            && self.file.size == other.size
            && self.file.language == other.language
    }
}
fn mode(meta: &std::fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        meta.mode()
    }
    #[cfg(not(unix))]
    {
        u32::from(meta.permissions().readonly())
    }
}
