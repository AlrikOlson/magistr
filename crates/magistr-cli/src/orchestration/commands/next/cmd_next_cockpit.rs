//! `magistr next` on the cockpit: the loop's single-iteration case.

use std::path::Path;

use color_eyre::eyre;

use magistr_engine::NativeEngines;

use crate::agent::Backend;
use crate::error::ResultExt;
use crate::utils::format as fmt;
use crate::workflow::WorkflowConfig;
use crate::workflow::iteration::IterationLoop;

/// One iteration on the cockpit: an [`IterationLoop`] with a budget of
/// one and no state on disk — a crash leaves nothing for `next-loop` to
/// recover.
pub async fn cmd_next_cockpit(
    dir: &Path,
    backend: Backend,
    profile: crate::permissions::PermissionProfile,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let dir = super::canonicalize_dir(dir)?;
    let inputs = super::workflow_inputs(&inputs.for_project(&dir), false)?;
    let native = crate::roadmap::admit_native_host(&dir)?;
    let wf = WorkflowConfig::load(&dir)
        .suggest()?
        .with_native(native.as_ref().map(NativeEngines::reader));
    if wf.spec_summary().suggest()?.all_done() {
        eprintln!(
            "{}",
            fmt::success("All spec items complete. Nothing to do.")
        );
        return Ok(());
    }
    wf.cleanup().suggest()?;
    let iter_loop = IterationLoop::open(&dir, &wf.spec_path, 1, native)
        .suggest()?
        .with_run_mode(magistr_engine::workflow::RunMode::OneChunk);
    super::run_cockpit_loop(&dir, wf, iter_loop, backend, false, profile, &inputs).await
}
