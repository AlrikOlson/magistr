//! The rows a selected reasoning step's card takes.

use super::trace_step::TraceStep;
use crate::layout::TextMetrics;

/// One row of the detail card the Trace tab opens under the selected
/// step — the same list the view paints and the scroll counts, so the
/// two cannot drift.
///
/// A card is a blank row of breathing room, a title, the step's
/// metadata on as many rows as the width needs, a button row, then its
/// text fields wrapped to the width — the *brief* card shows what the
/// step thought, concluded and would do next, each cut to
/// [`TraceCardRow::BRIEF_LINES`]; the *full* card shows every field
/// whole — and a closing blank row. Every row is one cell high.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceCardRow {
    /// Breathing room above and below the card.
    Blank,
    /// `step 10 · Verify restart-safe scheduling`.
    Title(String),
    /// One wrapped row of the metadata line.
    Meta(String),
    /// The card's verbs: pin, the chunk jump, the revision links, and
    /// more or less.
    Buttons,
    /// One wrapped row of a text field; `first` carries the label,
    /// `linked` marks the execution ref that resolves to a chunk.
    Field {
        /// The field's label, on its first row.
        label: &'static str,
        /// This row's text.
        text: String,
        /// Whether this is the field's first row.
        first: bool,
        /// Whether the row is the chunk link.
        linked: bool,
    },
}

impl TraceCardRow {
    /// Columns the card's left accent and gutter take.
    pub const INDENT: u16 = 4;
    /// Columns the label column takes, so values align.
    pub const LABEL_COLS: u16 = 13;
    /// Rows a brief field shows before it is cut.
    pub const BRIEF_LINES: usize = 2;
    /// The narrowest a value column gets, however narrow the stage.
    const MIN_VALUE_COLS: u16 = 12;
    /// The label of the execution-ref row — the one the chunk jump rides.
    pub const EXECUTION_REF: &'static str = "execution ref";

    /// The card for `step` at `width` columns: every field when `full`,
    /// the brief cut otherwise; `jump` marks the execution ref as a
    /// chunk link.
    #[must_use]
    pub fn rows(step: &TraceStep, width: u16, full: bool, jump: bool) -> Vec<Self> {
        let value_cols = width
            .saturating_sub(Self::INDENT + Self::LABEL_COLS)
            .max(Self::MIN_VALUE_COLS);
        let meta_cols = width.saturating_sub(Self::INDENT).max(Self::MIN_VALUE_COLS);
        let mut rows = vec![Self::Blank, Self::Title(title(step))];
        rows.extend(wrapped(&meta(step), meta_cols).into_iter().map(Self::Meta));
        rows.push(Self::Buttons);
        let fields: Vec<(&'static str, &str, bool)> = if full {
            vec![
                ("context", step.context.as_str(), false),
                ("thought", step.thought.as_str(), false),
                ("outcome", step.outcome.as_str(), false),
                ("next action", step.next_action.as_str(), false),
                ("rationale", step.rationale.as_str(), false),
                ("uncertainty", step.uncertainty.as_str(), false),
                (
                    Self::EXECUTION_REF,
                    step.execution_ref.as_deref().unwrap_or(""),
                    jump,
                ),
            ]
        } else {
            vec![
                ("thought", step.thought.as_str(), false),
                ("outcome", step.outcome.as_str(), false),
                ("next action", step.next_action.as_str(), false),
            ]
        };
        for (label, text, linked) in fields {
            if text.trim().is_empty() {
                continue;
            }
            let mut lines = wrapped(text, value_cols);
            if !full && lines.len() > Self::BRIEF_LINES {
                lines.truncate(Self::BRIEF_LINES);
                if let Some(last) = lines.last_mut() {
                    let cut = last
                        .chars()
                        .take(usize::from(value_cols).saturating_sub(2))
                        .collect::<String>();
                    *last = format!("{} …", cut.trim_end());
                }
            }
            for (index, line) in lines.into_iter().enumerate() {
                rows.push(Self::Field {
                    label,
                    text: line,
                    first: index == 0,
                    linked,
                });
            }
        }
        rows.push(Self::Blank);
        rows
    }
}

/// `step 10 · <purpose>`, or just the number when the step has none.
fn title(step: &TraceStep) -> String {
    if step.purpose.is_empty() {
        format!("step {}", step.number)
    } else {
        format!("step {} · {}", step.number, step.purpose)
    }
}

/// The metadata line: confidence, what it depends on, the estimate, its
/// branch, revisions, the pin and the final mark — only the parts the
/// step has.
fn meta(step: &TraceStep) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(confidence) = step.confidence {
        parts.push(format!("confidence {confidence:.2}"));
    }
    if !step.dependencies.is_empty() {
        parts.push(format!(
            "depends on {}",
            step.dependencies
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if step.estimated_total > 0 {
        parts.push(format!("estimate {}", step.estimated_total));
    }
    if let Some(branch) = &step.branch_id {
        let name = step.branch_name.as_deref().unwrap_or(branch);
        match step.branch_from {
            Some(from) => parts.push(format!("branch {name} from {from}")),
            None => parts.push(format!("branch {name}")),
        }
    }
    if let Some(revises) = step.revises {
        parts.push(format!("revises {revises}"));
    }
    if let Some(by) = step.revised_by {
        parts.push(format!("revised by {by}"));
    }
    if step.pinned {
        parts.push("pinned".to_owned());
    }
    if step.is_final {
        parts.push("final".to_owned());
    }
    if parts.is_empty() {
        "no metadata".to_owned()
    } else {
        parts.join(" · ")
    }
}

/// `text` word-wrapped to `columns`, one owned string per row.
fn wrapped(text: &str, columns: u16) -> Vec<String> {
    TextMetrics::new(text)
        .wrap(columns)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
#[path = "trace_card.tests.rs"]
mod tests;
