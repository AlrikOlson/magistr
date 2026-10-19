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
    _force: bool,
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
    let mut layers = inputs.preset_layers();
    layers.project_root = Some(
        tree.as_ref()
            .and_then(|tree| {
                tree.projects
                    .first()
                    .map(|project| tree.root.join(&project.path))
            })
            .unwrap_or_else(|| dir.clone())
            .into_boxed_path(),
    );
    let preset = config::preset::load_preset_layered(
        stack.language,
        stack.framework,
        stack.package_manager,
        &layers,
    )
    .unwrap_or_else(|_| config::preset::fallback_preset());

    let result = if yes {
        // The defaults, widened by what the project already supports —
        // an interactive run asks instead, and an answer is a choice.
        let mut defaults = scaffold::interview::InterviewResult::defaults_for(&stack);
        if !had_agents.is_empty() {
            defaults.selected_agents = had_agents.clone();
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

    let prepared = pipeline.prepare(&dir).suggest()?;
    super::print_plan_plain(prepared.changes(), "Proposed project setup", None);
    if !yes
        && !scaffold::interview::prompt_yes_no(&mut stdin.lock(), "Apply these changes?", true)
            .suggest()?
    {
        eprintln!("Setup cancelled. No files changed.");
        return Ok(());
    }
    let written = prepared.apply().suggest()?;
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

    crate::unified_mcp::check_connection(&dir).await?;
    for agent in &pipeline.selections.selected_agents {
        if matches!(agent.as_str(), "codex" | "claude") && which::which(agent).is_err() {
            eprintln!(
                "{} is not installed. Its project setup is ready; install the agent and open this project.",
                agent
            );
        }
    }

    eprintln!();
    eprintln!("{}", fmt::bold("magistr scaffolded successfully"));
    eprintln!("  Project: {}", stack.project_name);
    eprintln!("  Stack:   {} + {}", stack.language, stack.framework);
    eprintln!("  Files:   {} generated", written.len());

    eprintln!("MCP connection and read-only context check passed.");
    eprintln!(
        "Starter prompt: Read the project context and source, then propose the first plan. Wait for my instruction before implementing it."
    );
    Ok(())
}
