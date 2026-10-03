//! Workflow commands: next, next --continue, next-loop (TUI and non-TUI variants),
//! and headless/gate-only modes for CI pipelines.

mod cmd_next;
#[cfg(feature = "terminal")]
mod cmd_next_cockpit;
mod cmd_next_continue;
mod cmd_next_gate_only;
mod cmd_next_headless;
mod cmd_next_loop;
#[cfg(feature = "terminal")]
mod cmd_next_loop_cockpit;
mod cmd_next_loop_headless;
#[cfg(feature = "terminal")]
mod cmd_next_loop_tui;
#[cfg(feature = "terminal")]
mod cmd_next_tui;
mod cmd_resolve_dispatch;
#[cfg(feature = "terminal")]
mod run_cockpit_loop;
pub use cmd_resolve_dispatch::cmd_resolve_dispatch;

pub use cmd_next::cmd_next;
#[cfg(feature = "terminal")]
pub use cmd_next_cockpit::cmd_next_cockpit;
pub use cmd_next_continue::cmd_next_continue;
pub use cmd_next_gate_only::cmd_next_gate_only;
pub use cmd_next_headless::cmd_next_headless;
pub use cmd_next_loop::cmd_next_loop;
#[cfg(feature = "terminal")]
pub use cmd_next_loop_cockpit::cmd_next_loop_cockpit;
pub use cmd_next_loop_headless::cmd_next_loop_headless;
#[cfg(feature = "terminal")]
pub use cmd_next_loop_tui::cmd_next_loop_tui;
#[cfg(feature = "terminal")]
pub use cmd_next_tui::cmd_next_tui;
#[cfg(feature = "terminal")]
pub(crate) use run_cockpit_loop::run_cockpit_loop;

/// Canonicalize a project directory path, converting IO errors to `MagistrError`.
///
/// Shared by all `next*` command handlers to avoid repeating the
/// `canonicalize → map_err → suggest` chain.
fn canonicalize_dir(dir: &std::path::Path) -> color_eyre::eyre::Result<std::path::PathBuf> {
    use crate::error::{MagistrError, ResultExt};
    dir.canonicalize()
        .map_err(|e| MagistrError::io(dir, e))
        .suggest()
}
mod workflow_inputs;
use workflow_inputs::workflow_inputs;

mod select_loop_track;
use select_loop_track::select_loop_track;

mod loop_request;
pub use loop_request::LoopRequest;
#[cfg(feature = "terminal")]
mod cockpit_run;
#[cfg(feature = "terminal")]
pub(crate) use cockpit_run::CockpitRun;
