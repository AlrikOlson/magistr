//! Which kind of check a check name records as.

use magistr_workflow::run::domain::check::CheckType;

/// The kind of check a check name stands for. An archetype suffix
/// (`test:app-typescript`) does not change the kind, and a custom check is
/// a lint — the one derivation every check record and check contract uses.
#[must_use]
pub fn check_type_for(check: &str) -> CheckType {
    match check.split(':').next().unwrap_or(check) {
        "check" => CheckType::Typecheck,
        "test" => CheckType::Test,
        _ => CheckType::Lint,
    }
}
