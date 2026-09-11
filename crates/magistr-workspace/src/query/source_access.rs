use super::{WorkspaceQueries, source_entry::SourceEntry, source_snapshot::SourceSnapshot};
use crate::{
    contracts::{ErrorKind, Freshness, WorkspaceError},
    file_content::FileContent,
    source_open::open_source,
};
use std::{
    fs::{self, Metadata},
    io::Read,
};

const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;

impl WorkspaceQueries<'_> {
    pub(super) fn snapshot(&self, path: &str) -> Result<SourceSnapshot, WorkspaceError> {
        let requested = path.replace('\\', "/");
        let admitted = self.policy.check_read(&requested)?;
        let relative = admitted
            .strip_prefix(&self.root)
            .map_err(|_| {
                WorkspaceError::new(
                    ErrorKind::PathOutsideWorkspace,
                    "source path escaped workspace",
                )
            })?
            .to_str()
            .ok_or_else(|| {
                WorkspaceError::new(ErrorKind::PatternInvalid, "source path is not UTF-8")
            })?
            .replace('\\', "/");
        for attempt in 0..2 {
            let before = fs::metadata(&admitted).map_err(io_error)?;
            if !before.is_file() {
                return Err(WorkspaceError::new(
                    ErrorKind::FileNotFound,
                    "source is not a regular file",
                ));
            }
            let file = open_source(&self.root, &admitted)?;
            let opened = file.metadata().map_err(io_error)?;
            if !opened.is_file() {
                return Err(WorkspaceError::new(
                    ErrorKind::FileNotFound,
                    "source is not a regular file",
                ));
            }
            let mut bytes = Vec::with_capacity((opened.len() as usize).min(MAX_FILE_BYTES));
            file.take((MAX_FILE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(io_error)?;
            if bytes.len() > MAX_FILE_BYTES {
                return Err(WorkspaceError::new(
                    ErrorKind::BudgetTooSmall,
                    "source exceeds the 1 MiB live-read ceiling; narrow to a smaller file",
                ));
            }
            let after = fs::metadata(&admitted).map_err(io_error)?;
            let admitted_again = self.policy.check_read(&requested)?;
            let stable =
                admitted == admitted_again && same(&before, &opened) && same(&opened, &after);
            if !stable && attempt == 0 {
                continue;
            }
            let content = FileContent::decode(bytes);
            let content_hash = content.content_hash();
            return Ok(SourceSnapshot {
                path: relative,
                content,
                content_hash,
                freshness: if stable {
                    Freshness::Warm
                } else {
                    Freshness::Cold
                },
            });
        }
        Err(WorkspaceError::new(
            ErrorKind::FileNotFound,
            "source changed during observation",
        ))
    }

    pub(crate) fn files(&self) -> Result<Vec<SourceEntry>, WorkspaceError> {
        let mut builder = ignore::WalkBuilder::new(&self.root);
        builder
            .hidden(false)
            .follow_links(false)
            .require_git(false)
            .filter_entry(|entry| {
                !(entry.file_type().is_some_and(|kind| kind.is_dir())
                    && matches!(entry.file_name().to_str(), Some(".git" | ".workbench")))
            });
        let mut rows = Vec::new();
        for entry in builder.build() {
            let entry = entry
                .map_err(|error| WorkspaceError::new(ErrorKind::FileNotFound, error.to_string()))?;
            let Some(kind) = entry.file_type() else {
                continue;
            };
            if !(kind.is_file() || kind.is_dir()) || entry.path() == self.root {
                continue;
            }
            if rows.len() >= MAX_ENTRIES {
                return Err(WorkspaceError::new(
                    ErrorKind::BudgetTooSmall,
                    "source walk exceeds the 100000-entry ceiling",
                ));
            }
            self.policy.check_read(entry.path())?;
            let path = entry
                .path()
                .strip_prefix(&self.root)
                .map_err(|_| {
                    WorkspaceError::new(
                        ErrorKind::PathOutsideWorkspace,
                        "walk entry escaped workspace",
                    )
                })?
                .to_str()
                .ok_or_else(|| {
                    WorkspaceError::new(ErrorKind::PatternInvalid, "source path is not UTF-8")
                })?
                .replace('\\', "/");
            rows.push(SourceEntry {
                language: crate::source_language::language(&path).map(str::to_owned),
                junk: crate::indexer::is_junk(&path),
                path,
                is_dir: kind.is_dir(),
            });
        }
        rows.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(rows)
    }
}

fn same(left: &Metadata, right: &Metadata) -> bool {
    let basic = left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.permissions() == right.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        basic
            && left.dev() == right.dev()
            && left.ino() == right.ino()
            && left.ctime() == right.ctime()
            && left.ctime_nsec() == right.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        basic
    }
}

fn io_error(error: std::io::Error) -> WorkspaceError {
    WorkspaceError::new(ErrorKind::FileNotFound, error.to_string())
}

#[cfg(test)]
#[path = "source_access.tests.rs"]
mod tests;
