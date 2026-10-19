//! Remove generated agent setup while retaining project configuration and native work.
use super::{
    AdminOperation, AdminService, ChangePlan, CodexSetup, Ownership, PlannedChange,
    manifest::ScaffoldManifest, owned_removal::OwnedRemoval,
};
use crate::error::Result;
use std::{collections::HashMap, path::Path};

pub(crate) fn removal_plan(dir: &Path) -> Result<ChangePlan> {
    CodexSetup::validate_manifest(dir)?;
    let manifest = ScaffoldManifest::load(dir);
    let mut plan = CodexSetup::removal_plan(dir)?;
    let removal = OwnedRemoval::new(dir, &manifest);
    let vars = HashMap::from([(
        "selected_agents".into(),
        "claude,portable,cursor,copilot,cline,windsurf,aider".into(),
    )]);
    for file in super::scaffold_files(&vars) {
        if !file.path.starts_with(".magistr/")
            || matches!(
                file.path,
                ".magistr/instructions.md" | ".magistr/conventions.md"
            )
        {
            plan.files.extend(removal.plan(file.path));
        }
    }
    if let Some(before) = CodexSetup::read(dir, ".mcp.json")? {
        let mut doc: serde_json::Value = serde_json::from_str(&before)?;
        if let Some(servers) = doc
            .get_mut("mcpServers")
            .and_then(serde_json::Value::as_object_mut)
        {
            if servers
                .get("magistr")
                .is_some_and(|entry| super::is_generated_mcp_entry(dir, "magistr", entry))
            {
                servers.remove("magistr");
                let after = format!("{}\n", serde_json::to_string_pretty(&doc)?);
                plan.files
                    .push(PlannedChange::merged(".mcp.json", Some(before), after));
            } else {
                plan.files.push(PlannedChange::preserved(
                    ".mcp.json",
                    Some(before),
                    Ownership::User,
                ));
            }
        }
    }
    plan.operation = AdminOperation::Removal;
    Ok(plan)
}

/// Apply exactly the displayed removal, revalidating each file before changing it.
pub(crate) fn apply_removal(dir: &Path, plan: ChangePlan) -> Result<Vec<String>> {
    let mut service = AdminService::open(dir, plan.operation, Vec::<String>::new());
    let mut failed = false;
    for change in &plan.files {
        failed |= matches!(
            service.execute(change).outcome,
            super::ApplyOutcome::Failed(_) | super::ApplyOutcome::Refused { .. }
        );
    }
    if !failed {
        service.manifest_mut().codex = None;
    }
    // Retire selection only after every planned component removal succeeded.
    // Failed changes retain ownership so a later explicit removal can retry them.
    let receipt = service.finish();
    match receipt.partial_error() {
        Some(error) => Err(error),
        None => Ok(receipt.applied_paths()),
    }
}
