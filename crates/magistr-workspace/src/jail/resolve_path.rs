use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};

use crate::contracts::{ErrorKind, WorkspaceError};

/// Adapted from Magistr's ACP target resolver without depending on a transport.
/// Existing links are followed before later components, including missing link
/// targets. Missing ordinary children remain valid future write destinations.
pub(super) fn resolve_path(path: &Path) -> Result<PathBuf, WorkspaceError> {
    if !path.is_absolute() || path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(denied("path must be absolute and contain no NUL byte"));
    }
    let mut pending = components(path);
    let mut resolved = PathBuf::new();
    let mut links = 0;
    while let Some(component) = pending.pop_front() {
        match component.as_os_str() {
            value if value == ".." => {
                resolved.pop();
            }
            value if value == "." => {}
            _ => {
                resolved.push(&component);
                // A prefix or root component cannot be a symlink, and probing a
                // bare Windows verbatim prefix (`\\?\C:`) fails with
                // ERROR_INVALID_FUNCTION rather than NotFound.
                if resolved.file_name().is_none() {
                    continue;
                }
                match std::fs::symlink_metadata(&resolved) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        links += 1;
                        if links > 40 {
                            return Err(denied("too many symbolic links while resolving path"));
                        }
                        let target = std::fs::read_link(&resolved).map_err(failed)?;
                        resolved.pop();
                        if target.is_absolute() {
                            resolved.clear();
                        }
                        let mut target = components(&target);
                        target.append(&mut pending);
                        pending = target;
                    }
                    Ok(_) => {}
                    // A name the filesystem cannot spell (Windows rejects `*` and
                    // `?`) is as absent as a missing child: nothing to follow.
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidFilename
                        ) => {}
                    Err(error) => return Err(failed(error)),
                }
            }
        }
    }
    Ok(resolved)
}

/// `candidate` under `root`: joined when relative, and when absolute restated
/// under the root's prefix spelling. Windows names one volume two ways, the
/// plain drive and the verbatim form `canonicalize` produces; a textual
/// prefix comparison against the root only works when both agree.
pub(super) fn join_under(root: &Path, candidate: &Path) -> PathBuf {
    if !candidate.is_absolute() {
        return root.join(candidate);
    }
    adopt_prefix_form(root, candidate)
}

#[cfg(windows)]
fn adopt_prefix_form(root: &Path, path: &Path) -> PathBuf {
    match (drive(root), drive(path)) {
        (Some((root_letter, root_prefix)), Some((letter, _))) if root_letter == letter => {
            let mut restated = PathBuf::from(root_prefix.as_os_str());
            restated.extend(path.components().skip(1));
            restated
        }
        _ => path.to_path_buf(),
    }
}

#[cfg(windows)]
fn drive(path: &Path) -> Option<(u8, std::path::PrefixComponent<'_>)> {
    use std::path::Prefix;
    match path.components().next()? {
        Component::Prefix(prefix) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                Some((letter.to_ascii_uppercase(), prefix))
            }
            _ => None,
        },
        _ => None,
    }
}

#[cfg(not(windows))]
fn adopt_prefix_form(_root: &Path, path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn components(path: &Path) -> VecDeque<PathBuf> {
    path.components()
        .map(|part| match part {
            Component::ParentDir => PathBuf::from(".."),
            Component::CurDir => PathBuf::from("."),
            part => PathBuf::from(part.as_os_str()),
        })
        .collect()
}

fn failed(error: std::io::Error) -> WorkspaceError {
    denied(&format!("cannot resolve path: {error}"))
}

fn denied(message: &str) -> WorkspaceError {
    WorkspaceError::new(ErrorKind::PathOutsideWorkspace, message)
}
