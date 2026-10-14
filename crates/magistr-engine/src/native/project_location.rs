//! Resolve repository sharing without copying configuration or live owner state.
use std::{
    io,
    path::{Path, PathBuf},
};

/// The canonical source directory and the project directory shared by linked worktrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLocation {
    pub working_copy: PathBuf,
    pub authority_root: PathBuf,
}

impl ProjectLocation {
    pub const ARCHIVE_REASON: &'static str = "Archived by Magistr";

    /// The physical Git working tree containing a project, or its own root outside Git.
    pub fn working_copy_root(root: &Path) -> io::Result<PathBuf> {
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "Choose a project directory.",
            ));
        }
        Ok(root
            .ancestors()
            .find(|path| path.join(".git").exists())
            .unwrap_or(&root)
            .to_path_buf())
    }

    pub fn is_archived(root: &Path) -> io::Result<bool> {
        let root = root.canonicalize()?;
        let Some(repository) = root.ancestors().find(|path| path.join(".git").exists()) else {
            return Ok(false);
        };
        let marker = repository.join(".git");
        if marker.is_dir() {
            return Ok(false);
        }
        let text = std::fs::read_to_string(marker)?;
        let gitdir = text
            .trim()
            .strip_prefix("gitdir: ")
            .ok_or_else(|| io::Error::other("Invalid Git link."))?;
        let lock = repository.join(gitdir).join("locked");
        match std::fs::read_to_string(lock) {
            Ok(reason) => Ok(reason.trim() == Self::ARCHIVE_REASON),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub fn require_open(root: &Path) -> io::Result<()> {
        if Self::is_archived(root)? {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Reopen this archived working copy before starting work.",
            ));
        }
        Ok(())
    }

    /// Resolve Git's on-disk links and verify the backlink of a linked working tree.
    pub fn resolve(root: &Path) -> io::Result<Self> {
        let working_copy = root.canonicalize()?;
        if !working_copy.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "Choose a project directory.",
            ));
        }
        let repository = working_copy
            .ancestors()
            .find(|path| path.join(".git").exists());
        let Some(repository) = repository else {
            return Ok(Self {
                authority_root: working_copy.clone(),
                working_copy,
            });
        };
        let marker = repository.join(".git");
        let authority_root = if marker.is_dir() {
            working_copy.clone()
        } else {
            let text = std::fs::read_to_string(&marker)?;
            let gitdir = text.trim().strip_prefix("gitdir: ").ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "The Git working-copy link is invalid.",
                )
            })?;
            let gitdir = repository.join(gitdir).canonicalize()?;
            let common_file = gitdir.join("commondir");
            if !common_file.is_file() {
                // A separate Git directory is still one working copy.
                working_copy.clone()
            } else {
                let common = std::fs::read_to_string(common_file)?;
                let common = gitdir.join(common.trim()).canonicalize()?;
                let backlink = std::fs::read_to_string(gitdir.join("gitdir"))?;
                if Path::new(backlink.trim()).canonicalize()? != marker.canonicalize()? {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "This directory is not the registered Git working copy.",
                    ));
                }
                if common.file_name().is_none_or(|name| name != ".git") {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "The main Git working copy could not be located.",
                    ));
                }
                let main = common
                    .parent()
                    .ok_or_else(|| io::Error::other("Missing main working copy."))?;
                main.join(
                    working_copy
                        .strip_prefix(repository)
                        .map_err(io::Error::other)?,
                )
                .canonicalize()?
            }
        };
        Ok(Self {
            working_copy,
            authority_root,
        })
    }
}

#[cfg(test)]
#[path = "project_location.tests.rs"]
mod tests;
