use crate::error::{self, ResultExt};
use crate::utils::format as fmt;
use crate::validate::ProjectValidation;
use color_eyre::eyre;

pub async fn cmd_validate(dir: &std::path::Path) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    eprintln!("{}\n", fmt::info("Running quality gates..."));

    let result = ProjectValidation::run(&dir).await.suggest()?;

    for gr in &result.results {
        let duration = format!("{:.1}s", gr.duration.as_secs_f64());
        let symbol = if gr.passed() { "✓" } else { "✗" };
        let label = &gr.name;
        if gr.passed() {
            eprintln!("  {} {} ({duration})", fmt::green(symbol), label);
        } else {
            eprintln!("  {} {} ({duration})", fmt::red(symbol), label);
        }
        if !gr.passed() && !gr.output.is_empty() {
            // Show first few lines of failure output
            for line in gr.output.lines().take(20) {
                eprintln!("    {line}");
            }
        }
    }

    // The org's required-gate policy, on top of the run's own verdict.
    let policy = crate::team::TeamConfig::governance_for(&dir);
    let unmet = result.policy_violations(&policy);
    if !unmet.is_empty() {
        eprintln!();
        eprintln!(
            "{}",
            fmt::error(&format!(
                "policy requires {} — not satisfied by this run",
                unmet.join(", ")
            ))
        );
        eprintln!(
            "  {}",
            fmt::info(
                "configure the gate in .magistr/project.json, or ask an Owner to relax .magistr/team.json"
            )
        );
        std::process::exit(1);
    }

    eprintln!();
    if result.all_passed() {
        eprintln!(
            "{}",
            fmt::success(&format!(
                "All gates passed ({:.1}s)",
                result.total_duration().as_secs_f64()
            ))
        );
        Ok(())
    } else {
        eprintln!(
            "{}",
            fmt::error(&format!(
                "{}/{} gates failed",
                result.failed_count(),
                result.results.len()
            ))
        );
        std::process::exit(1);
    }
}
