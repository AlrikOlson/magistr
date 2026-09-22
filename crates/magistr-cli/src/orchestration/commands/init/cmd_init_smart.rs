use crate::commands;
use crate::config;
use crate::detect;
use crate::error::{self, ResultExt};
use crate::scaffold;
use crate::utils::format as fmt;
use color_eyre::eyre;

pub async fn cmd_init_smart(
    dir: &std::path::Path,
    force: bool,
    agents: &[String],
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    commands::misc::ensure_no_existing_scaffold(&dir, force)?;

    eprintln!(
        "{}",
        fmt::info(&format!("Smart-detecting stack in {}...", dir.display()))
    );
    let smart = detect::detect_stack_smart(&dir).suggest()?;

    // Show confidence-scored detection
    commands::misc::print_smart_stack(&smart);
    eprintln!();

    // Show wizard intelligence: common setup detection + category summary
    commands::misc::print_wizard_summary(&smart);

    let low = smart.low_confidence_categories();
    if !low.is_empty() {
        eprintln!(
            "{}",
            fmt::warning(&format!("Low-confidence detections: {}.", low.join(", ")))
        );
        eprintln!("  Consider passing explicit values or using `magistr init --interactive`.");
        eprintln!();
    }

    // Scaffold using the detected stack (stripping confidence metadata)
    let stack = smart.to_stack();
    let pipeline = scaffold::pipeline::ScaffoldPipeline::headless(stack.clone())
        .with_inputs(inputs)
        .with_agents(agents);

    eprintln!("{}", fmt::info("Scaffolding agent configuration..."));
    let written = pipeline.generate(&dir).suggest()?;

    for path in &written {
        eprintln!("{}", fmt::success(path));
    }

    let warnings = config::validate::validate_stack(&stack);
    if !warnings.is_empty() {
        eprintln!();
        eprintln!("{}", fmt::warning("Missing tools:"));
        for w in &warnings {
            eprintln!("  {} {w}", fmt::yellow("•"));
        }
    }

    // Post-scaffold validation: run quality gates to verify config
    commands::misc::print_post_scaffold_validation(&dir).await;

    eprintln!();
    eprintln!("{}", fmt::bold("magistr scaffolded successfully"));
    eprintln!("  Project: {}", stack.project_name);
    eprintln!("  Stack:   {} + {}", stack.language, stack.framework);
    eprintln!("  Files:   {} generated", written.len());

    Ok(())
}
