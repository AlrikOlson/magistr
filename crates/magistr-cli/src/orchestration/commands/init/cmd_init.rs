use crate::commands;
use crate::config;
use crate::detect;
use crate::error::{self, ResultExt};
use crate::scaffold;
use crate::utils::format as fmt;
use color_eyre::eyre;

pub async fn cmd_init(
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

    // Try tree detection first — if multiple projects, use polyglot path.
    eprintln!(
        "{}",
        fmt::info(&format!("Detecting stack in {}...", dir.display()))
    );

    let tree = detect::build_project_tree(&dir).suggest()?;

    if tree.len() > 1 {
        return cmd_init_polyglot(&dir, &tree, agents, inputs).await;
    }

    // Single project — use the standard path.
    let stack = detect::detect_stack(&dir).suggest()?;

    eprintln!(
        "{}",
        fmt::info(&format!(
            "Detected: {} + {} ({})",
            stack.language, stack.framework, stack.package_manager
        ))
    );
    eprintln!(
        "{}",
        fmt::info(&format!(
            "Test: {} | Lint: {} | Format: {}",
            stack.test_framework, stack.linter, stack.formatter
        ))
    );

    let pipeline = scaffold::pipeline::ScaffoldPipeline::headless(stack.clone())
        .with_inputs(inputs)
        .with_agents(agents);

    eprintln!("{}", fmt::info("Scaffolding agent configuration..."));
    let written = pipeline.generate(&dir).suggest()?;

    for path in &written {
        eprintln!("{}", fmt::success(path));
    }

    // Validate tool availability
    let warnings = config::validate::validate_stack(&stack);
    if !warnings.is_empty() {
        eprintln!();
        eprintln!("{}", fmt::warning("Missing tools:"));
        for w in &warnings {
            eprintln!("  {} {w}", fmt::yellow("•"));
        }
    }

    // Post-scaffold validation: run quality gates to verify config
    commands::misc::print_post_scaffold_validation(&pipeline.preset.quality_gate, &dir).await;

    eprintln!();
    eprintln!("{}", fmt::bold("magistr scaffolded successfully"));
    eprintln!("  Project: {}", stack.project_name);
    eprintln!("  Stack:   {} + {}", stack.language, stack.framework);
    eprintln!("  Files:   {} generated", written.len());

    Ok(())
}

/// Polyglot init path — generates multi-archetype config from project tree.
async fn cmd_init_polyglot(
    dir: &std::path::Path,
    tree: &magistr_core::types::ProjectTree,
    agents: &[String],
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    use magistr_core::types::Topology;

    let topo_label = match tree.topology {
        Topology::SingleProject => "Single Project",
        Topology::FlatMonorepo => "Flat Monorepo",
        Topology::NestedFullstack => "Nested Fullstack",
        Topology::Workspace => "Workspace",
    };

    eprintln!(
        "{}",
        fmt::info(&format!("Detected {} projects ({topo_label})", tree.len()))
    );

    for project in &tree.projects {
        let path = if project.path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            project.path.display().to_string()
        };
        let lang = project.language.as_str();
        let fw = project
            .framework
            .map(|f| format!(" + {}", f.as_str()))
            .unwrap_or_default();
        let name = project.metadata.name.as_deref().unwrap_or("");
        eprintln!("  {} {path} [{lang}{fw}] {name}", fmt::cyan("•"));
    }

    let pipeline = scaffold::pipeline::ScaffoldPipeline::polyglot(tree)
        .with_inputs(inputs)
        .with_agents(agents);

    eprintln!(
        "{}",
        fmt::info("Scaffolding agent configuration (polyglot)...")
    );
    let written = pipeline.generate(dir).map_err(|e| eyre::eyre!("{e}"))?;

    for path in &written {
        eprintln!("{}", fmt::success(path));
    }

    // Post-scaffold validation with the primary project's preset
    commands::misc::print_post_scaffold_validation(&pipeline.preset.quality_gate, dir).await;

    eprintln!();
    eprintln!(
        "{}",
        fmt::bold("magistr scaffolded successfully (polyglot)")
    );
    eprintln!("  Projects:  {}", tree.len());
    eprintln!("  Topology:  {topo_label}");
    eprintln!("  Files:     {} generated", written.len());

    Ok(())
}
