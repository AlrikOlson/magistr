use std::path::Path;

use crate::error::{MagistrError, Result};

/// Validate shared configuration before any scaffold writes.
pub(crate) struct ExistingConfig;

impl ExistingConfig {
    pub(crate) fn validate(dir: &Path) -> Result<()> {
        let namespace = crate::paths::magistr_dir(dir);
        match std::fs::symlink_metadata(&namespace) {
            Ok(meta) if !meta.is_dir() => {
                return Err(MagistrError::Config(format!(
                    "{} must be a directory, not a symlink or file; initialization left it untouched",
                    namespace.display()
                )));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(MagistrError::io(&namespace, e)),
        }
        Self::read_json(&crate::paths::project_json(dir))?;
        if let Some(doc) = Self::read_json(&dir.join(".mcp.json"))?
            && let Some(servers) = doc.get("mcpServers")
            && !servers.is_object()
        {
            return Err(MagistrError::Config(
                ".mcp.json: mcpServers must be a JSON object; fix it before initialization".into(),
            ));
        }
        Ok(())
    }

    fn read_json(path: &Path) -> Result<Option<serde_json::Value>> {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if !meta.is_file() => {
                return Err(MagistrError::Config(format!(
                    "{} must be a regular JSON file; initialization left it untouched",
                    path.display()
                )));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(MagistrError::io(path, e)),
        }
        let content = std::fs::read_to_string(path).map_err(|e| MagistrError::io(path, e))?;
        let doc: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            MagistrError::Config(format!(
                "{}: {e}; fix the JSON before initialization",
                path.display()
            ))
        })?;
        if !doc.is_object() {
            return Err(MagistrError::Config(format!(
                "{} must contain a JSON object; fix it before initialization",
                path.display()
            )));
        }
        Ok(Some(doc))
    }
}
