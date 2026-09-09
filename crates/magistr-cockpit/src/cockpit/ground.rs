//! Where a new project stands.

/// The ground a project is born on (Birth F1): a folder that already
/// exists — dropped from the Finder, or typed — a repository to clone
/// into a folder, or nothing yet, a folder to create. The directory is
/// the entry point's to read and write; the cockpit only carries its
/// name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ground {
    /// An existing folder, by path.
    Folder(String),
    /// A repository, by URL, cloned into the folder at the path.
    Clone {
        /// The repository's URL.
        url: String,
        /// The folder the clone lands in.
        into: String,
    },
    /// Nothing yet: the folder at the path is created empty.
    Empty(String),
}

impl Ground {
    /// The folder the project lives in, whichever way it got there.
    #[must_use]
    pub fn dir(&self) -> &str {
        match self {
            Self::Folder(dir) | Self::Empty(dir) => dir,
            Self::Clone { into, .. } => into,
        }
    }

    /// The project's name before the survey names it: the folder's
    /// last segment.
    #[must_use]
    pub fn name(&self) -> &str {
        let dir = self.dir().trim_end_matches('/');
        dir.rsplit('/').next().unwrap_or(dir)
    }

    /// The folder a clone of `url` lands in beside `dir`'s parent —
    /// `~/Code/tidewatch.git` clones into `~/Code/tidewatch`.
    #[must_use]
    pub fn clone_into(url: &str, beside: &str) -> String {
        let repo = url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(url)
            .trim_end_matches(".git");
        let parent = beside.trim_end_matches('/');
        if parent.is_empty() {
            repo.to_owned()
        } else {
            format!("{parent}/{repo}")
        }
    }
}

#[cfg(test)]
#[path = "ground.tests.rs"]
mod tests;
