//! `magistr next-loop` on the cockpit.

use std::path::Path;

use color_eyre::eyre;

use crate::agent::Backend;
use crate::error::ResultExt;
use crate::workflow::WorkflowConfig;
use crate::workflow::iteration::IterationLoop;

/// Open (or recover) the iteration loop for `dir` exactly as the legacy
/// entry point did — the native engines opened before raw mode, the
/// state persisted from the start — and run it on the cockpit.
pub async fn cmd_next_loop_cockpit(
    dir: &Path,
    max_iterations: u32,
    backend: Backend,
) -> eyre::Result<()> {
    let dir = super::canonicalize_dir(dir)?;
    // The reasoning server prints its load banner to stderr: open the
    // engines before the terminal is taken over. `None` on a yaml
    // roadmap, which reads its spec file as before.
    let native = crate::roadmap::open_native_engines(&dir);
    let wf = WorkflowConfig::load(&dir)
        .suggest()?
        .with_native(native.clone());
    wf.cleanup()?;
    let iter_loop = if IterationLoop::has_persisted_state(&dir) {
        IterationLoop::recover_with(&dir, &wf.spec_path, native).suggest()?
    } else {
        let il = IterationLoop::open(&dir, &wf.spec_path, max_iterations, native).suggest()?;
        il.persist().suggest()?;
        il
    };
    super::run_cockpit_loop(&dir, wf, iter_loop, backend, true).await
}
