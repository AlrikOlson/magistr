//! `magistr next-loop` on the cockpit.

use std::path::Path;

use color_eyre::eyre;

use magistr_engine::NativeEngines;

use crate::agent::Backend;
use crate::error::ResultExt;
use crate::workflow::WorkflowConfig;
use crate::workflow::iteration::IterationLoop;

/// Start the requested loop for `dir`, recovering unfinished work and
/// archiving a settled previous run. Admit the native engines and persist
/// the loop before entering raw mode.
pub async fn cmd_next_loop_cockpit(
    dir: &Path,
    max_iterations: u32,
    backend: Backend,
    profile: crate::permissions::PermissionProfile,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let dir = super::canonicalize_dir(dir)?;
    let _loop_lease = magistr_core::workflow::loop_lease::LoopLease::acquire(&dir).suggest()?;
    let inputs = super::workflow_inputs(&inputs.for_project(&dir), true)?;
    // The reasoning server prints its load banner to stderr: admit the
    // host and open the engines before the terminal is taken over. `None`
    // on a yaml roadmap, which reads its spec file as before.
    let native = crate::roadmap::admit_native_host(&dir)?;
    let wf = WorkflowConfig::load(&dir)
        .suggest()?
        .with_native(native.as_ref().map(NativeEngines::reader));
    let mut iter_loop =
        IterationLoop::start_or_recover(&dir, &wf.spec_path, max_iterations, native).suggest()?;
    if let Some(selection) = inputs.session_selection() {
        iter_loop.bind_session_selection(selection.clone())?;
    }
    iter_loop.persist().suggest()?;
    super::run_cockpit_loop(&dir, wf, iter_loop, backend, true, profile, &inputs).await
}
