//! One reasoning step as the Trace tab holds it.

use magistr_engine::native::{DepEdge, NextAction, ThinkStep};

/// One think step as the Trace tab shows it, read off the store's own
/// [`ThinkStep`] record — from a snapshot or a `step_appended` event:
/// an absent text is empty, an absent flag is off.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TraceStep {
    /// The step's project-global number.
    pub number: u32,
    /// How many steps the trace expected when this one landed.
    pub estimated_total: u32,
    /// What the step is for (`hypothesis`, `analysis`, …).
    pub purpose: String,
    /// What the step was looking at.
    pub context: String,
    /// The reasoning itself.
    pub thought: String,
    /// What came out of it.
    pub outcome: String,
    /// What the step said would happen next.
    pub next_action: String,
    /// Why that action.
    pub rationale: String,
    /// The step's confidence, when it gave one.
    pub confidence: Option<f64>,
    /// What the step was unsure about.
    pub uncertainty: String,
    /// The earlier step this one revises, when it does.
    pub revises: Option<u32>,
    /// The later step that revised this one, once one did.
    pub revised_by: Option<u32>,
    /// Whether the step closed its line of reasoning.
    pub is_final: bool,
    /// The branch the step sits on; the main line carries none.
    pub branch_id: Option<String>,
    /// The branch's human name, when it has one.
    pub branch_name: Option<String>,
    /// The main-line step the branch forked from.
    pub branch_from: Option<u32>,
    /// Steps this one leans on.
    pub dependencies: Vec<u32>,
    /// The `ship_*` entity the step points at (`task:…`, `check:…`).
    pub execution_ref: Option<String>,
    /// Whether the step is pinned.
    pub pinned: bool,
}

impl TraceStep {
    /// The row for the store's record.
    #[must_use]
    pub fn from_think(step: &ThinkStep) -> Self {
        Self {
            number: step.step_number,
            estimated_total: step.estimated_total,
            purpose: step.purpose.clone(),
            context: step.context.clone(),
            thought: step.thought.clone(),
            outcome: step.outcome.clone(),
            next_action: next_action(&step.next_action),
            rationale: step.rationale.clone(),
            confidence: step.confidence,
            uncertainty: step.uncertainty_notes.clone().unwrap_or_default(),
            revises: step.revises_step,
            revised_by: step.revised_by,
            is_final: step.is_final_step.unwrap_or(false),
            branch_id: step.branch_id.clone(),
            branch_name: step.branch_name.clone(),
            branch_from: step.branch_from,
            dependencies: step
                .dependencies
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|edge| match edge {
                    DepEdge::Bare(n) | DepEdge::Tagged { step: n, .. } => *n,
                })
                .collect(),
            execution_ref: step.execution_ref.clone(),
            pinned: step.pinned.unwrap_or(false),
        }
    }

    /// The labelled fields an expanded step shows, in display order,
    /// with the empty ones dropped. The view paints one row per line of
    /// each value; [`Trace::body_rows`](super::Trace::body_rows) counts
    /// those same lines, so the scroll and the paint agree by
    /// construction.
    #[must_use]
    pub fn fields(&self) -> Vec<(&'static str, String)> {
        let mut fields: Vec<(&'static str, String)> = Vec::new();
        let mut push = |label: &'static str, value: String| {
            if !value.is_empty() {
                fields.push((label, value));
            }
        };
        push("purpose", self.purpose.clone());
        push("context", self.context.clone());
        push("thought", self.thought.clone());
        push("outcome", self.outcome.clone());
        push("next action", self.next_action.clone());
        push("rationale", self.rationale.clone());
        push(
            "confidence",
            self.confidence
                .map(|confidence| format!("{confidence:.2}"))
                .unwrap_or_default(),
        );
        push("uncertainty", self.uncertainty.clone());
        push(
            "estimated total",
            if self.estimated_total == 0 {
                String::new()
            } else {
                self.estimated_total.to_string()
            },
        );
        push(
            "depends on",
            self.dependencies
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        );
        push(
            "execution ref",
            self.execution_ref.clone().unwrap_or_default(),
        );
        if let Some(branch) = &self.branch_id {
            let name = self.branch_name.as_deref().unwrap_or(branch);
            let from = self
                .branch_from
                .map(|number| format!(" from {number}"))
                .unwrap_or_default();
            push("branch", format!("{name}{from}"));
        }
        push(
            "final",
            if self.is_final {
                "yes".to_owned()
            } else {
                String::new()
            },
        );
        fields
    }

    /// Rows the fields take: one per line of every non-empty value.
    #[must_use]
    pub fn field_rows(&self) -> usize {
        self.fields()
            .iter()
            .map(|(_, value)| value.lines().count())
            .sum()
    }
}

/// The `next_action` field: a plain string, or a structured action whose
/// `action` speaks for it.
fn next_action(value: &NextAction) -> String {
    match value {
        NextAction::Text(text) => text.clone(),
        NextAction::Structured(action) => action.action.clone(),
    }
}

#[cfg(test)]
#[path = "trace_step.tests.rs"]
mod tests;
