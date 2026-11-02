//! Recognized one-way setup upgrades. Planning never writes; all publication
//! uses the same verified administrative writer as init and repair.
use std::{collections::HashSet, path::Path};

use super::{AdminOperation, AdminService, ChangePlan, FileAction, PlannedChange};
use crate::{
    config::ProjectSettingsUpgrade,
    error::{MagistrError, Result},
};

pub(crate) struct ProjectUpgrade;

/// The YAML plan's file before the plan words, and its current name.
const OLDER_YAML_PLAN: &str = ".magistr-roadmap.yml";
const YAML_PLAN: &str = ".magistr-plan.yml";

impl ProjectUpgrade {
    pub(crate) fn content(path: &str, before: &str) -> Result<Option<String>> {
        match path {
            ".magistr/project.json" => ProjectSettingsUpgrade::prepare(before),
            OLDER_YAML_PLAN => Ok(Some(Self::yaml_plan(before))),
            ".mcp.json" => {
                let mut doc: serde_json::Value = serde_json::from_str(before)?;
                let changed = doc
                    .get_mut("mcpServers")
                    .and_then(|servers| servers.get_mut("magistr"))
                    .is_some_and(Self::connection);
                Ok(changed.then(|| {
                    format!(
                        "{}\n",
                        serde_json::to_string_pretty(&doc).expect("JSON value")
                    )
                }))
            }
            ".codex/config.toml" => {
                let mut doc = super::codex_config::CodexConfig::parse(before)?;
                let Some(server) = doc
                    .get_mut("mcp_servers")
                    .and_then(|v| v.get_mut("magistr"))
                else {
                    return Ok(None);
                };
                if server.get("url").is_some() {
                    return Ok(None);
                }
                let command = server
                    .get("command")
                    .and_then(toml_edit::Item::as_str)
                    .unwrap_or("");
                let Some(args) = server.get("args").and_then(toml_edit::Item::as_array) else {
                    return Ok(None);
                };
                let args: Option<Vec<String>> =
                    args.iter().map(|v| v.as_str().map(str::to_owned)).collect();
                let Some(args) = args.and_then(|args| Self::arguments(command, &args)) else {
                    return Ok(None);
                };
                server["args"] = toml_edit::value(args.into_iter().collect::<toml_edit::Array>());
                Ok(Some(doc.to_string()))
            }
            _ => Ok(None),
        }
    }

    /// The YAML plan in the plan words: phase statuses `pending` and
    /// `blocked` become `to_do` and `waiting`. A line rewrite keeps the
    /// author's comments, order and formatting.
    fn yaml_plan(before: &str) -> String {
        before
            .split_inclusive('\n')
            .map(|line| {
                let body = line.trim_end_matches(['\n', '\r']);
                let ending = &line[body.len()..];
                let Some((key, value)) = body.split_once(':') else {
                    return line.to_owned();
                };
                if key.trim() != "status" {
                    return line.to_owned();
                }
                let renamed = match value.trim() {
                    "pending" => "to_do",
                    "blocked" => "waiting",
                    _ => return line.to_owned(),
                };
                format!("{key}: {renamed}{ending}")
            })
            .collect()
    }

    pub(crate) fn connection(entry: &mut serde_json::Value) -> bool {
        if entry.get("url").is_some() {
            return false;
        }
        let command = entry["command"].as_str().unwrap_or("");
        let Some(args) = entry["args"].as_array() else {
            return false;
        };
        let args: Option<Vec<String>> =
            args.iter().map(|v| v.as_str().map(str::to_owned)).collect();
        let Some(args) = args.and_then(|args| Self::arguments(command, &args)) else {
            return false;
        };
        entry["args"] = serde_json::json!(args);
        true
    }

    fn arguments(command: &str, args: &[String]) -> Option<Vec<String>> {
        let name = Path::new(command).file_name()?.to_str()?;
        let offset = match name {
            "magistr" => 0,
            "npx"
                if args.len() == 7
                    && args[0] == "--yes"
                    && args[1] == "--package"
                    && args[2].starts_with("magistr@")
                    && args[3] == "magistr" =>
            {
                4
            }
            _ => return None,
        };
        if !matches!(&args[offset..], [verb, target, _] if verb == "mcp-launch" && target == "magistr")
        {
            return None;
        }
        let mut upgraded = args.to_vec();
        upgraded[offset] = "mcp".into();
        upgraded[offset + 1] = "--dir".into();
        Some(upgraded)
    }

    pub(crate) fn plan(root: &Path) -> Result<ChangePlan> {
        let mut plan = ChangePlan::new(AdminOperation::Repair);
        for path in [".magistr/project.json", ".mcp.json", ".codex/config.toml"] {
            let before = super::read_target(root, path)
                .map_err(|e| MagistrError::Config(format!("{path}: {e}")))?;
            if let Some(before) = before
                && let Some(after) = Self::content(path, &before)?
            {
                plan.files
                    .push(PlannedChange::merged(path, Some(before), after));
            }
        }
        if let Some(after) = magistr_workspace::config::WorkspaceConfigUpgrade::preview(root)
            .map_err(|e| MagistrError::io(root.join("workbench.toml"), e))?
        {
            let before = super::read_target(root, ".magistr/workspace.toml")
                .map_err(|e| MagistrError::Config(e.to_string()))?;
            if before.as_deref() != Some(&after) {
                plan.files.push(PlannedChange::merged(
                    ".magistr/workspace.toml",
                    before,
                    after,
                ));
            }
        }
        Self::include_yaml_plan(root, &mut plan)?;
        super::project_identity_upgrade::ProjectIdentityUpgrade::include(root, &mut plan)?;
        Ok(plan)
    }

    /// Move a YAML plan saved before the plan words to its current file. The
    /// administrative writer keeps the original under the upgrade backups
    /// before removing it; a current file already in place is never replaced.
    fn include_yaml_plan(root: &Path, plan: &mut ChangePlan) -> Result<()> {
        let read = |path: &str| {
            super::read_target(root, path).map_err(|e| MagistrError::Config(format!("{path}: {e}")))
        };
        let Some(before) = read(OLDER_YAML_PLAN)? else {
            return Ok(());
        };
        let after = Self::yaml_plan(&before);
        match read(YAML_PLAN)? {
            Some(current) if current != after => {
                return Err(MagistrError::Config(format!(
                    "Both {OLDER_YAML_PLAN} and {YAML_PLAN} exist. Keep the plan you want in {YAML_PLAN}, remove the other, and retry; neither was changed."
                )));
            }
            Some(_) => {}
            None => plan
                .files
                .push(PlannedChange::merged(YAML_PLAN, None, after)),
        }
        plan.files.push(PlannedChange::removal(
            OLDER_YAML_PLAN,
            before,
            super::Ownership::User,
        ));
        Ok(())
    }

    /// Fold known conversions into the full setup preview without replacing the
    /// rest of its generated content or touching a customized transport.
    pub(crate) fn include(root: &Path, plan: &mut ChangePlan) -> Result<()> {
        for upgrade in Self::plan(root)?.files {
            if let Some(change) = plan.files.iter_mut().find(|c| c.path == upgrade.path) {
                if matches!(change.action, FileAction::Preserved | FileAction::Unchanged) {
                    *change = upgrade;
                } else if let Some(after) = Self::content(&change.path, &change.new_content)? {
                    *change =
                        PlannedChange::merged(&change.path, change.existing_content.clone(), after);
                }
            } else {
                plan.files.push(upgrade);
            }
        }
        super::project_identity_upgrade::ProjectIdentityUpgrade::include(root, plan)?;
        plan.files
            .sort_by_key(|change| change.path != magistr_engine::native::WorkflowUpgrade::REQUEST);
        Ok(())
    }

    pub(crate) fn apply(root: &Path) -> Result<()> {
        let plan = Self::plan(root)?;
        if plan.files.is_empty() {
            return Ok(());
        }
        let mut service = AdminService::open(root, plan.operation, HashSet::<String>::new());
        for change in &plan.files {
            if service.execute(change).outcome.is_failed() {
                break;
            }
        }
        match service.finish().partial_error() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "project_upgrade.tests.rs"]
mod tests;
