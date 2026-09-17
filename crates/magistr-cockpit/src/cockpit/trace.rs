//! The reasoning model's state.

use std::collections::BTreeMap;

#[path = "investigation.rs"]
mod investigation;
pub use investigation::Investigation;

use magistr_workflow_contracts::{BranchStatus, ThinkBranch};
use magistr_workflow_contracts::{NativeEvent, NativeSnapshot, ShipFrame, ThinkFrame};

use super::ship_moment::ShipMoment;
use super::trace_card::TraceCardRow;
use super::trace_focus::TraceFocus;
use super::trace_step::TraceStep;

/// Rows the step graph takes above the list once the pixel layer
/// covers it: room for the main line and up to three branch lanes.
pub const TRACE_GRAPH_ROWS: usize = 4;

/// What the reasoning model shows: the reasoning steps as the feed delivered
/// them (in arrival order), one of them selected with its detail card
/// open, the branches' statuses, the recorded work — the ship timeline
/// of actions and checks — and the checkpoint report pane while one is
/// open.
///
/// Pure state: typed events arrive through [`Trace::apply`], the store's
/// snapshot through [`Trace::seed`], the port's answers through
/// [`Trace::set_pinned`] and [`Trace::open_viewer`]; nothing here talks
/// to an engine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trace {
    steps: Vec<TraceStep>,
    branches: BTreeMap<String, ThinkBranch>,
    /// The selected step's number — the one whose card is open.
    selected: Option<u32>,
    /// Whether the card shows every field or the brief cut.
    full: bool,
    moments: Vec<ShipMoment>,
    viewer: Option<Vec<String>>,
}

impl Trace {
    /// Fold one typed event in: a think step appended lands (or replaces
    /// a re-recorded number), a revision and a pin mark their step, a
    /// branch status change keeps the branch's status, a think clear
    /// empties the think half; a ship action or check joins the
    /// timeline, a ship clear empties it. Every other event is ignored.
    pub fn apply(&mut self, event: &NativeEvent) {
        match event {
            NativeEvent::Think(frame) => self.apply_think(frame),
            NativeEvent::Ship(frame) => self.apply_ship(frame),
            NativeEvent::Roadmap(_) | NativeEvent::Signal(_) | NativeEvent::Magistr(_) => {}
        }
    }

    fn apply_think(&mut self, frame: &ThinkFrame) {
        match frame {
            ThinkFrame::StepAppended { step, .. } => self.land(TraceStep::from_think(step)),
            ThinkFrame::StepRevised {
                revised_step,
                by_step,
                ..
            } => self.mark_revised(*revised_step, *by_step),
            ThinkFrame::PinChanged {
                step_number,
                pinned,
                ..
            } => self.set_pinned(*step_number, *pinned),
            ThinkFrame::BranchStatusChanged {
                branch_id,
                status,
                merged_into,
                ..
            } => {
                let branch =
                    self.branches
                        .entry(branch_id.clone())
                        .or_insert_with(|| ThinkBranch {
                            id: branch_id.clone(),
                            name: String::new(),
                            from_step: None,
                            status: *status,
                            merged_into: None,
                        });
                branch.status = *status;
                branch.merged_into = if *status == BranchStatus::Merged {
                    *merged_into
                } else {
                    None
                };
            }
            ThinkFrame::Cleared => {
                self.steps.clear();
                self.branches.clear();
                self.selected = None;
            }
            ThinkFrame::EstimateRevised { .. } => {}
        }
    }

    fn apply_ship(&mut self, frame: &ShipFrame) {
        match frame {
            ShipFrame::ActionRecorded { task_id, action } => {
                self.moments.push(ShipMoment::action(task_id, action));
            }
            ShipFrame::CheckRecorded { task_id, check } => {
                self.moments.push(ShipMoment::check(task_id, check))
            }
            ShipFrame::Cleared => self.moments.clear(),
            ShipFrame::ObjectiveSet { .. }
            | ShipFrame::TaskAdded { .. }
            | ShipFrame::TaskStarted { .. }
            | ShipFrame::TaskCompleted { .. }
            | ShipFrame::TaskBlocked { .. }
            | ShipFrame::ObjectiveShipped { .. } => {}
        }
    }

    /// A step landed: a re-recorded number replaces, a new one appends,
    /// and the step it revises is marked.
    fn land(&mut self, step: TraceStep) {
        if let Some(revised) = step.revises {
            self.mark_revised(revised, step.number);
        }
        match self.steps.iter_mut().find(|s| s.number == step.number) {
            Some(existing) => *existing = step,
            None => self.steps.push(step),
        }
    }

    /// Take the store's trace as the tab's: the persisted steps in stored
    /// order, then every action and check on the snapshot's tasks in the
    /// order the engines stamped them.
    ///
    /// Replaces both halves rather than appending, so a re-seed mid-run
    /// never doubles a moment the feed already delivered. Persisted branch
    /// metadata restores lifecycle and merge destinations after a restart.
    /// Legacy snapshots without it retain what the feed knows. Selection and
    /// an open checkpoint pane survive either kind of snapshot.
    pub fn seed(&mut self, snapshot: &NativeSnapshot) {
        self.steps.clear();
        self.moments.clear();
        for branch in &snapshot.branches {
            self.branches.insert(branch.id.clone(), branch.clone());
        }
        for step in &snapshot.steps {
            self.land(TraceStep::from_think(step));
        }
        // One offset on every stamp, so the strings sort chronologically.
        let mut moments: Vec<(&str, ShipMoment)> = Vec::new();
        for task in &snapshot.tasks {
            for action in &task.actions {
                moments.push((
                    action.timestamp.as_str(),
                    ShipMoment::action(&task.id, action),
                ));
            }
            for check in &task.checks {
                moments.push((check.timestamp.as_str(), ShipMoment::check(&task.id, check)));
            }
        }
        moments.sort_by_key(|(timestamp, _)| *timestamp);
        self.moments
            .extend(moments.into_iter().map(|(_, moment)| moment));
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

    /// A click on the step at feed `index`: it becomes the selection
    /// and its card opens, or the selection folds when it already was —
    /// one step selected at a time.
    pub fn toggle(&mut self, index: usize) {
        let Some(step) = self.steps.get(index) else {
            return;
        };
        self.selected = if self.selected == Some(step.number) {
            None
        } else {
            Some(step.number)
        };
    }

    /// Select the step at feed `index`; whether the feed has one.
    pub fn select_index(&mut self, index: usize) -> bool {
        match self.steps.get(index) {
            Some(step) => {
                self.selected = Some(step.number);
                true
            }
            None => false,
        }
    }

    /// A revision link was followed, or a search landed: the step it
    /// names becomes the selection, when the feed has it; whether it did.
    pub fn expand_step(&mut self, number: u32) -> bool {
        if self.step(number).is_some() {
            self.selected = Some(number);
            return true;
        }
        false
    }

    /// Move the selection one step down the feed (`forward`) or up it —
    /// from the newest step when nothing is selected, stopping at either
    /// end; whether it moved.
    pub fn select_step(&mut self, forward: bool) -> bool {
        if self.steps.is_empty() {
            return false;
        }
        let at = match self.selected_index() {
            Some(at) if forward => at.saturating_add(1),
            Some(at) => at.saturating_sub(1),
            None => self.steps.len() - 1,
        };
        let at = at.min(self.steps.len() - 1);
        let moved = self.selected_index() != Some(at);
        self.selected = Some(self.steps[at].number);
        moved
    }

    /// Fold the card; whether one was open.
    pub fn deselect(&mut self) -> bool {
        self.selected.take().is_some()
    }

    /// The selected step's feed index.
    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        let number = self.selected?;
        self.steps.iter().position(|step| step.number == number)
    }

    /// The selected step.
    #[must_use]
    pub fn selected(&self) -> Option<&TraceStep> {
        self.selected_index().map(|at| &self.steps[at])
    }

    /// Whether the step is the selection, with its card open.
    #[must_use]
    pub fn is_expanded(&self, number: u32) -> bool {
        self.selected == Some(number)
    }

    /// Whether the card shows every field rather than the brief cut.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.full
    }

    /// `[More]` / `[Less]` on the card, or Enter on the selection.
    pub fn toggle_detail(&mut self) {
        self.full = !self.full;
    }

    /// The selection's neighbourhood — what the graph lights and the
    /// list marks — or `None` while nothing is selected.
    #[must_use]
    pub fn focus(&self) -> Option<TraceFocus> {
        self.selected_index()
            .map(|selected| TraceFocus::of(&self.steps, selected))
    }

    /// The engine pinned or unpinned the step — the `pin_changed` frame,
    /// or the port's answer to \[Pin\].
    pub fn set_pinned(&mut self, number: u32, pinned: bool) {
        if let Some(step) = self.steps.iter_mut().find(|step| step.number == number) {
            step.pinned = pinned;
        }
    }

    /// Per step in feed order, whether it is pinned.
    #[must_use]
    pub fn pins(&self) -> Vec<bool> {
        self.steps.iter().map(|step| step.pinned).collect()
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

    /// Questions explored, their exact member decisions, outcomes and recorded links.
    #[must_use]
    pub fn investigations(&self) -> Vec<Investigation<'_>> {
        Investigation::all(self)
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
            if let Some(from) = self
                .steps
                .iter()
                .filter(|candidate| candidate.branch_id.as_deref() == Some(branch))
                .find_map(|candidate| candidate.branch_from)
                .or_else(|| self.branch(branch).and_then(|branch| branch.from_step))
                .and_then(|number| self.steps.iter().position(|s| s.number == number))
            {
                links.push((from, index));
            }
        }
        links
    }

    /// Merge edges use the recorded synthesis destination when available.
    /// Older records with only a merged status retain the first later main-line
    /// inference. An explicit target missing from this trace creates no substitute edge.
    #[must_use]
    pub fn merges(&self) -> Vec<(usize, usize)> {
        let mut merges = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        for step in &self.steps {
            let Some(branch) = step.branch_id.as_deref() else {
                continue;
            };
            if seen.contains(&branch) || self.branch_status(branch) != "merged" {
                continue;
            }
            seen.push(branch);
            let last = self
                .steps
                .iter()
                .rposition(|s| s.branch_id.as_deref() == Some(branch));
            let Some(last) = last else {
                continue;
            };
            if let Some(number) = self.branch_merge(branch) {
                if let Some(next) = self.steps.iter().position(|step| step.number == number) {
                    merges.push((last, next));
                }
            } else if let Some(next) = self.steps[last + 1..]
                .iter()
                .position(|s| s.branch_id.is_none())
            {
                merges.push((last, last + 1 + next));
            }
        }
        merges
    }

    /// The lanes as an indented list — the main line first, then each
    /// branch with its name, status and fork point — what the stage
    /// shows where the pixel graph would sit.
    #[must_use]
    pub fn branch_rows(&self) -> Vec<String> {
        let main = self
            .steps
            .iter()
            .filter(|step| step.branch_id.is_none())
            .count();
        let mut rows = vec![format!("Main reasoning · {main} steps")];
        for question in self.investigations() {
            let name = question.question;
            let count = question.members.len();
            let status = question.status_label();
            let from = question
                .from
                .map_or_else(String::new, |n| format!(" after {n}"));
            rows.push(format!("└─ {name} ({status}){from} · {count} steps"));
        }
        rows
    }

    /// A branch's status from persisted metadata or a live status event.
    /// Older steps without either remain active.
    #[must_use]
    pub fn branch_status(&self, branch: &str) -> &str {
        self.branches
            .get(branch)
            .map_or("active", |branch| branch_status_name(branch.status))
    }

    /// Recorded branch metadata, when the snapshot or live feed supplies it.
    #[must_use]
    pub fn branch(&self, id: &str) -> Option<&ThinkBranch> {
        self.branches.get(id)
    }

    /// The exact recorded merge destination; never the positional fallback.
    #[must_use]
    pub fn branch_merge(&self, id: &str) -> Option<u32> {
        self.branch(id)
            .filter(|branch| branch.status == BranchStatus::Merged)
            .and_then(|branch| branch.merged_into)
    }

    /// Whether any step sits on a branch.
    #[must_use]
    pub fn has_branches(&self) -> bool {
        self.steps.iter().any(|step| step.branch_id.is_some())
    }

    /// How many branches the feed holds.
    #[must_use]
    pub fn branch_count(&self) -> usize {
        self.branch_rows().len() - 1
    }

    /// How many steps are pinned.
    #[must_use]
    pub fn pinned_count(&self) -> usize {
        self.steps.iter().filter(|step| step.pinned).count()
    }

    /// The heading's accounting: `10 steps · 1 branch · 2 pinned`, with
    /// the parts that are zero left out.
    #[must_use]
    pub fn heading_line(&self) -> String {
        let plural = |n: usize, one: &str, many: &str| {
            if n == 1 {
                format!("{n} {one}")
            } else {
                format!("{n} {many}")
            }
        };
        let mut parts = vec![plural(self.steps.len(), "step", "steps")];
        let branches = self.branch_count();
        if branches > 0 {
            parts.push(plural(branches, "branch", "branches"));
        }
        let pinned = self.pinned_count();
        if pinned > 0 {
            parts.push(format!("{pinned} pinned"));
        }
        parts.join(" · ")
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
    /// its checkpoint button, or the checkpoint pane's own line) and the
    /// graph block — [`TRACE_GRAPH_ROWS`] of pixels whenever there are
    /// steps and the pixel layer is live, else the lane list once a
    /// branch forked. The block carries the pixel surface's anchor, so
    /// it must stay whole.
    #[must_use]
    pub fn pinned_rows(&self, pixels: bool) -> usize {
        let graph = if pixels && !self.steps.is_empty() {
            TRACE_GRAPH_ROWS
        } else if self.has_branches() {
            self.branch_rows().len()
        } else {
            0
        };
        1 + graph
    }

    /// The selected step's card, wrapped to `width` columns, with the
    /// execution ref linked when `jump`; empty while nothing is selected.
    #[must_use]
    pub fn card_rows(&self, width: u16, jump: bool) -> Vec<TraceCardRow> {
        self.selected()
            .map(|step| TraceCardRow::rows(step, width, self.full, jump))
            .unwrap_or_default()
    }

    /// Rows the scrolling body takes at `width` columns: the checkpoint
    /// report's lines while the pane is open, else one per step plus the
    /// selected step's card, plus the recorded-work heading and its
    /// moments. Counted from the same [`TraceCardRow::rows`] the view
    /// paints, so the window and the rows never drift.
    #[must_use]
    pub fn body_rows(&self, _width: u16) -> usize {
        if let Some(lines) = &self.viewer {
            return lines.len();
        }
        if self.is_empty() {
            // The "no trace yet" note.
            return 1;
        }
        let steps = self.steps.len();
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

/// The branch status in the store's spelling — the word the lane list
/// prints after the branch's name.
fn branch_status_name(status: magistr_workflow_contracts::BranchStatus) -> &'static str {
    use magistr_workflow_contracts::BranchStatus;
    match status {
        BranchStatus::Active => "active",
        BranchStatus::Merged => "merged",
        BranchStatus::Abandoned => "abandoned",
    }
}

#[cfg(test)]
#[path = "trace.tests.rs"]
mod tests;
