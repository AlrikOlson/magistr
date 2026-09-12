use std::{
    fs::{self, Metadata},
    io::Read,
    path::Path,
};

use sha2::{Digest, Sha256};

use super::{EntryKind, ManifestEntry, SelectionPolicy, SnapshotError, SourceManifest};

/// The bytes classified as text or binary by the index's own rule: a NUL
/// within the first 8 KiB makes the file binary.
const CLASSIFY_BYTES: usize = 8192;

/// Observe every selected path under `root` (canonical, absolute). Reads are
/// unlocked and per file: a file whose metadata differs after its bytes were
/// hashed is refused rather than recorded, and a selection past the policy's
/// ceiling refuses the whole manifest.
pub(super) fn capture(
    root: &Path,
    policy: &SelectionPolicy,
) -> Result<SourceManifest, SnapshotError> {
    policy.validate()?;
    let mut manifest = SourceManifest::default();
    for item in walker(root).build() {
        let entry = item.map_err(|error| SnapshotError::Io {
            path: root.display().to_string(),
            detail: error.to_string(),
        })?;
        let Some(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() || entry.path() == root {
            continue;
        }
        let relative = relative(root, entry.path())?;
        if manifest.len() == policy.max_entries {
            return Err(SnapshotError::SelectionTruncated {
                limit: policy.max_entries,
            });
        }
        if let Some(observed) = observe(root, entry.path(), &relative)? {
            manifest.entries.insert(relative, observed);
        }
    }
    for input in policy.explicit_inputs() {
        let absolute = root.join(input);
        let observed = match fs::symlink_metadata(&absolute) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => ManifestEntry::missing(),
            Err(error) => return Err(io(input, error)),
            Ok(metadata) if metadata.is_dir() => {
                return Err(SnapshotError::InvalidInput {
                    path: input.to_owned(),
                    reason: "a directory is not an explicit input; name its files".into(),
                });
            }
            Ok(_) => observe(root, &absolute, input)?.unwrap_or_else(ManifestEntry::missing),
        };
        if manifest.len() == policy.max_entries && !manifest.entries.contains_key(input) {
            return Err(SnapshotError::SelectionTruncated {
                limit: policy.max_entries,
            });
        }
        manifest.entries.insert(input.to_owned(), observed);
    }
    Ok(manifest)
}

/// The workspace's selection: hidden files in, ignore rules honored without
/// requiring a repository, links not followed, owner and repository state
/// never entered.
fn walker(root: &Path) -> ignore::WalkBuilder {
    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .hidden(false)
        .follow_links(false)
        .require_git(false)
        .filter_entry(|entry| !matches!(entry.file_name().to_str(), Some(".git" | ".workbench")));
    builder
}

fn relative(root: &Path, path: &Path) -> Result<String, SnapshotError> {
    let relative = path.strip_prefix(root).map_err(|_| SnapshotError::Io {
        path: path.display().to_string(),
        detail: "walked outside the workspace root".into(),
    })?;
    relative
        .to_str()
        .map(|text| text.replace('\\', "/"))
        .ok_or_else(|| SnapshotError::NonUtf8Path(path.to_path_buf()))
}

/// One path's record; `None` for a special file (socket, fifo, device) that
/// carries no source bytes.
fn observe(
    root: &Path,
    absolute: &Path,
    relative: &str,
) -> Result<Option<ManifestEntry>, SnapshotError> {
    let metadata = fs::symlink_metadata(absolute).map_err(|error| io(relative, error))?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        let target = fs::read_link(absolute).map_err(|error| io(relative, error))?;
        let bytes = target.as_os_str().as_encoded_bytes();
        return Ok(Some(ManifestEntry {
            kind: EntryKind::Symlink,
            mode: mode(&metadata),
            size: bytes.len() as u64,
            digest: format!("{:x}", Sha256::digest(bytes)),
        }));
    }
    if !file_type.is_file() {
        return Ok(None);
    }
    let mut file =
        crate::source_open::open_source(root, absolute).map_err(|error| SnapshotError::Io {
            path: relative.to_owned(),
            detail: error.message,
        })?;
    let before = file.metadata().map_err(|error| io(relative, error))?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut hashed: u64 = 0;
    let mut binary = false;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| io(relative, error))?;
        if read == 0 {
            break;
        }
        let head = &buffer[..read];
        if (hashed as usize) < CLASSIFY_BYTES {
            let end = (CLASSIFY_BYTES - hashed as usize).min(read);
            binary |= head[..end].contains(&0);
        }
        digest.update(head);
        hashed += read as u64;
    }
    let after = file.metadata().map_err(|error| io(relative, error))?;
    if !same(&before, &after) || hashed != before.len() {
        return Err(SnapshotError::ChangedDuringCapture(relative.to_owned()));
    }
    Ok(Some(ManifestEntry {
        kind: if binary {
            EntryKind::Binary
        } else {
            EntryKind::Text
        },
        mode: mode(&before),
        size: before.len(),
        digest: format!("{:x}", digest.finalize()),
    }))
}

fn same(before: &Metadata, after: &Metadata) -> bool {
    before.len() == after.len()
        && before.modified().ok() == after.modified().ok()
        && mode(before) == mode(after)
}

pub(super) fn mode(metadata: &Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.mode() & 0o7777
    }
    #[cfg(not(unix))]
    {
        if metadata.permissions().readonly() {
            0o444
        } else {
            0o644
        }
    }
}

fn io(path: &str, error: std::io::Error) -> SnapshotError {
    SnapshotError::Io {
        path: path.to_owned(),
        detail: error.to_string(),
    }
}
