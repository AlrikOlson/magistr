/// Collect quality gate commands from the project config (for auto-approval).
pub fn collect_gate_commands() -> Vec<String> {
    let cwd = std::env::current_dir().ok();
    let config_path = cwd.as_deref().map(|d| d.join(".magistr/project.json"));
    if let Some(path) = config_path
        && let Ok(content) = std::fs::read_to_string(&path)
        && let Ok(config) = serde_json::from_str::<crate::types::ProjectConfig>(&content)
    {
        return [
            config.quality_gate.check,
            config.quality_gate.test,
            config.quality_gate.lint,
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    }
    Vec::new()
}
