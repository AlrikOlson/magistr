use std::path::Path;

use crate::error::{MagistrError, MapMagistrErr, Result};

use super::emitters;
use super::manifest::ScaffoldManifest;
use super::owned_removal::OwnedRemoval;
use super::{
    GITIGNORE_SECTION_END, GITIGNORE_SECTION_START, MAGISTR_MCP_SERVERS, is_generated_mcp_entry,
};

/// Remove unchanged, recorded scaffold files from shared agent directories.
///
/// A known filename is not ownership evidence: customized and unrecorded files
/// survive, as do non-regular entries and paths traversing symlinks. Only empty
/// parents of removed files are pruned. CLAUDE.md and ROADMAP.md remain user-owned.
///
/// Explicit reset removes the .magistr namespace and generated MCP/gitignore
/// entries. Initialization never calls it.
pub fn remove_scaffold(dir: &Path) -> Result<()> {
    // Capture ownership once, before removing the namespace holding the manifest.
    let manifest = ScaffoldManifest::load(dir);
    if let Some(owned) = &manifest.codex {
        for change in super::CodexSetup::shared_removal_changes(dir, owned)? {
            if change.action == super::FileAction::Overwrite {
                let path = dir.join(&change.path);
                std::fs::write(&path, &change.new_content)
                    .map_err(|e| MagistrError::io(&path, e))?;
            }
        }
    }
    let mut removal = OwnedRemoval::new(dir, &manifest);
    let codex_paths = super::CodexSetup::removal_paths(&manifest);
    for path in manifest.files.keys() {
        // Include recorded files retired by a newer binary, but never let an
        // arbitrary manifest entry claim a root project document or source file.
        if Path::new(path).starts_with(".claude") || codex_paths.contains(path) {
            removal.remove(path)?;
        }
    }
    emitters::remove_emitter_files(&mut removal)?;
    for path in ["run-autonomous.sh", "format-stream.sh"] {
        removal.remove(path)?;
    }
    removal.prune();

    let magistr_dir = crate::paths::magistr_dir(dir);
    if magistr_dir.exists() {
        std::fs::remove_dir_all(&magistr_dir).map_err(|e| MagistrError::io(&magistr_dir, e))?;
    }
    remove_magistr_mcp_entries(dir)?;
    remove_magistr_gitignore_entries(dir)?;
    Ok(())
}

/// Remove only magistr-managed MCP server entries from `.mcp.json`.
fn remove_magistr_mcp_entries(dir: &Path) -> Result<()> {
    let mcp_path = dir.join(".mcp.json");
    if !mcp_path.exists() {
        return Ok(());
    }

    let content = match std::fs::read_to_string(&mcp_path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let mut doc: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    let modified = if let Some(obj) = doc.as_object_mut() {
        if let Some(servers) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
            let before = servers.len();
            for key in MAGISTR_MCP_SERVERS {
                // The name alone does not establish ownership. Use the same
                // launch-line check as setup and integration removal.
                if servers
                    .get(*key)
                    .is_some_and(|entry| is_generated_mcp_entry(dir, key, entry))
                {
                    servers.remove(*key);
                }
            }
            servers.len() != before
        } else {
            false
        }
    } else {
        false
    };

    if !modified {
        return Ok(());
    }

    let should_delete = doc
        .as_object()
        .map(|obj| {
            let servers_empty = obj
                .get("mcpServers")
                .and_then(|v| v.as_object())
                .is_some_and(|m| m.is_empty());
            servers_empty && obj.len() == 1
        })
        .unwrap_or(false);

    if should_delete {
        let _ = std::fs::remove_file(&mcp_path);
    } else {
        let pretty = format!(
            "{}\n",
            serde_json::to_string_pretty(&doc).with_context(MagistrError::Config)?
        );
        std::fs::write(&mcp_path, pretty.as_bytes()).map_err(|e| MagistrError::io(&mcp_path, e))?;
    }

    Ok(())
}

fn remove_magistr_gitignore_entries(dir: &Path) -> Result<()> {
    let gi_path = dir.join(".gitignore");
    if !gi_path.exists() {
        return Ok(());
    }

    let content = match std::fs::read_to_string(&gi_path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let has_start = content.contains(GITIGNORE_SECTION_START);
    let has_end = content.contains(GITIGNORE_SECTION_END);

    if !has_start || !has_end {
        return Ok(());
    }

    let start_pos = content.find(GITIGNORE_SECTION_START).unwrap();
    let end_pos = content.find(GITIGNORE_SECTION_END).unwrap();
    let end_line_end = content[end_pos..]
        .find('\n')
        .map(|i| end_pos + i + 1)
        .unwrap_or(content.len());

    let before = &content[..start_pos];
    let after = &content[end_line_end..];
    let result = format!("{before}{after}");
    let trimmed = result.trim();

    if trimmed.is_empty() {
        let _ = std::fs::remove_file(&gi_path);
    } else {
        std::fs::write(&gi_path, result.as_bytes()).map_err(|e| MagistrError::io(&gi_path, e))?;
    }

    Ok(())
}

#[cfg(test)]
#[path = "remove_scaffold.tests.rs"]
mod tests;
