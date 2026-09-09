/// Collect quality gate commands from the selected project's config.
pub fn collect_gate_commands(project_dir: &std::path::Path) -> Vec<String> {
    let config_path = project_dir.join(".magistr/project.json");
    if let Ok(content) = std::fs::read_to_string(&config_path)
        && let Ok(config) = serde_json::from_str::<crate::types::ProjectConfig>(&content)
    {
        return [
            config.quality_gate.check,
            config.quality_gate.test,
            config.quality_gate.lint,
        ]
        .into_iter()
        .chain(config.custom_gates.into_iter().map(|gate| gate.command))
        .filter(|s| !s.is_empty())
        .collect();
    }
    Vec::new()
}
