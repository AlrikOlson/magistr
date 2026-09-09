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
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    let stack = detect::detect_stack(&dir).suggest()?;
    let preset =
        config::preset::load_preset_full(stack.language, stack.framework, stack.package_manager)
            .unwrap_or_else(|_| config::preset::fallback_preset());

    let yaml = scaffold::ci::generate_workflow(
        stack.language,
        stack.package_manager,
        &preset.quality_gate,
    );

    let result = scaffold::ci::write_workflow(&dir, &yaml, force).suggest()?;

    if renderer.is_machine_readable() {
        let (status, path) = match &result {
            scaffold::ci::CiWriteResult::Created(p) => ("created", p.display().to_string()),
            scaffold::ci::CiWriteResult::Overwritten(p) => ("overwritten", p.display().to_string()),
            scaffold::ci::CiWriteResult::AlreadyExists(p) => {
                ("already_exists", p.display().to_string())
            }
        };
        let data = serde_json::json!({
            "status": status,
            "path": path,
            "language": stack.language.as_str(),
            "package_manager": stack.package_manager.as_str(),
        });
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }

    match result {
        scaffold::ci::CiWriteResult::Created(path) => {
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
        scaffold::ci::CiWriteResult::Overwritten(path) => {
            eprintln!(
                "{} Overwrote CI workflow: {}",
                fmt::yellow("⟳"),
                path.display()
            );
        }
        scaffold::ci::CiWriteResult::AlreadyExists(path) => {
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
