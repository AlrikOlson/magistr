//! One reasoning step as the think feed carries it.

/// One think step off the feed — every field a `step_appended` frame's
/// `step` carries that the Trace tab shows, parsed leniently: a missing
/// text is empty, a missing flag is off, only the step number is
/// required.
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
    /// The step behind a `step_appended` frame's `step` value; `None`
    /// without a step number.
    #[must_use]
    pub fn from_step(step: &serde_json::Value) -> Option<Self> {
        let number = u32::try_from(step["step_number"].as_u64()?).ok()?;
        let text = |key: &str| step[key].as_str().unwrap_or_default().to_owned();
        let some_text = |key: &str| step[key].as_str().map(str::to_owned);
        let step_ref = |key: &str| step[key].as_u64().and_then(|n| u32::try_from(n).ok());
        Some(Self {
            number,
            estimated_total: step_ref("estimated_total").unwrap_or(0),
            purpose: text("purpose"),
            context: text("context"),
            thought: text("thought"),
            outcome: text("outcome"),
            next_action: next_action(&step["next_action"]),
            rationale: text("rationale"),
            confidence: step["confidence"].as_f64(),
            uncertainty: text("uncertainty_notes"),
            revises: step_ref("revises_step"),
            revised_by: step_ref("revised_by"),
            is_final: step["is_final_step"].as_bool().unwrap_or(false),
            branch_id: some_text("branch_id"),
            branch_name: some_text("branch_name"),
            branch_from: step_ref("branch_from"),
            dependencies: dependencies(&step["dependencies"]),
            execution_ref: some_text("execution_ref"),
            pinned: step["pinned"].as_bool().unwrap_or(false),
        })
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
fn next_action(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Object(_) => value["action"].as_str().unwrap_or_default().to_owned(),
        _ => String::new(),
    }
}

/// Dependency edges: each entry a bare step number or `{step, relation}`.
fn dependencies(value: &serde_json::Value) -> Vec<u32> {
    value
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_u64().or_else(|| entry["step"].as_u64()))
                .filter_map(|n| u32::try_from(n).ok())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "trace_step.tests.rs"]
mod tests;
