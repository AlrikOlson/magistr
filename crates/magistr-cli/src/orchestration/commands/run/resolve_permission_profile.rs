use crate::permissions::{self, PermissionProfile};
use color_eyre::eyre;

/// Resolve the permission profile from the `--trust` CLI flag.
///
/// Falls back to the project's `.claude/permissions.toml` config if no flag is provided,
/// then to `PermissionProfile::Standard` as the ultimate default.
pub fn resolve_permission_profile(
    project_dir: &std::path::Path,
    trust_flag: Option<&str>,
) -> eyre::Result<PermissionProfile> {
    if let Some(trust) = trust_flag {
        trust
            .parse::<PermissionProfile>()
            .map_err(|e| eyre::eyre!(e))
    } else {
        // Try loading from project config, fall back to Standard
        let config = permissions::config::load_config(project_dir).unwrap_or_default();
        Ok(config.profile)
    }
}
