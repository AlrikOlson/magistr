use crate::agent;
use crate::commands;
use crate::detect;
use crate::error::{self, ResultExt};
use color_eyre::eyre;

/// Run the project setup interview in the terminal cockpit.
pub async fn cmd_init_interactive_tui(
    dir: &std::path::Path,
    force: bool,
    _env: &crate::env::MagistrEnv,
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    commands::misc::ensure_no_existing_scaffold(&dir, force)?;

    let stack = detect::detect_stack(&dir).suggest()?;
    let dir_str = dir.display().to_string();
    let init_prompt = agent::system_prompt::build_init_prompt(&stack, &dir_str);

    let backend = agent::Backend::default();
    let config =
        agent::AgentConfig::new("Set up this project with AI-assisted configuration.", &dir)
            .system_prompt(init_prompt)
            .permission_mode(crate::agent::PermissionMode::AcceptEdits)
            .backend(backend);

    let profile = commands::run::resolve_permission_profile(&dir, None)
        .unwrap_or(crate::permissions::PermissionProfile::Standard);
    commands::run::cmd_run_cockpit(config, profile).await
}
