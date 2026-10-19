//! CLAUDE.md intelligence — lint, doctor, and stack-based suggestions.
//!
//! Provides three capabilities:
//! - **Lint**: Validate CLAUDE.md against known anti-patterns
//! - **Doctor**: Diagnose why Claude might be ignoring your rules
//! - **Suggest**: Recommend improvements based on detected stack

pub mod check_status;
pub mod diagnostic;
pub mod doctor;
pub mod doctor_check;
pub mod report;
pub mod rules;
pub mod severity;
pub mod suggest;
pub mod suggestion;

use std::path::Path;

pub use check_status::CheckStatus;
pub use diagnostic::LintDiagnostic;
pub use doctor_check::DoctorCheck;
pub use report::LintReport;
pub use severity::Severity;

/// Lint a CLAUDE.md file at the given path.
///
/// Reads the file and runs all lint rules against its content.
/// Returns diagnostics sorted by line number.
pub fn lint_file(path: &Path) -> crate::error::Result<LintReport> {
    let content =
        std::fs::read_to_string(path).map_err(|e| crate::error::MagistrError::io(path, e))?;
    let mut diagnostics = rules::run_all(&content);
    diagnostics.sort_by_key(|d| d.line.unwrap_or(0));

    Ok(LintReport {
        path: path.display().to_string(),
        diagnostics,
    })
}

/// Run doctor checks on a project directory.
///
/// Examines CLAUDE.md, .claude/rules/, settings, and overall project health.
pub fn doctor(dir: &Path, claude_selected: bool) -> Vec<DoctorCheck> {
    doctor::run_all(dir, claude_selected)
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
