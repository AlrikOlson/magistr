use super::{NativeConfig, owner_lease::OwnerLease};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// One-time relocation of a project's recognized workflow records. Configuration
/// setup requests it before pinning the older identity; admission finishes it.
pub struct WorkflowUpgrade;

#[derive(Serialize, Deserialize)]
struct Request {
    version: u32,
    project_id: String,
    complete: bool,
    #[serde(default)]
    files: Vec<(PathBuf, String)>,
}

impl WorkflowUpgrade {
    pub const REQUEST: &'static str = ".magistr/upgrades/workflow.json";

    pub fn request(project_id: &str) -> String {
        serde_json::to_string_pretty(&Request {
            version: 1,
            project_id: project_id.into(),
            complete: false,
            files: vec![],
        })
        .expect("request")
    }

    pub fn has_records(config: &NativeConfig, id: &str) -> io::Result<bool> {
        Ok(!records(config.data_dir(), id)?.is_empty())
    }

    pub(super) fn ready(
        config: &NativeConfig,
        id: &str,
    ) -> Result<(), magistr_workflow_contracts::ReadError> {
        use magistr_workflow_contracts::{ReadError, ReadErrorKind};
        if !config.persist || older_root(config.data_dir()).is_none() {
            return Ok(());
        }
        let Some(bytes) = read_regular(&config.workflow_root().join(Self::REQUEST))
            .map_err(|e| ReadError::new(ReadErrorKind::Io, e.to_string()))?
        else {
            return Ok(());
        };
        let request: Request = serde_json::from_slice(&bytes)
            .map_err(|e| ReadError::new(ReadErrorKind::Decode, e.to_string()))?;
        if request.project_id == id && !request.complete {
            return Err(ReadError::new(
                ReadErrorKind::Interrupted,
                "The saved-work upgrade is unfinished. Start Magistr or run magistr repair to finish it.",
            ));
        }
        Ok(())
    }

    pub(super) fn apply(config: &NativeConfig, id: &str, lease: &mut OwnerLease) -> io::Result<()> {
        if !config.persist {
            return Ok(());
        }
        let request_path = config.workflow_root().join(Self::REQUEST);
        let Some(before) = read_regular(&request_path)? else {
            return Ok(());
        };
        let mut request: Request = serde_json::from_slice(&before).map_err(io::Error::other)?;
        if request.version != 1 || request.project_id != id {
            return Err(io::Error::other(
                "The saved upgrade belongs to a different project or format. Restore its project settings before retrying.",
            ));
        }
        if request.complete {
            return Ok(());
        }
        if older_root(config.data_dir()).is_none() && !Self::has_records(config, id)? {
            return Err(io::Error::other(
                "The previous workflow store could not be located for this custom storage path. Restore its saved records to the configured store and retry; originals are untouched.",
            ));
        }
        let mut imported = Vec::new();
        if let Some(source) = older_root(config.data_dir())
            && source.try_exists()?
        {
            for name in ["project-owner.lock", "mcp-owner.lock"] {
                let relative = Path::new("roadmap/sessions").join(format!("{id}.{name}"));
                lease.lock(&checked_path(&source, &relative)?, "older workflow history")?;
            }
            // Fence every source before conversion. The originals remain recovery
            // evidence; only current documents are published to the new store.
            for relative in records(&source, id)? {
                let old = checked_path(&source, &relative)?;
                lease.lock(&old.with_extension("json.lock"), "older saved record")?;
                let bytes = read_regular(&old)?
                    .ok_or_else(|| io::Error::other("Saved work moved during upgrade; retry."))?;
                imported.push((relative, bytes));
            }
        }
        // The family codecs validate converted documents. In particular the
        // strict current readers must never decode old bytes before conversion.
        let imported = converted_records(config, id, imported)?;
        for (relative, bytes) in &imported {
            let path = checked_path(config.data_dir(), relative)?;
            lease.lock(&path.with_extension("json.lock"), "saved record upgrade")?;
            if let Some(current) = read_regular(&path)?
                && current != *bytes
            {
                return Err(io::Error::other(format!(
                    "Saved work differs at {}. Both copies are intact; reconcile this record before retrying the upgrade.",
                    path.display()
                )));
            }
        }
        // Validate every destination before the first publish. Deterministic
        // conversion makes a partial copy byte-identical on the next admission.
        request.files.clear();
        for (relative, bytes) in &imported {
            let path = config.data_dir().join(relative);
            if read_regular(&path)?.is_none() {
                publish(&path, bytes, false)?;
            }
            request
                .files
                .push((relative.clone(), hex::encode(Sha256::digest(bytes))));
        }
        // A crash before this atomic receipt leaves a pending request. Retrying
        // accepts only byte-identical prior copies and never imports twice.
        if read_regular(&request_path)?.as_deref() != Some(&before) {
            return Err(io::Error::other("The upgrade request changed; retry."));
        }
        request.complete = true;
        publish(
            &request_path,
            &serde_json::to_vec_pretty(&request).map_err(io::Error::other)?,
            true,
        )
    }
}

fn older_root(current: &Path) -> Option<PathBuf> {
    let magistr = current.parent()?;
    (current.file_name()? == "workflow" && magistr.file_name()? == "magistr")
        .then(|| magistr.parent().map(|base| base.join("think-and-ship")))
        .flatten()
}

fn records(root: &Path, id: &str) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for family in ["think", "ship", "roadmap", "signal"] {
        visit(root, Path::new(family), id, &mut found)?;
    }
    found.sort();
    Ok(found)
}

fn visit(root: &Path, relative: &Path, id: &str, found: &mut Vec<PathBuf>) -> io::Result<()> {
    let path = root.join(relative);
    if fs::symlink_metadata(&path).is_ok_and(|m| m.is_symlink()) {
        return Err(io::Error::other(format!(
            "{} must not be a symbolic link during upgrade",
            path.display()
        )));
    }
    let entries = match fs::read_dir(&path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let child = relative.join(entry.file_name());
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let selected = name == format!("{id}.json")
            || name.starts_with(&format!("{id}__"))
            || relative.components().any(|part| part.as_os_str() == id);
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            if selected {
                return Err(io::Error::other(
                    "Saved work must not be a symbolic link during upgrade.",
                ));
            }
        } else if kind.is_dir() {
            visit(root, &child, id, found)?;
        } else if kind.is_file() && selected && child.extension().is_some_and(|ext| ext == "json") {
            found.push(child);
        }
    }
    Ok(())
}

fn converted_records(
    config: &NativeConfig,
    id: &str,
    records: Vec<(PathBuf, Vec<u8>)>,
) -> io::Result<Vec<(PathBuf, Vec<u8>)>> {
    use magistr_workflow::{roadmap::RoadmapEngine, ship, signal::SignalEngine, think};
    use std::collections::BTreeMap;
    let execution = super::ExecutionContext::new(&config.workflow_root(), "default")?;
    let execution_root = Path::new("ship/executions").join(execution.key());
    let mut converted = BTreeMap::new();
    let mut histories = BTreeMap::new();
    for (relative, bytes) in records {
        serde_json::from_slice::<serde_json::Value>(&bytes)
            .map_err(|error| io::Error::other(format!("{}: {error}", relative.display())))?;
        if relative.parent() == Some(Path::new("think/sessions")) {
            let session = relative
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| io::Error::other("The saved reasoning filename is invalid."))?;
            histories.insert(session.to_owned(), bytes);
            continue;
        }
        let mut target = relative.clone();
        let bytes = if relative.starts_with("ship") {
            if let Ok(receipt) = relative.strip_prefix("ship/receipts") {
                target = execution_root.join("receipts").join(receipt);
                bytes
            } else if relative
                .components()
                .any(|part| part.as_os_str() == "receipts")
            {
                bytes
            } else {
                if relative.parent() == Some(Path::new("ship/sessions")) {
                    target = execution_root.join("sessions").join(format!("{id}.json"));
                }
                ship::persistence::Persistence::upgrade_document(&bytes, id)
                    .map_err(|error| io::Error::other(format!("{}: {error}", relative.display())))?
            }
        } else if relative == Path::new("roadmap/sessions").join(format!("{id}.json")) {
            RoadmapEngine::upgrade_document(&bytes, id).map_err(io::Error::other)?
        } else if relative == Path::new("signal/sessions").join(format!("{id}.json")) {
            SignalEngine::upgrade_document(&bytes, id).map_err(io::Error::other)?
        } else {
            bytes
        };
        if let Some(existing) = converted.insert(target, bytes.clone())
            && existing != bytes
        {
            return Err(io::Error::other(
                "Two different saved work records have the same destination; originals are unchanged.",
            ));
        }
    }
    if !histories.is_empty() {
        let owner = think::persistence::Persistence::for_project(&config.persistence_config(), id);
        let bytes = owner
            .upgrade_documents(&histories)
            .map_err(io::Error::other)?;
        converted.insert(
            Path::new("think/sessions").join(format!("{id}.json")),
            bytes,
        );
    }
    Ok(converted.into_iter().collect())
}

fn read_regular(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(meta) if !meta.is_file() || meta.is_symlink() => Err(io::Error::other(format!(
            "{} must be a regular file during upgrade",
            path.display()
        ))),
        Ok(meta) if meta.len() > 128 * 1024 * 1024 => Err(io::Error::other(
            "Saved record exceeds the upgrade size limit; original preserved.",
        )),
        Ok(_) => fs::read(path).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn checked_path(root: &Path, relative: &Path) -> io::Result<PathBuf> {
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part);
        if fs::symlink_metadata(&path).is_ok_and(|m| m.is_symlink()) {
            return Err(io::Error::other(format!(
                "{} must not be a symbolic link during upgrade",
                path.display()
            )));
        }
    }
    Ok(path)
}

fn publish(path: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
    let parent = path.parent().expect("workflow directory");
    fs::create_dir_all(parent)?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    if replace {
        staged.persist(path).map_err(|e| e.error)?;
    } else {
        staged.persist_noclobber(path).map_err(|e| e.error)?;
    }
    sync_directory(parent)
}

/// Make a new directory entry durable. Windows cannot open a directory as a
/// file and commits the entry with the rename itself.
fn sync_directory(directory: &Path) -> io::Result<()> {
    #[cfg(unix)]
    fs::File::open(directory)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

#[cfg(test)]
#[path = "workflow_upgrade.tests.rs"]
mod tests;
