//! Explicit resolution of one interrupted agent attempt.

use std::io::Write;
use std::num::NonZeroU32;
use std::path::Path;

use crate::error::ResultExt;
use color_eyre::eyre;
use magistr_core::workflow::iteration::{IterationState, OperationOutcome};

pub fn cmd_resolve_dispatch(
    dir: &Path,
    run: &str,
    command: &str,
    attempt: NonZeroU32,
    outcome: &str,
    reason: &str,
    confirm_stopped: bool,
) -> eyre::Result<()> {
    if !confirm_stopped {
        return Err(eyre::eyre!(
            "Confirm that the loop and its agents have stopped with --confirm-stopped."
        ));
    }
    let outcome = match outcome {
        "completed" => OperationOutcome::Completed,
        "failed" => OperationOutcome::Failed,
        "cancelled" => OperationOutcome::Cancelled,
        _ => return Err(eyre::eyre!("Choose completed, failed or cancelled.")),
    };
    let dir = super::canonicalize_dir(dir)?;
    IterationState::resolve_dispatch(&dir, run, command, attempt, outcome, reason).suggest()?;
    writeln!(
        std::io::stdout().lock(),
        "Saved {outcome} for {command}, attempt {attempt}."
    )?;
    Ok(())
}
