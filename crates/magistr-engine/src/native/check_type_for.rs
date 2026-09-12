//! Which kind of check a gate name records as.

use magistr_workflow::ship::domain::check::CheckType;

/// The kind of check a gate name stands for. An archetype suffix
/// (`test:app-typescript`) does not change the kind, and a custom gate is
/// a lint — the one derivation every gate record and gate contract uses.
#[must_use]
pub fn check_type_for(gate: &str) -> CheckType {
    match gate.split(':').next().unwrap_or(gate) {
        "check" => CheckType::Typecheck,
        "test" => CheckType::Test,
        _ => CheckType::Lint,
    }
}
