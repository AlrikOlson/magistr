use super::{NativeConfig, NativeReader, owner_lease::OwnerLease};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
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
            // Discover records only after fencing the previous writer,
            // including an owner that has not saved its first record yet.
            for relative in records(&source, id)? {
                let old = source.join(&relative);
                let new = checked_path(config.data_dir(), &relative)?;
                lease.lock(&old.with_extension("json.lock"), "older saved record")?;
                lease.lock(&new.with_extension("json.lock"), "saved record upgrade")?;
                let bytes = read_regular(&old)?
                    .ok_or_else(|| io::Error::other("Saved work moved during upgrade; retry."))?;
                validate(&relative, &bytes, id)?;
                if let Some(current) = read_regular(&new)?
                    && current != bytes
                {
                    return Err(io::Error::other(format!(
                        "Saved work differs between {} and {}. Both copies are intact; reconcile this record before retrying the upgrade.",
                        old.display(),
                        new.display()
                    )));
                }
                imported.push((relative, bytes));
            }
            // The family owners validate the selected project using their
            // actual readers before anything is published to the new store.
            let reader =
                NativeReader::open_for_project(&config.clone().with_data_dir(&source), id.into());
            reader.roadmap_chunks().map_err(io::Error::other)?;
            reader.think_sessions().map_err(io::Error::other)?;
            reader.ship_snapshot().map_err(io::Error::other)?;
            reader.signal_list().map_err(io::Error::other)?;
        }
        for (relative, bytes) in &imported {
            let path = config.data_dir().join(relative);
            if read_regular(&path)?.is_none() {
                publish(&path, bytes, false)?;
            }
            request
                .files
                .push((relative.clone(), format!("{:x}", Sha256::digest(bytes))));
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

fn validate(path: &Path, bytes: &[u8], id: &str) -> io::Result<()> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(io::Error::other)?;
    if path.components().any(|part| part.as_os_str() == "receipts") {
        return Ok(());
    }
    if path.starts_with("ship") {
        return magistr_workflow::ship::persistence::Persistence::validate_document(bytes, id)
            .map_err(io::Error::other);
    }
    if path.starts_with("think") && (value["schema_version"] != 2 || value["project_id"] != id) {
        return Err(io::Error::other(format!(
            "{} uses an older reasoning format this upgrade cannot safely convert; original preserved.",
            path.display()
        )));
    }
    Ok(())
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
    File::open(parent)?.sync_all()
}

#[cfg(test)]
#[path = "workflow_upgrade.tests.rs"]
mod tests;
