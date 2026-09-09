//! The Trace tab's state.

use std::collections::BTreeMap;

use magistr_engine::NativeFrame;

use super::ship_moment::ShipMoment;
use super::trace_step::TraceStep;

/// What the Trace tab shows: the think trace as the feed delivered it
/// (steps in arrival order, one expanded at a time), the branches'
/// statuses, the ship timeline of actions and checks, and the
/// checkpoint report pane while one is open.
///
/// Pure state: broadcast frames arrive through [`Trace::apply`], the
/// port's answers through [`Trace::set_pinned`] and
/// [`Trace::open_viewer`]; nothing here talks to an engine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trace {
    steps: Vec<TraceStep>,
    branch_status: BTreeMap<String, String>,
    expanded: Option<u32>,
    moments: Vec<ShipMoment>,
    viewer: Option<Vec<String>>,
}

impl Trace {
    /// Fold one broadcast frame in: a think `step_appended` lands (or
    /// replaces a re-recorded number), `step_revised` and `pin_changed`
    /// mark their step, `branch_status_changed` keeps the branch's
    /// status, `cleared` empties the think half; a ship
    /// `action_recorded` or `check_recorded` joins the timeline,
    /// ship `cleared` empties it. Every other frame is ignored.
    pub fn apply(&mut self, frame: &NativeFrame) {
        let payload = &frame.payload;
        match (frame.family.as_str(), frame.kind.as_str()) {
            ("think", "step_appended") => {
                let Some(step) = TraceStep::from_step(&payload["step"]) else {
                    return;
                };
                if let Some(revised) = step.revises {
                    self.mark_revised(revised, step.number);
                }
                match self.steps.iter_mut().find(|s| s.number == step.number) {
                    Some(existing) => *existing = step,
                    None => self.steps.push(step),
                }
            }
            ("think", "step_revised") => {
                if let (Some(revised), Some(by)) = (
                    step_number(&payload["revised_step"]),
                    step_number(&payload["by_step"]),
                ) {
                    self.mark_revised(revised, by);
                }
            }
            ("think", "pin_changed") => {
                if let Some(number) = step_number(&payload["step_number"]) {
                    self.set_pinned(number, payload["pinned"].as_bool().unwrap_or(false));
                }
            }
            ("think", "branch_status_changed") => {
                if let (Some(branch), Some(status)) =
                    (payload["branch_id"].as_str(), payload["status"].as_str())
                {
                    self.branch_status
                        .insert(branch.to_owned(), status.to_owned());
                }
            }
            ("think", "cleared") => {
                self.steps.clear();
                self.branch_status.clear();
                self.expanded = None;
            }
            ("ship", "action_recorded") => self.moments.push(ShipMoment::from_action(payload)),
            ("ship", "check_recorded") => self.moments.push(ShipMoment::from_check(payload)),
            ("ship", "cleared") => self.moments.clear(),
            _ => {}
        }
    }

    /// Take the store's trace as the tab's: the steps and the ship
    /// timeline the entry point (or the loop driver, at each iteration)
    /// read off the engines, as
    /// [`StoreSeed::trace`](crate::engine::StoreSeed::trace) carries them.
    ///
    /// Replaces both halves rather than appending, so a re-seed mid-run
    /// never doubles a moment the feed already delivered — the store is
    /// the superset. What the feed alone knows survives: the branch
    /// statuses, which step is expanded, and an open checkpoint pane.
    pub fn seed(&mut self, frames: &[NativeFrame]) {
        self.steps.clear();
        self.moments.clear();
        for frame in frames {
            self.apply(frame);
        }
    }

    /// The steps, in arrival order.
    #[must_use]
    pub fn steps(&self) -> &[TraceStep] {
        &self.steps
    }

    /// The step numbered `number`, when the feed delivered it.
    #[must_use]
    pub fn step(&self, number: u32) -> Option<&TraceStep> {
        self.steps.iter().find(|step| step.number == number)
    }

    /// A click on the step at feed `index`: it expands to every field,
    /// or folds when it already shows — one step expands at a time.
    pub fn toggle(&mut self, index: usize) {
        let Some(step) = self.steps.get(index) else {
            return;
        };
        self.expanded = if self.expanded == Some(step.number) {
            None
        } else {
            Some(step.number)
        };
    }

    /// A revision link was followed: the step it names expands, when the
    /// feed has it; whether it did.
    pub fn expand_step(&mut self, number: u32) -> bool {
        if self.step(number).is_some() {
            self.expanded = Some(number);
            return true;
        }
        false
    }

    /// Whether the step shows every field.
    #[must_use]
    pub fn is_expanded(&self, number: u32) -> bool {
        self.expanded == Some(number)
    }

    /// The engine pinned or unpinned the step — the `pin_changed` frame,
    /// or the port's answer to \[Pin\].
    pub fn set_pinned(&mut self, number: u32, pinned: bool) {
        if let Some(step) = self.steps.iter_mut().find(|step| step.number == number) {
            step.pinned = pinned;
        }
    }

    /// The ship timeline, oldest first.
    #[must_use]
    pub fn moments(&self) -> &[ShipMoment] {
        &self.moments
    }

    /// Every step's lane, in feed order: the main line is lane 0, each
    /// branch takes the next lane in order of first appearance.
    #[must_use]
    pub fn lanes(&self) -> Vec<(u32, u8)> {
        let mut order: Vec<&str> = Vec::new();
        self.steps
            .iter()
            .map(|step| {
                let lane = match step.branch_id.as_deref() {
                    None => 0,
                    Some(branch) => {
                        let at = order.iter().position(|known| *known == branch);
                        let at = at.unwrap_or_else(|| {
                            order.push(branch);
                            order.len() - 1
                        });
                        u8::try_from(at + 1).unwrap_or(u8::MAX)
                    }
                };
                (step.number, lane)
            })
            .collect()
    }

    /// The fork edges, as feed-index pairs: each branch's first step
    /// linked from the step it forked off, when the feed has both.
    #[must_use]
    pub fn links(&self) -> Vec<(usize, usize)> {
        let mut seen: Vec<&str> = Vec::new();
        let mut links = Vec::new();
        for (index, step) in self.steps.iter().enumerate() {
            let Some(branch) = step.branch_id.as_deref() else {
                continue;
            };
            if seen.contains(&branch) {
                continue;
            }
            seen.push(branch);
            if let Some(from) = step
                .branch_from
                .and_then(|number| self.steps.iter().position(|s| s.number == number))
            {
                links.push((from, index));
            }
        }
        links
    }

    /// The branches as an indented list — the main line first, then each
    /// branch with its name, status and fork point — what the stage
    /// shows where the pixel graph would sit.
    #[must_use]
    pub fn branch_rows(&self) -> Vec<String> {
        let main = self
            .steps
            .iter()
            .filter(|step| step.branch_id.is_none())
            .count();
        let mut rows = vec![format!("main · {main} steps")];
        let mut seen: Vec<&str> = Vec::new();
        for step in &self.steps {
            let Some(branch) = step.branch_id.as_deref() else {
                continue;
            };
            if seen.contains(&branch) {
                continue;
            }
            seen.push(branch);
            let count = self
                .steps
                .iter()
                .filter(|s| s.branch_id.as_deref() == Some(branch))
                .count();
            let name = step.branch_name.as_deref().unwrap_or(branch);
            let status = self.branch_status(branch);
            let from = step
                .branch_from
                .map_or_else(String::new, |n| format!(" from {n}"));
            rows.push(format!("└─ {name} ({status}){from} · {count} steps"));
        }
        rows
    }

    /// A branch's status by id — `active` until a `branch_status_changed`
    /// frame says otherwise (`kept`, `abandoned`, …). The same word
    /// [`Trace::branch_rows`] prints, readable on its own.
    #[must_use]
    pub fn branch_status(&self, branch: &str) -> &str {
        self.branch_status
            .get(branch)
            .map_or("active", String::as_str)
    }

    /// Whether any step sits on a branch — whether a graph is worth
    /// drawing at all.
    #[must_use]
    pub fn has_branches(&self) -> bool {
        self.steps.iter().any(|step| step.branch_id.is_some())
    }

    /// Open the checkpoint pane on the report's lines.
    pub fn open_viewer(&mut self, lines: Vec<String>) {
        self.viewer = Some(lines);
    }

    /// The checkpoint pane's lines, while one is open.
    #[must_use]
    pub fn viewer(&self) -> Option<&[String]> {
        self.viewer.as_deref()
    }

    /// Close the checkpoint pane; whether one was open.
    pub fn close_viewer(&mut self) -> bool {
        self.viewer.take().is_some()
    }

    /// Rows the tab keeps above the scrolling body: the heading (with
    /// its \[Checkpoint\] button, or the checkpoint pane's own line) and,
    /// when a branch forked, the graph block — that one carries the
    /// pixel surface's anchor, so it must stay whole.
    #[must_use]
    pub fn pinned_rows(&self) -> usize {
        if self.viewer.is_some() {
            return 1;
        }
        let graph = if self.has_branches() {
            self.branch_rows().len()
        } else {
            0
        };
        1 + graph
    }

    /// Rows the scrolling body takes: the checkpoint report's lines
    /// while the pane is open, else one per step plus the expanded
    /// step's button row and fields, plus the ship heading and its
    /// moments. Counted from the same
    /// [`TraceStep::fields`](super::TraceStep::fields) the view paints,
    /// so the window and the rows never drift.
    #[must_use]
    pub fn body_rows(&self) -> usize {
        if let Some(lines) = &self.viewer {
            return lines.len();
        }
        if self.is_empty() {
            // The "no trace yet" note.
            return 1;
        }
        let steps: usize = self
            .steps
            .iter()
            .map(|step| {
                if self.is_expanded(step.number) {
                    1 + 1 + step.field_rows()
                } else {
                    1
                }
            })
            .sum();
        let ship = if self.moments.is_empty() {
            0
        } else {
            1 + self.moments.len()
        };
        steps + ship
    }

    /// Whether the tab has nothing to show yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty() && self.moments.is_empty() && self.viewer.is_none()
    }

    fn mark_revised(&mut self, revised: u32, by: u32) {
        if let Some(step) = self.steps.iter_mut().find(|step| step.number == revised) {
            step.revised_by = Some(by);
        }
    }
}

fn step_number(value: &serde_json::Value) -> Option<u32> {
    value.as_u64().and_then(|n| u32::try_from(n).ok())
}

#[cfg(test)]
#[path = "trace.tests.rs"]
mod tests;
