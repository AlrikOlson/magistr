use crate::utils::format as fmt;
use crate::validate;

/// Run post-scaffold validation and print results.
pub async fn print_post_scaffold_validation(dir: &std::path::Path) {
    eprintln!();
    eprintln!("{}", fmt::info("Validating scaffold (running checks)..."));

    // Validate the commands the project actually keeps after reinitialization.
    let configured = validate::load_project_config(dir)
        .map(|config| config.checks)
        .ok();
    let Some(configured) = configured else {
        eprintln!(
            "  {}",
            fmt::warning("Could not read the project's check commands.")
        );
        return;
    };
    for (name, command) in [
        ("check", &configured.check),
        ("test", &configured.test),
        ("lint", &configured.lint),
    ] {
        if command.trim().is_empty() {
            eprintln!("  {name}: not configured");
        }
    }
    let results = validate::post_scaffold_check(&configured, dir).await;

    if results.is_empty() {
        eprintln!("  {}", fmt::dim("(no check commands configured)"));
        return;
    }

    let all_passed = results.iter().all(|r| r.passed);
    for r in &results {
        eprintln!(
            "  {} {} ({:.1}s)",
            r.status_icon(),
            r.check.label(),
            r.duration.as_secs_f64(),
        );
    }

    if !all_passed {
        eprintln!();
        eprintln!(
            "  {}",
            fmt::yellow("⚠ Some checks failed. The scaffold was still created, but")
        );
        eprintln!(
            "  {}",
            fmt::yellow("  your check commands may need adjustment.")
        );
        eprintln!(
            "  {}",
            fmt::yellow("  Edit .magistr/project.json to fix the commands.")
        );
    }
}
