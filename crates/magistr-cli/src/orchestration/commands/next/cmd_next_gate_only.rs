use std::io::Write;
use std::time::Instant;

use color_eyre::eyre;

use crate::cli::HeadlessFormat;
use crate::error::ResultExt;
use crate::utils::format as fmt;
use crate::validate;

/// Run quality gates only, without an AI iteration.
///
/// Runs the project's standard, archetype, and required custom gates and reports
/// results as plain text or JSON. Exits with code 1 if any gate fails.
pub async fn cmd_next_gate_only(dir: &std::path::Path, format: HeadlessFormat) -> eyre::Result<()> {
    let dir = super::canonicalize_dir(dir)?;
    let work_lease = magistr_core::workflow::loop_lease::LoopLease::acquire(&dir).suggest()?;
    work_lease.require_settled(&dir).suggest()?;

    let start = Instant::now();

    let result = validate::ProjectValidation::run(&dir).await.suggest()?;

    let duration = start.elapsed();
    let mut stdout = std::io::stdout().lock();

    match format {
        HeadlessFormat::Plain => {
            render_gates_plain(&mut stdout, &result, duration)?;
        }
        HeadlessFormat::Json => {
            render_gates_json(&mut stdout, &result, duration)?;
        }
    }

    if !result.all_passed() {
        std::process::exit(1);
    }

    Ok(())
}

/// Render gate results as plain text to the given writer.
fn render_gates_plain(
    w: &mut dyn Write,
    result: &validate::ProjectValidation,
    total_duration: std::time::Duration,
) -> std::io::Result<()> {
    for gr in &result.results {
        let status = if gr.passed() { "PASS" } else { "FAIL" };
        let dur = gr.duration.as_secs_f64();
        writeln!(w, "{status}  {:<6} ({dur:.1}s)  {}", gr.name, gr.command)?;
        if !gr.passed() && !gr.output.is_empty() {
            // Show first 20 lines of failure output
            for line in gr.output.lines().take(20) {
                writeln!(w, "  {line}")?;
            }
        }
    }

    let total = total_duration.as_secs_f64();
    let total_count = result.results.len();
    writeln!(w)?;
    if result.all_passed() {
        writeln!(
            w,
            "{}",
            fmt::success(&format!("All {total_count} gates passed ({total:.1}s)"))
        )?;
    } else {
        let failed = result.failed_count();
        writeln!(
            w,
            "{}",
            fmt::error(&format!(
                "{failed}/{total_count} gates failed ({total:.1}s)"
            ))
        )?;
    }
    Ok(())
}

/// Render gate results as a JSON report to the given writer.
fn render_gates_json(
    w: &mut dyn Write,
    result: &validate::ProjectValidation,
    total_duration: std::time::Duration,
) -> std::io::Result<()> {
    let gates: serde_json::Map<String, serde_json::Value> = result
        .results
        .iter()
        .map(|gr| {
            (
                gr.name.clone(),
                serde_json::json!({
                    "passed": gr.passed(),
                    "duration_secs": gr.duration.as_secs_f64(),
                    "command": gr.command,
                }),
            )
        })
        .collect();

    let report = serde_json::json!({
        "type": "gate_report",
        "all_passed": result.all_passed(),
        "passed_count": result.passed_count(),
        "failed_count": result.failed_count(),
        "duration_secs": total_duration.as_secs_f64(),
        "gates": gates,
    });

    serde_json::to_writer(&mut *w, &report)?;
    writeln!(w)?;
    Ok(())
}

#[cfg(test)]
#[path = "cmd_next_gate_only.tests.rs"]
mod tests;
