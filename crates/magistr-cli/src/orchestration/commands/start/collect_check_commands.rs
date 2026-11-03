/// Collect check commands from the selected project's config.
pub fn collect_check_commands(project_dir: &std::path::Path) -> Vec<String> {
    let Ok(config_path) = crate::paths::project_json(project_dir) else {
        return Vec::new();
    };
    if let Ok(content) = std::fs::read_to_string(&config_path)
        && let Ok(config) = serde_json::from_str::<crate::types::ProjectConfig>(&content)
    {
        return [config.checks.check, config.checks.test, config.checks.lint]
            .into_iter()
            .chain(config.custom_checks.into_iter().map(|check| check.command))
            .filter(|s| !s.is_empty())
            .collect();
    }
    Vec::new()
}
