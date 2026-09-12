use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};

use magistr_application::effects::EffectError;

/// Resolve each existing component before processing its successors. This
/// preserves symlink/.. semantics and resolves symlinked ancestors of new files.
/// This is a policy target resolver, not an atomic filesystem sandbox.
pub fn resolve_target(root: &Path, path: &Path) -> Result<PathBuf, EffectError> {
    let joined = if path.is_absolute() {
        adopt_prefix_form(root, path)
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
                // A prefix or root component cannot be a symlink, and probing a
                // bare Windows verbatim prefix (`\\?\C:`) fails with
                // ERROR_INVALID_FUNCTION rather than NotFound.
                if resolved.file_name().is_none() {
                    continue;
                }
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

/// Windows spells one volume two ways: `C:\` and the verbatim `\\?\C:\`
/// that `canonicalize` produces. Prefix comparison is textual, so an absolute
/// request spelled differently from the root would never `starts_with` it.
/// Restate the request under the root's spelling when both name the same drive.
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

fn failed(error: std::io::Error) -> EffectError {
    EffectError::Failed(error.to_string())
}
