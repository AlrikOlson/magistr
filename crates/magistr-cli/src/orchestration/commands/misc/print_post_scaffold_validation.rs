use crate::types;
use crate::utils::format as fmt;
use crate::validate;

/// Run post-scaffold validation and print results.
pub async fn print_post_scaffold_validation(qg: &types::QualityGate, dir: &std::path::Path) {
    eprintln!();
    eprintln!(
        "{}",
        fmt::info("Validating scaffold (running quality gates)...")
    );

    // Validate the commands the project actually keeps after reinitialization.
    // Read without load_project_config's write-back migration.
    let configured = std::fs::read(crate::paths::project_json(dir))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|doc| doc.get("quality_gate").cloned())
        .and_then(|value| serde_json::from_value::<types::QualityGate>(value).ok());
    let results = validate::post_scaffold_check(configured.as_ref().unwrap_or(qg), dir).await;

    if results.is_empty() {
        eprintln!("  {}", fmt::dim("(no quality gate commands configured)"));
        return;
    }

    let all_passed = results.iter().all(|r| r.passed);
    for r in &results {
        eprintln!(
            "  {} {} ({:.1}s)",
            r.status_icon(),
            r.gate.label(),
            r.duration.as_secs_f64(),
        );
    }

    if !all_passed {
        eprintln!();
        eprintln!(
            "  {}",
            fmt::yellow("⚠ Some gates failed. The scaffold was still created, but")
        );
        eprintln!(
            "  {}",
            fmt::yellow("  your quality gate commands may need adjustment.")
        );
        eprintln!(
            "  {}",
            fmt::yellow("  Edit .magistr/project.json to fix the commands.")
        );
    }
}
