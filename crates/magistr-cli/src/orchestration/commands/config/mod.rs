//! Configuration management commands: export, import, migrate, lint, doctor.

use crate::detect;
use crate::lint;
use crate::utils::format as fmt;
use color_eyre::eyre;

pub(crate) fn print_lint_report(report: &lint::LintReport) {
    if report.diagnostics.is_empty() {
        println!("{} {} — no issues found", fmt::icon_pass(), report.path);
        return;
    }

    println!("Linting {}\n", report.path);
    for d in &report.diagnostics {
        let loc = d.line.map(|l| format!(":{l}")).unwrap_or_default();
        let rule_loc = format!("{}{loc}", d.rule);
        let colored_rule = match d.severity {
            lint::Severity::Error => fmt::red(&rule_loc),
            lint::Severity::Warning => fmt::yellow(&rule_loc),
            lint::Severity::Info => fmt::cyan(&rule_loc),
        };
        println!("  {colored_rule}: {}", d.message);
        if let Some(fix) = &d.fix {
            println!("    {}", fmt::dim(&format!("fix: {fix}")));
        }
    }
    println!();
    println!(
        "  {} error(s), {} warning(s)",
        report.errors(),
        report.warnings()
    );
}

pub(crate) fn print_lint_json(
    report: &lint::LintReport,
    suggest: bool,
    dir: &std::path::Path,
) -> eyre::Result<()> {
    let diagnostics: Vec<serde_json::Value> = report
        .diagnostics
        .iter()
        .map(|d| {
            serde_json::json!({
                "rule": d.rule,
                "severity": d.severity.label(),
                "message": d.message,
                "line": d.line,
                "fix": d.fix,
            })
        })
        .collect();

    let mut result = serde_json::json!({
        "path": report.path,
        "passed": report.passed(),
        "errors": report.errors(),
        "warnings": report.warnings(),
        "diagnostics": diagnostics,
    });

    if suggest && let Ok(stack) = detect::detect_stack(dir) {
        let claude_md = dir.join("CLAUDE.md");
        if let Ok(content) = std::fs::read_to_string(&claude_md) {
            let suggestions = lint::suggest::suggest_improvements(&stack, &content);
            let sug_json: Vec<serde_json::Value> = suggestions
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "category": s.category,
                        "rule": s.rule,
                        "rationale": s.rationale,
                    })
                })
                .collect();
            result["suggestions"] = serde_json::json!(sug_json);
        }
    }

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) fn print_validation_warnings(warnings: &[crate::config::shareable::PresetWarning]) {
    use crate::config::shareable::ValidationSeverity;

    for w in warnings {
        let label = match w.severity {
            ValidationSeverity::Error => fmt::red("error"),
            ValidationSeverity::Warning => fmt::yellow("warning"),
        };
        eprintln!("  {label} [{}]: {}", w.field, w.message);
    }
}

pub(crate) fn print_suggestions(suggestions: &[lint::suggest::Suggestion]) {
    if suggestions.is_empty() {
        println!(
            "{} No additional suggestions — CLAUDE.md covers the basics.",
            fmt::icon_pass()
        );
        return;
    }

    println!("\n{}\n", fmt::bold("Suggested improvements:"));
    for s in suggestions {
        println!("  {} {}", fmt::cyan(&format!("[{}]", s.category)), s.rule);
        println!("    {}", fmt::dim(&format!("why: {}", s.rationale)));
    }
    println!();
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;

mod cmd_doctor;
mod cmd_export;
mod cmd_guard;
mod cmd_import;
mod cmd_init_ci;
mod cmd_init_from_preset;
mod cmd_lint;
mod cmd_migrate;
mod cmd_validate_ci;
mod guard_decision;
mod parse_language_or_unknown;

pub use cmd_doctor::cmd_doctor;
pub use cmd_export::cmd_export;
pub use cmd_guard::cmd_guard;
pub use cmd_import::cmd_import;
pub use cmd_init_ci::cmd_init_ci;
pub use cmd_init_from_preset::cmd_init_from_preset;
pub use cmd_lint::cmd_lint;
pub use cmd_migrate::cmd_migrate;
pub use cmd_validate_ci::cmd_validate_ci;
pub use parse_language_or_unknown::parse_language_or_unknown;
