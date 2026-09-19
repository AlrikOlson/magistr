use super::*;
use crate::detect;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use crate::scaffold;
use color_eyre::eyre;

pub fn cmd_init_dry_run(
    dir: &std::path::Path,
    renderer: &dyn OutputRenderer,
    agents: &[String],
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    use std::io::IsTerminal;

    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    let stack = detect::detect_stack(&dir).suggest()?;
    let pipeline = scaffold::pipeline::ScaffoldPipeline::headless(stack)
        .with_inputs(inputs)
        .with_agents(agents);
    let preview = pipeline.preview(&dir).suggest()?;

    if renderer.is_machine_readable() {
        let json = plan_json(&preview, PlanDetail::Full);
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else if std::io::stdin().is_terminal() {
        // Interactive mode: walk through files with [a]pply [s]kip [v]iew [A]ll [q]uit
        scaffold::interactive::run_interactive(&preview, &dir).suggest()?;
    } else {
        // Non-interactive (piped): just print the preview
        print_plan_plain(
            &preview,
            "📋 Scaffold preview (dry run — no files will be written)",
            Some(&format!(
                "Run {} to apply these changes.",
                crate::utils::format::bold("magistr init")
            )),
        );
    }

    Ok(())
}
