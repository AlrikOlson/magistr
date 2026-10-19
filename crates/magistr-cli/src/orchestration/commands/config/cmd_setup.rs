use crate::{config::inputs::ProjectInputs, error::ResultExt, scaffold};
use color_eyre::eyre;
use std::{
    io::{self, IsTerminal},
    path::Path,
};

pub async fn cmd_setup(
    dir: &Path,
    remove: bool,
    dry_run: bool,
    yes: bool,
    inputs: &ProjectInputs,
) -> eyre::Result<()> {
    let dir = dir.canonicalize()?;
    let prepared = if remove {
        None
    } else {
        Some(scaffold::repair::prepare(&dir, inputs).suggest()?)
    };
    let plan = match &prepared {
        Some(prepared) => prepared.changes().clone(),
        None => scaffold::integrations::removal_plan(&dir).suggest()?,
    };
    crate::commands::init::print_plan_plain(
        &plan,
        if remove {
            "Remove agent setup"
        } else {
            "Repair agent setup"
        },
        None,
    );
    if dry_run {
        return Ok(());
    }
    if !yes {
        eyre::ensure!(
            io::stdin().is_terminal(),
            "Use --dry-run to review changes, then --yes to apply them without prompts."
        );
        if !scaffold::interview::prompt_yes_no(
            &mut io::stdin().lock(),
            "Apply these changes?",
            false,
        )
        .suggest()?
        {
            return Ok(());
        }
    }
    let paths = match prepared {
        Some(prepared) => prepared.apply().suggest()?,
        None => scaffold::integrations::apply_removal(&dir, plan).suggest()?,
    };
    eprintln!("Updated {} files.", paths.len());
    if !remove {
        crate::unified_mcp::check_connection(&dir).await?;
    }
    Ok(())
}
