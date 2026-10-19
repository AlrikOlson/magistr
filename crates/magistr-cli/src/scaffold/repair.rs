//! Explicit repair uses the same reviewed setup plan and ownership checks as init.
use super::{
    ChangePlan,
    pipeline::{PreparedSetup, ScaffoldPipeline},
};
use crate::{config::inputs::ProjectInputs, error::Result};
use std::path::Path;

pub(crate) fn prepare(dir: &Path, inputs: &ProjectInputs) -> Result<PreparedSetup> {
    crate::validate::load_project_config(dir)?;
    let stack = crate::detect::detect_stack(dir)?;
    Ok(ScaffoldPipeline::headless(stack)
        .with_inputs(inputs)
        .prepare(dir)?
        .for_repair())
}

/// Read-only preview. No repair runs when MCP starts.
pub(crate) fn plan(dir: &Path) -> Result<ChangePlan> {
    Ok(prepare(dir, &ProjectInputs::from_env(dir))?
        .changes()
        .clone())
}

pub fn repair(dir: &Path) -> Result<Vec<String>> {
    prepare(dir, &ProjectInputs::from_env(dir))?.apply()
}

#[cfg(test)]
#[path = "repair.tests.rs"]
mod tests;
