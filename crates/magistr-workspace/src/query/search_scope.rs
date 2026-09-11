use globset::{GlobBuilder, GlobMatcher};

use super::{SearchInput, source_entry::SourceEntry};
use crate::contracts::{ErrorKind, WorkspaceError};

/// The retained path glob and include exclusions, applied before any worker call.
pub(crate) struct SearchScope {
    path: Option<GlobMatcher>,
    exclude_tests: bool,
    exclude_vendor: bool,
}

impl SearchScope {
    pub fn new(input: &SearchInput) -> Result<Self, WorkspaceError> {
        Ok(Self {
            path: input.path.as_deref().map(Self::glob).transpose()?,
            exclude_tests: input
                .include
                .as_ref()
                .is_some_and(|i| i.tests == Some(false)),
            exclude_vendor: input
                .include
                .as_ref()
                .is_some_and(|i| i.vendor == Some(false)),
        })
    }

    pub fn allows(&self, file: &SourceEntry) -> bool {
        if file.is_dir
            || self.exclude_tests && Self::is_test(&file.path)
            || self.exclude_vendor && file.junk
        {
            return false;
        }
        self.path
            .as_ref()
            .is_none_or(|glob| glob.is_match(&file.path))
    }

    pub fn glob(pattern: &str) -> Result<GlobMatcher, WorkspaceError> {
        GlobBuilder::new(pattern)
            .literal_separator(true)
            .backslash_escape(false)
            .build()
            .map(|glob| glob.compile_matcher())
            .map_err(|error| WorkspaceError::new(ErrorKind::PatternInvalid, error.to_string()))
    }

    pub fn is_glob(query: &str) -> bool {
        query.contains(['*', '?', '[', ']', '{', '}'])
    }

    pub fn is_test(path: &str) -> bool {
        let mut segments = path.split('/').peekable();
        while let Some(segment) = segments.next() {
            if segments.peek().is_some() {
                if matches!(
                    segment,
                    "test" | "tests" | "__tests__" | "spec" | "specs" | "benches" | "examples"
                ) {
                    return true;
                }
            } else if let Some((stem, extension)) = segment.rsplit_once('.') {
                return !extension.is_empty()
                    && (stem.ends_with(".test")
                        || stem.ends_with(".spec")
                        || stem.ends_with("_test"));
            }
        }
        false
    }
}

#[cfg(test)]
#[path = "search_scope.tests.rs"]
mod tests;
