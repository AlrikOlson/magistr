use super::IndexError;
use std::{collections::BTreeSet, path::Path};
use tokio_util::sync::CancellationToken;
/// The existing native ignore walker, collected with a hard admission ceiling.
pub(super) struct Selection {
    pub paths: BTreeSet<String>,
    pub truncated: bool,
}
impl Selection {
    /// Recheck the candidate's inherited ignore files without enumerating the
    /// workspace. A watcher can be disabled or its event can still be pending.
    pub fn allows(root: &Path, path: &str) -> bool {
        let relative = Path::new(path);
        if relative.as_os_str().is_empty()
            || relative.components().any(|part| {
                !matches!(part, std::path::Component::Normal(_))
                    || matches!(part.as_os_str().to_str(), Some(".git" | ".workbench"))
            })
        {
            return false;
        }
        // Rebuild for each validation so edits to inherited/global rules are
        // observed. The incremental API shares the walker's exact precedence
        // and nested-directory bases without enumerating the source tree.
        let Some(mut matcher) = Self::walker(root).build_matchers().pop() else {
            return false;
        };
        let (matched, error) = matcher.matched_with_errors(relative, false);
        error.is_none() && !matched.is_ignore()
    }

    pub fn scan(
        root: &Path,
        state_dir: &Path,
        max_files: usize,
        cancel: &CancellationToken,
    ) -> Result<Self, IndexError> {
        let state_dir = state_dir.to_path_buf();
        let walker = Self::walker(root)
            .filter_entry(move |entry| {
                let name = entry.file_name().to_string_lossy();
                !(entry.path().starts_with(&state_dir)
                    || entry.file_type().is_some_and(|t| t.is_dir())
                        && matches!(name.as_ref(), ".git" | ".workbench"))
            })
            .build();
        let mut paths = BTreeSet::new();
        let mut truncated = false;
        for item in walker {
            if cancel.is_cancelled() {
                return Err(IndexError::Unavailable("index stopped".into()));
            }
            let entry = item.map_err(|e| IndexError::Unavailable(e.to_string()))?;
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            if paths.len() == max_files {
                truncated = true;
                break;
            }
            let path = entry
                .path()
                .strip_prefix(root)
                .map_err(|e| IndexError::Unavailable(e.to_string()))?;
            let Some(path) = path.to_str() else {
                return Err(IndexError::Unavailable(
                    "non-UTF8 path is not indexable".into(),
                ));
            };
            paths.insert(path.replace('\\', "/"));
        }
        Ok(Self { paths, truncated })
    }

    fn walker(root: &Path) -> ignore::WalkBuilder {
        let mut builder = ignore::WalkBuilder::new(root);
        builder.hidden(false).follow_links(false).require_git(false);
        builder
    }
}

#[cfg(test)]
#[path = "selection.tests.rs"]
mod tests;
