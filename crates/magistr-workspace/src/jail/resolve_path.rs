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
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(failed(error)),
                }
            }
        }
    }
    Ok(resolved)
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
