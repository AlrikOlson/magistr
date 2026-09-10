use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};

use magistr_application::effects::EffectError;

/// Resolve each existing component before processing its successors. This
/// preserves symlink/.. semantics and resolves symlinked ancestors of new files.
/// This is a policy target resolver, not an atomic filesystem sandbox.
pub fn resolve_target(root: &Path, path: &Path) -> Result<PathBuf, EffectError> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    if !joined.is_absolute() {
        return Err(EffectError::InvalidRequest(
            "effect root must be absolute".into(),
        ));
    }
    let mut pending = components(&joined);
    let mut resolved = PathBuf::new();
    let mut links = 0;
    while let Some(component) = pending.pop_front() {
        match component.as_os_str() {
            value if value == std::ffi::OsStr::new("..") => {
                resolved.pop();
            }
            value if value == std::ffi::OsStr::new(".") => {}
            _ => {
                resolved.push(&component);
                match std::fs::symlink_metadata(&resolved) {
                    Ok(meta) if meta.file_type().is_symlink() => {
                        links += 1;
                        if links > 40 {
                            return Err(EffectError::InvalidRequest(
                                "too many symbolic links".into(),
                            ));
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

fn failed(error: std::io::Error) -> EffectError {
    EffectError::Failed(error.to_string())
}
