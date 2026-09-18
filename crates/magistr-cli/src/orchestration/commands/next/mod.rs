//! Workflow commands: next, next --continue, next-loop (TUI and non-TUI variants),
//! and headless/gate-only modes for CI pipelines.

mod cmd_next;
mod cmd_next_cockpit;
mod cmd_next_continue;
mod cmd_next_gate_only;
mod cmd_next_headless;
mod cmd_next_loop;
mod cmd_next_loop_cockpit;
mod cmd_next_loop_headless;
mod cmd_next_loop_tui;
mod cmd_next_tui;
mod cmd_resolve_dispatch;
mod run_cockpit_loop;
pub use cmd_resolve_dispatch::cmd_resolve_dispatch;

pub use cmd_next::cmd_next;
pub use cmd_next_cockpit::cmd_next_cockpit;
pub use cmd_next_continue::cmd_next_continue;
pub use cmd_next_gate_only::cmd_next_gate_only;
pub use cmd_next_headless::cmd_next_headless;
pub use cmd_next_loop::cmd_next_loop;
pub use cmd_next_loop_cockpit::cmd_next_loop_cockpit;
pub use cmd_next_loop_headless::cmd_next_loop_headless;
pub use cmd_next_loop_tui::cmd_next_loop_tui;
pub use cmd_next_tui::cmd_next_tui;
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
