use crate::commands;
use crate::config;
use crate::detect;
use crate::error::{self, ResultExt};
use crate::scaffold;
use crate::utils::format as fmt;
use color_eyre::eyre;

/// `magistr init` as a plain shell interview: detection summary, archetype
/// confirmation (including the polyglot project-tree confirmation),
/// agent/MCP/hook choices and a quality-gate preview, then the scaffold.
///
/// With `yes` every prompt is answered with its default, non-interactively —
/// the same answer set an interview accepts when every prompt gets Enter.
pub async fn cmd_init_interview(
    dir: &std::path::Path,
    force: bool,
    yes: bool,
    agents: &[String],
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    // Re-init must keep the agent choices the project already expressed.
    let had_agents = scaffold::agents_present(&dir);

    commands::misc::ensure_no_existing_scaffold(&dir, force)?;

    eprintln!(
        "{}",
        fmt::info(&format!("Detecting stack in {}...", dir.display()))
    );
    let tree = detect::build_project_tree(&dir).suggest()?;
    let stdin = std::io::stdin();

    // Polyglot: confirm the detected project tree before anything else.
    let tree = if tree.len() > 1 {
        if yes {
            Some(tree)
        } else {
            let mut reader = stdin.lock();
            match scaffold::interview::confirm_project_tree(&mut reader, &tree).suggest()? {
                Some(t) => Some(t),
                None => {
                    eprintln!(
                        "{}",
                        fmt::warning("Every project excluded — nothing to scaffold.")
                    );
                    return Ok(());
                }
            }
        }
    } else {
        None
    };

    // The stack driving the interview: the single project's, or the polyglot
    // primary's (mirrors `ScaffoldPipeline::polyglot`).
    let stack = match &tree {
        Some(t) => t
            .projects
            .first()
            .map(|p| {
                let abs = t.root.join(&p.path);
                detect::detect_stack(&abs).unwrap_or_default()
            })
            .unwrap_or_default(),
        None => detect::detect_stack(&dir).suggest()?,
    };
    let preset = config::preset::load_preset_layered(
        stack.language,
        stack.framework,
        stack.package_manager,
        &inputs.preset_layers(),
    )
    .unwrap_or_else(|_| config::preset::fallback_preset());

    let result = if yes {
        // The defaults, widened by what the project already supports —
        // an interactive run asks instead, and an answer is a choice.
        let mut defaults = scaffold::interview::InterviewResult::defaults_for(&stack);
        for agent in &had_agents {
            if !defaults.selected_agents.contains(agent) {
                defaults.selected_agents.push(agent.clone());
            }
        }
        defaults
    } else {
        let mut reader = stdin.lock();
        match scaffold::interview::run_interview(&mut reader, &stack, &preset) {
            Ok(r) => r,
            Err(error::MagistrError::Cancelled(msg)) => {
                eprintln!("{}", fmt::warning(&format!("Init cancelled: {msg}")));
                return Ok(());
            }
            Err(e) => return Err(e).suggest(),
        }
    };
    let include_hooks = result.include_hooks;
    let selections = result.to_selections();

    let pipeline = match &tree {
        Some(t) => {
            let mut p = scaffold::pipeline::ScaffoldPipeline::polyglot(t);
            p.selections = selections;
            p
        }
        None => scaffold::pipeline::ScaffoldPipeline::with_selections(stack.clone(), selections),
    }
    .with_inputs(inputs)
    .with_agents(agents);

    eprintln!("{}", fmt::info("Scaffolding agent configuration..."));
    let is_new_config = !crate::paths::project_json(&dir).exists();
    let written = pipeline.generate(&dir).suggest()?;
    for path in &written {
        eprintln!("{}", fmt::success(path));
    }

    // Declining on a fresh setup removes the preset hooks just injected.
    // Reinitialization keeps the existing project's explicit hook choices.
    if is_new_config && !include_hooks {
        strip_lifecycle_hooks(&dir);
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

    commands::misc::print_post_scaffold_validation(&pipeline.preset.quality_gate, &dir).await;

    eprintln!();
    eprintln!("{}", fmt::bold("magistr scaffolded successfully"));
    eprintln!("  Project: {}", stack.project_name);
    eprintln!("  Stack:   {} + {}", stack.language, stack.framework);
    eprintln!("  Files:   {} generated", written.len());

    Ok(())
}

/// Remove the lifecycle hooks `generate()` wrote into project.json.
fn strip_lifecycle_hooks(dir: &std::path::Path) {
    let config_path = crate::paths::project_json(dir);
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return;
    };
    let Ok(mut config) = serde_json::from_str::<crate::types::ProjectConfig>(&content) else {
        return;
    };
    config.lifecycle_hooks.clear();
    if let Ok(json) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(&config_path, json);
    }
}
