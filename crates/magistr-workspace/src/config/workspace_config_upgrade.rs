use std::{
    io::{self, Write},
    path::Path,
};
use toml_edit::{DocumentMut, Item, TableLike};

/// Converts the retired project-local workspace configuration once. Preview is
/// read-only; apply is called only under the workspace owner's admission.
pub struct WorkspaceConfigUpgrade;

impl WorkspaceConfigUpgrade {
    pub fn preview(root: &Path) -> io::Result<Option<String>> {
        let Some(old) = read(root, "workbench.toml")? else {
            return Ok(None);
        };
        let current = read(root, ".magistr/workspace.toml")?;
        Ok(Some(merge(&old, current.as_deref())?))
    }

    pub(crate) fn apply(root: &Path) -> io::Result<()> {
        let Some(old) = read(root, "workbench.toml")? else {
            return Ok(());
        };
        let current = read(root, ".magistr/workspace.toml")?;
        let after = merge(&old, current.as_deref())?;
        // Preserve the retired original before changing the active file. A
        // content-addressed archive also makes a crash before unlink retryable.
        use sha2::{Digest, Sha256};
        let archive = format!(
            ".magistr/upgrades/workspace-config/{}.toml",
            hex::encode(Sha256::digest(old.as_bytes()))
        );
        match read(root, &archive)? {
            Some(saved) if saved != old => {
                return Err(io::Error::other(
                    "The workspace configuration backup changed; restore it before retrying.",
                ));
            }
            Some(_) => {}
            None => publish(root, &archive, &old, None)?,
        }
        if current.as_deref() != Some(&after) {
            publish(root, ".magistr/workspace.toml", &after, current.as_deref())?;
        }
        if read(root, "workbench.toml")?.as_deref() != Some(&old) {
            return Err(io::Error::other(
                "workbench.toml changed during its upgrade; retry to preserve the new settings.",
            ));
        }
        std::fs::remove_file(root.join("workbench.toml"))?;
        std::fs::File::open(root)?.sync_all()
    }
}

fn parse(content: &str) -> io::Result<DocumentMut> {
    let doc = content.parse::<DocumentMut>().map_err(io::Error::other)?;
    if let Some(version) = doc.get("config_version")
        && version.as_integer() != Some(1)
    {
        return Err(io::Error::other(
            "Cannot upgrade workspace settings with an unsupported config_version; original left unchanged.",
        ));
    }
    Ok(doc)
}

fn merge(old: &str, current: Option<&str>) -> io::Result<String> {
    let old_doc = parse(old)?;
    let Some(current) = current else {
        return Ok(old.to_owned());
    };
    let mut doc = parse(current)?;
    merge_table(old_doc.as_table(), doc.as_table_mut(), "")?;
    Ok(doc.to_string())
}

fn merge_table(old: &dyn TableLike, current: &mut dyn TableLike, prefix: &str) -> io::Result<()> {
    for (key, value) in old.iter() {
        let path = if prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{prefix}.{key}")
        };
        match current.get_mut(key) {
            None => {
                current.insert(key, value.clone());
            }
            Some(found) => {
                if let (Some(a), Some(b)) = (value.as_table_like(), found.as_table_like_mut()) {
                    merge_table(a, b, &path)?;
                } else if semantic(value) != semantic(found) {
                    return Err(io::Error::other(format!(
                        "Workspace settings disagree at {path} between workbench.toml and .magistr/workspace.toml. Make that setting agree and retry; neither value has been replaced."
                    )));
                }
            }
        }
    }
    Ok(())
}

fn semantic(item: &Item) -> Option<toml::Value> {
    format!("value = {item}")
        .parse::<toml::Table>()
        .ok()?
        .remove("value")
}

fn read(root: &Path, relative: &str) -> io::Result<Option<String>> {
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        path.push(component);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_symlink() => {
                return Err(io::Error::other(format!(
                    "{} must not be a symlink during upgrade",
                    path.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
    }
    std::fs::read_to_string(path).map(Some)
}

fn publish(root: &Path, relative: &str, content: &str, expected: Option<&str>) -> io::Result<()> {
    if read(root, relative)?.as_deref() != expected {
        return Err(io::Error::other(format!(
            "{relative} changed during upgrade; retry."
        )));
    }
    let path = root.join(relative);
    let parent = path.parent().expect("configuration directory");
    std::fs::create_dir_all(parent)?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(content.as_bytes())?;
    if expected.is_some() {
        staged
            .as_file()
            .set_permissions(std::fs::metadata(&path)?.permissions())?;
    }
    staged.as_file().sync_all()?;
    if read(root, relative)?.as_deref() != expected {
        return Err(io::Error::other(format!(
            "{relative} changed during upgrade; retry."
        )));
    }
    if expected.is_some() {
        staged.persist(&path).map_err(|e| e.error)?;
    } else {
        staged.persist_noclobber(&path).map_err(|e| e.error)?;
    }
    std::fs::File::open(parent)?.sync_all()
}

#[cfg(test)]
#[path = "workspace_config_upgrade.tests.rs"]
mod tests;
