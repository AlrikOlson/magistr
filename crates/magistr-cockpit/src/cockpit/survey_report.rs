//! What the survey found and wrote.

use super::scaffold_kind::ScaffoldKind;
use super::survey_kind::SurveyKind;

/// The survey's answer (Birth F2), gathered off the loop thread by the
/// entry point that owns the ground — `magistr init`'s real autodetect
/// and the scaffold it writes — and delivered as one
/// [`JobOutput::Survey`](crate::job::JobOutput::Survey). Every row is a
/// real detection or a real file; the cockpit lands them one by one as
/// the sweep passes, and never invents one.
///
/// The chips the interview pre-offers come from here too: what the
/// code scan suggests the project is, and what it suggests must never
/// break. The reader's own words always win over them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SurveyReport {
    /// The project's name as the ground names it — its config's, or
    /// the folder's.
    pub name: String,
    /// The facts found, in the order they were found: the kind and the
    /// row's copy (`Rust · edition 2024 · cargo workspace, 4 crates`).
    pub detected: Vec<(SurveyKind, String)>,
    /// The receipts, in the order they were written: the kind, the
    /// value (`.magistr/project.json`) and whether it is there.
    pub written: Vec<(ScaffoldKind, String, bool)>,
    /// Faint points the sweep passes over — code the survey noticed
    /// that will inform the charting, not yet stars — as thousandths
    /// of the field's width and height (integers, so a report is
    /// `Eq` like every other job output).
    pub points: Vec<(u16, u16)>,
    /// What the scan suggests the project should become — question
    /// one's chips.
    pub becomes: Vec<String>,
    /// What the scan suggests must never break — question two's
    /// chips, each a gate or a protected path once picked.
    pub never_break: Vec<String>,
}

impl SurveyReport {
    /// A report on the project called `name`, with nothing found yet.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// How many rows the sweep lands in all — every fact and every
    /// receipt.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.detected.len() + self.written.len()
    }
}

#[cfg(test)]
#[path = "survey_report.tests.rs"]
mod tests;
