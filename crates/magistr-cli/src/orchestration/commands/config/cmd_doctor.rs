use crate::error::{self, ResultExt};
use crate::lint;
use crate::output::OutputRenderer;
use crate::utils::format as fmt;
use color_eyre::eyre;

pub fn cmd_doctor(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    let mut checks = lint::doctor(&dir);
    // The workbench `run` allowlist is seeded from project.json once; flag
    // the two drifting apart.
    checks.push(crate::scaffold::workbench_drift::doctor_check(&dir));

    if renderer.is_machine_readable() {
        let checks_json: Vec<serde_json::Value> = checks
            .iter()
            .map(|c| {
                serde_json::json!({
                    "name": c.name,
                    "description": c.description,
                    "status": c.status.label(),
                    "detail": c.detail,
                })
            })
            .collect();

        let fails = checks
            .iter()
            .filter(|c| c.status == lint::doctor::CheckStatus::Fail)
            .count();

        let result = serde_json::json!({
            "passed": fails == 0,
            "checks": checks_json,
        });

        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "{} — checking project health\n",
            fmt::bold("magistr doctor")
        );
        for c in &checks {
            let icon = c.status.icon();
            let colored_icon = match c.status {
                lint::doctor::CheckStatus::Pass => fmt::green(icon),
                lint::doctor::CheckStatus::Warn => fmt::yellow(icon),
                lint::doctor::CheckStatus::Fail => fmt::red(icon),
            };
            println!("  {colored_icon} {}: {}", c.name, c.description);
            if let Some(detail) = &c.detail {
                println!("    {}", fmt::dim(&format!("→ {detail}")));
            }
        }

        let fails = checks
            .iter()
            .filter(|c| c.status == lint::doctor::CheckStatus::Fail)
            .count();
        let warns = checks
            .iter()
            .filter(|c| c.status == lint::doctor::CheckStatus::Warn)
            .count();

        println!();
        if fails == 0 && warns == 0 {
            println!("{}", fmt::success("All checks passed!"));
        } else {
            println!("  {} fail(s), {} warning(s)", fails, warns);
        }
    }

    let has_failures = checks
        .iter()
        .any(|c| c.status == lint::doctor::CheckStatus::Fail);
    if has_failures {
        std::process::exit(1);
    }

    Ok(())
}
