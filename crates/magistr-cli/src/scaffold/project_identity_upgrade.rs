use super::{ChangePlan, PlannedChange};
use crate::error::{MagistrError, Result};
use magistr_engine::native::{NativeConfig, WorkflowUpgrade};
use std::path::Path;

/// Adoption of an existing declared identity belongs to setup, alongside the
/// project settings writer. The workflow owner executes the relocation request.
pub(super) struct ProjectIdentityUpgrade;

impl ProjectIdentityUpgrade {
    pub(super) fn include(root: &Path, plan: &mut ChangePlan) -> Result<()> {
        let shared = super::target_root::target_root(root, ".magistr/project.json")?;
        let read = |path| {
            super::read_target(&shared, path)
                .map_err(|e| MagistrError::Config(format!("{path}: {e}")))
        };
        let Some(legacy) = read(".think-and-ship/project.json")? else {
            return Ok(());
        };
        let before = read(".magistr/project.json")?;
        let planned = plan
            .files
            .iter()
            .find(|c| c.path == ".magistr/project.json");
        let Some(current) = planned
            .map(|c| c.new_content.clone())
            .or_else(|| before.clone())
        else {
            return Ok(());
        };
        let old: serde_json::Value = serde_json::from_str(&legacy)?;
        let Some(id) = old["id"].as_str().filter(|id| {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        }) else {
            return Err(MagistrError::Config("The older project identity is invalid; restore .think-and-ship/project.json before upgrading.".into()));
        };
        let original: serde_json::Value = serde_json::from_str(before.as_deref().unwrap_or("{}"))?;
        let declared = original["project"]["id"].as_str();
        if declared.is_some_and(|current| current != id)
            || (declared.is_some() && original["roadmap_backend"] != "think-and-ship")
        {
            return Ok(());
        }
        let config = NativeConfig::from_env(&shared);
        let derived = magistr_engine::native::project_id_for_path(&shared);
        if declared.is_none()
            && derived != id
            && WorkflowUpgrade::has_records(&config, &derived)
                .map_err(|e| MagistrError::io(&shared, e))?
        {
            return Err(MagistrError::Config(
                "This project has saved work under two identities. Keep both copies and reconcile the project identity before retrying; no identity was changed.".into()
            ));
        }
        let mut document: serde_json::Value = serde_json::from_str(&current)?;
        document["project"]["id"] = id.into();
        let change = PlannedChange::merged(
            ".magistr/project.json",
            before,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        );
        if let Some(existing) = plan.files.iter_mut().find(|c| c.path == change.path) {
            *existing = change;
        } else {
            plan.files.push(change);
        }
        if read(WorkflowUpgrade::REQUEST)?.is_none()
            && !plan
                .files
                .iter()
                .any(|change| change.path == WorkflowUpgrade::REQUEST)
        {
            plan.files.insert(
                0,
                PlannedChange::merged(WorkflowUpgrade::REQUEST, None, WorkflowUpgrade::request(id)),
            );
        }
        Ok(())
    }
    pub(super) fn precondition(root: &Path, change: &PlannedChange) -> Result<()> {
        if change.path != ".magistr/project.json" {
            return Ok(());
        }
        let before: serde_json::Value =
            serde_json::from_str(change.existing_content.as_deref().unwrap_or("{}"))?;
        let after: serde_json::Value = serde_json::from_str(&change.new_content)?;
        if before["project"]["id"] == after["project"]["id"] {
            return Ok(());
        }
        let shared = super::target_root::target_root(root, &change.path)?;
        let Some(legacy) = super::read_target(&shared, ".think-and-ship/project.json")
            .map_err(|e| MagistrError::Config(e.to_string()))?
        else {
            return Ok(());
        };
        let legacy: serde_json::Value = serde_json::from_str(&legacy)?;
        if legacy["id"] != after["project"]["id"] {
            return Ok(());
        }
        let request = super::read_target(&shared, WorkflowUpgrade::REQUEST)
            .map_err(|e| MagistrError::Config(e.to_string()))?;
        let request: serde_json::Value = serde_json::from_str(request.as_deref().unwrap_or("{}"))?;
        if request["version"] != 1 || request["project_id"] != after["project"]["id"] {
            return Err(MagistrError::Config("The saved-work upgrade must be prepared before changing project identity. Run magistr repair again.".into()));
        }
        Ok(())
    }
}
