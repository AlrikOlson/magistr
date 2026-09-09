use crate::scaffold;
use crate::scaffold::ScaffoldSelections;
use crate::scaffold::research::{RecommendedFormat, RecommendedQualityGate, RecommendedTesting};
use crate::types;
use crate::utils;

/// Build the template substitution map from a detected stack, preset, and selections.
pub fn build_template_vars(
    stack: &types::Stack,
    preset: &types::Preset,
    selections: &ScaffoldSelections,
) -> std::collections::HashMap<String, String> {
    log_ai_fallback_status(selections, preset);

    let (check_cmd, test_cmd, lint_cmd) =
        resolve_quality_gate(&selections.quality_gate, &preset.quality_gate);
    let (format_cmd, format_extensions) = resolve_format(&selections.format, &preset.format);
    let (companion_pattern, test_dir) = resolve_testing(&selections.testing, &preset.testing);

    let allowed_tools = format_quoted_list(&preset.allowed_tools, ",\n      ");
    let selected_agents = selections.selected_agents.join(",");
    let selected_mcp_servers = selections.selected_mcp_servers.join(",");
    let mcp_permissions =
        super::cmd_plugins::build_mcp_permissions(&selections.selected_mcp_servers);
    let ai_conventions = format_bullets(&selections.conventions);
    let ai_tips = format_bullets(&selections.tips);
    let reference_repos = format_quoted_list(&selections.reference_repos, ", ");

    scaffold::vars([
        ("project_name", stack.project_name.as_str()),
        ("project_description", selections.description.as_str()),
        ("project_type", selections.project_type.as_str()),
        ("language", stack.language.as_str()),
        ("framework", stack.framework.as_str()),
        ("package_manager", stack.package_manager.as_str()),
        ("test_framework", stack.test_framework.as_str()),
        ("linter", stack.linter.as_str()),
        ("formatter", stack.formatter.as_str()),
        ("check_cmd", check_cmd),
        ("test_cmd", test_cmd),
        ("lint_cmd", lint_cmd),
        ("format_cmd", format_cmd),
        ("format_extensions", &format_extensions),
        ("companion_pattern", companion_pattern),
        ("test_dir", test_dir),
        (
            "state_prefix",
            &utils::slug::sanitize_prefix(&stack.project_name),
        ),
        ("allowed_tools", &allowed_tools),
        ("selected_agents", &selected_agents),
        ("selected_mcp_servers", &selected_mcp_servers),
        ("mcp_permissions", &mcp_permissions),
        ("ai_conventions", &ai_conventions),
        ("ai_tips", &ai_tips),
        ("reference_repos", &reference_repos),
    ])
}

// ── Resolution helpers ─────────────────────────────────────────────────

/// Resolve quality gate commands: AI recommendation wins over preset.
fn resolve_quality_gate<'a>(
    ai: &'a Option<RecommendedQualityGate>,
    preset: &'a types::QualityGate,
) -> (&'a str, &'a str, &'a str) {
    let check = ai
        .as_ref()
        .and_then(|qg| qg.check.as_deref())
        .unwrap_or(&preset.check);
    let test = ai
        .as_ref()
        .and_then(|qg| qg.test.as_deref())
        .unwrap_or(&preset.test);
    let lint = ai
        .as_ref()
        .and_then(|qg| qg.lint.as_deref())
        .unwrap_or(&preset.lint);
    (check, test, lint)
}

/// Resolve format config: AI recommendation wins over preset.
fn resolve_format<'a>(
    ai: &'a Option<RecommendedFormat>,
    preset: &'a types::FormatConfig,
) -> (&'a str, String) {
    let cmd = ai
        .as_ref()
        .and_then(|f| f.command.as_deref())
        .unwrap_or(&preset.command);
    let exts = ai
        .as_ref()
        .and_then(|f| f.extensions.as_ref())
        .unwrap_or(&preset.extensions);
    (cmd, format_quoted_list(exts, ", "))
}

/// Resolve testing config: AI recommendation wins over preset.
fn resolve_testing<'a>(
    ai: &'a Option<RecommendedTesting>,
    preset: &'a types::TestingConfig,
) -> (&'a str, &'a str) {
    let companion = ai
        .as_ref()
        .and_then(|t| t.companion_pattern.as_deref())
        .unwrap_or(&preset.companion_pattern);
    let dir = ai
        .as_ref()
        .and_then(|t| t.test_dir.as_deref())
        .unwrap_or(&preset.test_dir);
    (companion, dir)
}

// ── Formatting helpers ─────────────────────────────────────────────────

/// Format items as quoted strings joined by a separator: `"a", "b"`.
fn format_quoted_list(items: &[String], sep: &str) -> String {
    items
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(sep)
}

/// Format items as markdown bullet points, or empty string if none.
fn format_bullets(items: &[String]) -> String {
    if items.is_empty() {
        String::new()
    } else {
        items
            .iter()
            .map(|s| format!("- {s}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Log which AI fields were populated vs fell back to preset.
fn log_ai_fallback_status(selections: &ScaffoldSelections, preset: &types::Preset) {
    if selections.conventions.is_empty() {
        tracing::debug!("No AI conventions — using preset defaults");
    } else {
        tracing::debug!("AI provided {} conventions", selections.conventions.len());
    }
    if selections.quality_gate.is_none() {
        tracing::debug!(
            "No AI quality gate — using preset: {}",
            preset.quality_gate.check
        );
    }
    if selections.tips.is_empty() {
        tracing::debug!("No AI tips — scaffold will omit tips section");
    }
}
