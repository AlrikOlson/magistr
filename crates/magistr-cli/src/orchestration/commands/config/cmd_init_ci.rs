use crate::config;
use crate::detect;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use crate::scaffold;
use crate::utils::format as fmt;
use color_eyre::eyre;

/// Generate a CI workflow file from detected stack.
pub fn cmd_init_ci(
    dir: &std::path::Path,
    force: bool,
    renderer: &dyn OutputRenderer,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    let stack = detect::detect_stack(&dir).suggest()?;
    let mut layers = inputs.preset_layers();
    layers.project_root = Some(dir.clone().into_boxed_path());
    let preset = config::preset::load_preset_layered(
        stack.language,
        stack.framework,
        stack.package_manager,
        &layers,
    )
    .unwrap_or_else(|_| config::preset::fallback_preset());

    let yaml = scaffold::ci::generate_workflow(
        stack.language,
        stack.package_manager,
        &preset.quality_gate,
    );

    // The workflow file is an administrative change like any other: planned
    // first, then applied through the administrative service, which
    // revalidates the target at its write and answers a receipt. A target
    // that changed after planning is reported failed, never overwritten.
    let plan = scaffold::ci::plan_workflow(&dir, yaml, force);
    let receipt = scaffold::ci::apply_workflow(&dir, &plan);
    let outcome = scaffold::ci::CiOutcome::of(&receipt).suggest()?;
    let change = receipt
        .get(scaffold::ci::WORKFLOW_PATH)
        .expect("an outcome was read from the workflow's receipt");
    let path = dir.join(scaffold::ci::WORKFLOW_PATH);

    if renderer.is_machine_readable() {
        let data = serde_json::json!({
            "status": outcome.label(),
            "path": path.display().to_string(),
            "language": stack.language.as_str(),
            "package_manager": stack.package_manager.as_str(),
            "operation": receipt.operation.label(),
            "action": change.action.label(),
            "ownership": change.ownership.label(),
            "outcome": change.outcome.label(),
        });
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }

    match outcome {
        scaffold::ci::CiOutcome::Created => {
            eprintln!(
                "{}",
                fmt::success(&format!("Generated CI workflow: {}", path.display()))
            );
            eprintln!(
                "  {}",
                fmt::dim(&format!(
                    "Stack: {} + {} | Gates: check → test → lint",
                    stack.language.as_str(),
                    stack.package_manager.as_str(),
                ))
            );
        }
        scaffold::ci::CiOutcome::Overwritten => {
            eprintln!(
                "{} Overwrote CI workflow: {}",
                fmt::yellow("⟳"),
                path.display()
            );
        }
        scaffold::ci::CiOutcome::Unchanged => {
            eprintln!(
                "{}",
                fmt::success(&format!(
                    "CI workflow already up to date: {}",
                    path.display()
                ))
            );
        }
        scaffold::ci::CiOutcome::AlreadyExists => {
            eprintln!(
                "{} CI workflow already exists: {}",
                fmt::yellow("!"),
                path.display()
            );
            eprintln!("  {}", fmt::dim("Use --force to overwrite."));
        }
    }

    Ok(())
}
