//! One object card, in one place, for every terminal view.

use super::marked::Marked;
use super::space::Space;
use super::{ChunkState, Model, Pane, Selected, Widget};
use crate::layout::{Frame, Length, Node, TextMetrics};
use crate::pixel::Icon;
use crate::render::Style;
use crate::theme::Pen;
use crate::turn::diff_kind;
use crate::widget_id::WidgetId;

#[path = "flight_inspector.rs"]
mod flight;

fn task_name(model: &Model, id: &str) -> String {
    model
        .rail()
        .ship()
        .task(id)
        .map_or_else(|| "Recorded work".to_owned(), |task| task.title.clone())
}

/// The card's status line for a chunk's state.
const fn chunk_state_label(state: ChunkState) -> &'static str {
    match state {
        ChunkState::Done => "Completed",
        ChunkState::InProgress => "In progress",
        ChunkState::Pending => "Waiting",
        ChunkState::Stuck => "Needs attention",
        ChunkState::Blocked => "Blocked",
        ChunkState::Backlog => "Backlog",
        ChunkState::Obsoleted => "Obsoleted",
    }
}

fn state_label(value: &str) -> String {
    match value {
        "done" => "Completed".into(),
        "in_progress" => "In progress".into(),
        "pending" => "Waiting".into(),
        "stuck" => "Needs attention".into(),
        _ => {
            let text = value.replace('_', " ");
            let mut letters = text.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + letters.as_str()
            })
        }
    }
}

fn work_name(chunk: &super::RoadmapChunk) -> &str {
    if !chunk.title.trim().is_empty() {
        &chunk.title
    } else if !chunk.name.trim().is_empty() && chunk.name != chunk.id {
        &chunk.name
    } else {
        "Untitled work item"
    }
}

fn work_reference_name(model: &Model, reference: &str) -> Option<String> {
    let unavailable = || "Unavailable work item".to_owned();
    if let Some(id) = reference.strip_prefix("task:") {
        return Some(
            model
                .rail()
                .ship()
                .task(id)
                .map_or_else(unavailable, |task| {
                    if task.title.trim().is_empty() {
                        "Untitled task".to_owned()
                    } else {
                        task.title.clone()
                    }
                }),
        );
    }
    if let Some(number) = reference
        .strip_prefix("think:")
        .or_else(|| reference.strip_prefix("step:"))
    {
        return Some(
            number
                .parse::<u32>()
                .ok()
                .and_then(|number| model.trace().step(number))
                .map_or_else(unavailable, |step| {
                    if step.purpose.trim().is_empty() {
                        format!("Step {}", step.number)
                    } else {
                        format!("Step {} — {}", step.number, step.purpose)
                    }
                }),
        );
    }
    // Related records name their target directly. The reverse cross-reference
    // lookup belongs to step-to-roadmap navigation and would return this owner.
    let id = reference.strip_prefix("chunk:").unwrap_or(reference);
    let Some(chunk) = model.roadmap().chunk(id) else {
        return Some(unavailable());
    };
    if model.selected() == Some(&Selected::Chunk(chunk.id.clone())) {
        return None;
    }
    Some(work_name(chunk).to_owned())
}

fn work_names(model: &Model, refs: &[String]) -> String {
    let mut seen = std::collections::HashSet::new();
    refs.iter()
        .filter_map(|reference| work_reference_name(model, reference))
        .filter(|name| seen.insert(name.clone()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Inspector rendering and its scroll extent share the same card rows.
pub struct Inspector;

struct Card {
    title: String,
    status: String,
    icon: Icon,
    metadata: String,
    actions: Vec<(&'static str, WidgetId)>,
    fields: Vec<(String, String)>,
    diff: Option<String>,
    viewer: Option<Vec<String>>,
    proposals: Vec<(usize, String)>,
}

impl Inspector {
    fn full_roadmap_details(model: &Model) -> bool {
        model.uses_flight_recorder()
            && model.tab() == super::StageTab::Roadmap
            && matches!(model.selected(), Some(Selected::Chunk(_)))
    }

    /// Resolve the displayed dependency against the current selected work item.
    pub(super) fn roadmap_dependency(model: &Model, index: usize) -> Option<Selected> {
        if !Self::full_roadmap_details(model) {
            return None;
        }
        let Selected::Chunk(id) = model.selected()? else {
            return None;
        };
        let reference = model.roadmap().chunk(id)?.deps.get(index)?;
        let dependency = model.roadmap().chunk_for_ref(reference)?;
        Some(Selected::Chunk(dependency.id.clone()))
    }

    /// Render the selected object with a fixed header and scrolling fields.
    #[must_use]
    pub fn view(model: &Model, pen: &Pen) -> Node {
        let mut panel = Node::column()
            .id(Widget::Inspector.id())
            .style(pen.stage.clone())
            .viewport()
            .min_width(Length::Cells(0))
            .frame(
                Frame::new(pen.panel(model.pane() == Pane::Inspector).clone())
                    .title("Details")
                    .title_style(pen.heading.clone())
                    .drawn(model.has_pixels()),
            );
        if model.inspector_stacked() {
            panel = panel
                .height(Length::Cells(model.inspector_stack_rows()))
                .shrink(0.0);
        } else {
            panel = panel
                .width(Length::Cells(Space::INSPECTOR_COLS))
                .shrink(0.0);
        }
        let Some(card) = Card::of(model) else {
            let text = if model.selected().is_some() {
                "This record is unavailable."
            } else {
                "Select a record to see details."
            };
            return panel.child(line(text, &pen.note));
        };
        let width = model.inspector_width();
        let actions = card
            .actions
            .iter()
            .fold(Node::row().gap(Space::TIGHT), |row, (label, id)| {
                row.child(
                    line(
                        label,
                        if *label == "Undo" {
                            &pen.deny
                        } else {
                            &pen.link
                        },
                    )
                    .id(*id),
                )
            })
            .child(
                line(
                    if card.viewer.is_some() {
                        "Close"
                    } else if model.inspector_open() {
                        "Less"
                    } else {
                        "More"
                    },
                    &pen.permission,
                )
                .id(Widget::InspectorDetail.id()),
            );
        let fields = card.rows(width, model.inspector_open(), pen);
        let height = Self::field_height(model);
        let top = model
            .inspector_scroll
            .min(fields.len().saturating_sub(height));
        // A horizontal gutter only: the stacked card's five inner rows remain usable.
        let content = Node::column()
            .grow(1.0)
            .min_width(Length::Cells(0))
            .viewport()
            .child(line(&card.title, &pen.heading).id(Widget::InspectorTitle.id()))
            .child(
                Marked::new(model, pen)
                    .icon(card.icon)
                    .text(&format!(" {}", card.status))
                    .node()
                    .style(pen.stage.clone())
                    .height(Length::Cells(1))
                    .shrink(0.0)
                    .id(Widget::InspectorStatus.id()),
            )
            .child(line(&card.metadata, &pen.note).id(Widget::InspectorMetadata.id()))
            .child(
                actions
                    .height(Length::Cells(1))
                    .shrink(0.0)
                    .viewport()
                    .id(Widget::InspectorActions.id()),
            )
            .child(
                Node::column()
                    .grow(1.0)
                    .viewport()
                    .id(Widget::InspectorFields.id())
                    .children(fields.into_iter().skip(top).take(height)),
            );
        panel.child(
            Node::row()
                .grow(1.0)
                .viewport()
                .child(Node::leaf().width(Length::Cells(Space::PAD)).shrink(0.0))
                .child(content)
                .child(Node::leaf().width(Length::Cells(Space::PAD)).shrink(0.0)),
        )
    }

    /// All hit targets rendered inside this card, including its domain actions.
    pub(super) fn contains_target(id: WidgetId) -> bool {
        matches!(
            Widget::from_id(id),
            Some(
                Widget::Inspector
                    | Widget::InspectorTitle
                    | Widget::InspectorStatus
                    | Widget::InspectorMetadata
                    | Widget::InspectorActions
                    | Widget::InspectorFields
                    | Widget::InspectorDetail
                    | Widget::LinkedWork
                    | Widget::ToolOpen
                    | Widget::ToolUndo
                    | Widget::Checkpoint
                    | Widget::RoadmapStart
                    | Widget::RoadmapComplete
                    | Widget::RoadmapRequest
            )
        ) || Widget::roadmap_dependency_index(id).is_some()
            || Widget::trace_pin_step(id).is_some()
            || Widget::trace_link_step(id).is_some()
            || Widget::trace_chunk_step(id).is_some()
            || Widget::change_open_index(id).is_some()
            || Widget::change_undo_index(id).is_some()
            || Widget::change_approve_index(id).is_some()
            || Widget::proposal_accept_index(id).is_some()
            || Widget::proposal_reject_index(id).is_some()
            || Widget::signal_promote_index(id).is_some()
            || Widget::signal_ignore_index(id).is_some()
            || Widget::signal_snooze_index(id).is_some()
    }

    /// Scroll within the current field set, clamped to its visible extent.
    pub fn scroll(model: &mut Model, delta: isize) {
        if model.has_pixels() && !model.surface().is_native() {
            Self::flight_scroll(model, delta);
            return;
        }
        let pen = Pen::for_theme(&model.theme());
        let total = Card::of(model).map_or(0, |card| {
            card.rows(model.inspector_width(), model.inspector_open(), &pen)
                .len()
        });
        let max = total.saturating_sub(Self::field_height(model));
        model.inspector_scroll = model.inspector_scroll.saturating_add_signed(delta).min(max);
    }

    fn field_height(model: &Model) -> usize {
        if model.inspector_stacked() {
            usize::from(model.inspector_stack_rows()).saturating_sub(6)
        } else {
            // Stage adds a tab row and (on graphics) vertical padding.
            model
                .stage_rows()
                .saturating_add(
                    1 + if model.has_pixels() {
                        2 * Space::PAD as usize
                    } else {
                        0
                    },
                )
                .saturating_sub(4)
        }
    }
}

impl Card {
    fn of(model: &Model) -> Option<Self> {
        let mut card = Self {
            title: String::new(),
            status: String::new(),
            icon: Icon::Pending,
            metadata: String::new(),
            actions: Vec::new(),
            fields: Vec::new(),
            diff: None,
            viewer: None,
            proposals: Vec::new(),
        };
        if let Some(lines) = model.trace().viewer() {
            card.title = "Checkpoint".into();
            card.status = "Saved reasoning".into();
            card.metadata = format!("{} lines · Esc closes", lines.len());
            card.viewer = Some(lines.to_vec());
            return Some(card);
        }
        if let Some((title, lines)) = model.changes().viewer() {
            card.title = title.into();
            card.status = "Read only".into();
            card.metadata = format!("{} lines · Esc closes", lines.len());
            card.viewer = Some(lines.to_vec());
            return Some(card);
        }
        match model.selected()? {
            Selected::Task(id) => {
                let task = model.rail().ship().task(id)?;
                card.title = task.title.clone();
                card.status = match task.state {
                    super::TaskState::Planned => "Planned",
                    super::TaskState::Active => "In progress",
                    super::TaskState::Completed => "Completed",
                    super::TaskState::Blocked => "Blocked",
                    super::TaskState::Skipped => "Skipped",
                }
                .into();
                card.metadata = format!(
                    "{} {} passed · {} failed",
                    task.checks_passed,
                    if task.checks_passed == 1 {
                        "check"
                    } else {
                        "checks"
                    },
                    task.checks_failed
                );
                for item in
                    super::session_item::SessionItem::collect(model.trace(), model.changes())
                {
                    let super::session_item::SessionItem::Moment(_, moment) = item else {
                        continue;
                    };
                    match moment {
                        super::ShipMoment::Action {
                            task_id,
                            description,
                            ..
                        } if task_id == id => {
                            card.fields.push(("Action".into(), description.clone()));
                        }
                        super::ShipMoment::Check {
                            task_id,
                            name,
                            passed,
                            ..
                        } if task_id == id => {
                            card.fields.push((
                                "Check".into(),
                                format!("{} · {name}", if *passed { "Passed" } else { "Failed" }),
                            ));
                        }
                        _ => {}
                    }
                }
                if card.fields.is_empty() {
                    card.fields.push((
                        "Updates".into(),
                        "No actions or checks recorded yet.".into(),
                    ));
                }
            }
            Selected::Tool(id) => {
                let tool = model.turn().tool(id)?;
                card.tool(tool, id);
                if tool
                    .record()
                    .terminal
                    .as_deref()
                    .and_then(|terminal| model.term(terminal))
                    .is_some_and(super::Term::is_killed)
                {
                    card.status = "Stopped by you".into();
                    card.icon = Icon::Stopped;
                }
            }
            Selected::Chunk(id) => {
                let chunk = model.roadmap().chunk(id)?;
                card.title = work_name(chunk).to_owned();
                card.status = chunk_state_label(chunk.state).into();
                card.icon = match chunk.state {
                    ChunkState::Done => Icon::Done,
                    ChunkState::InProgress => Icon::InProgress,
                    ChunkState::Blocked | ChunkState::Stuck => Icon::Blocked,
                    ChunkState::Backlog | ChunkState::Pending | ChunkState::Obsoleted => {
                        Icon::Pending
                    }
                };
                card.metadata = format!(
                    "{} · {}",
                    chunk.band(),
                    chunk.group.as_deref().unwrap_or("work item")
                );
                let full = Inspector::full_roadmap_details(model);
                if !full {
                    card.fields.push(("Title".into(), chunk.title.clone()));
                }
                card.fields
                    .push(("Description".into(), chunk.description.clone()));
                if full || model.inspector_open() {
                    card.fields.extend(
                        chunk.acceptance.iter().enumerate().map(|(i, text)| {
                            (format!("Success condition {}", i + 1), text.clone())
                        }),
                    );
                    if !full {
                        card.fields
                            .push(("Depends on".into(), work_names(model, &chunk.deps)));
                    }
                    card.fields
                        .push(("Related work".into(), work_names(model, &chunk.cross_refs)));
                }
                card.proposals = model
                    .roadmap()
                    .proposals()
                    .into_iter()
                    .enumerate()
                    .filter(|(_, (owner, _, _, _))| owner.id == chunk.id)
                    .map(|(index, (_, kind, suggestion, reason))| {
                        (index, format!("{} → {suggestion} · {reason}", kind.label()))
                    })
                    .collect();
                if chunk.state.is_working() {
                    card.actions
                        .push(("Complete", Widget::RoadmapComplete.id()));
                } else if matches!(
                    chunk.state,
                    ChunkState::Pending | ChunkState::Blocked | ChunkState::Backlog
                ) {
                    card.actions.push(("Start", Widget::RoadmapStart.id()));
                }
            }
            Selected::Signal(id) => {
                let signals = model.roadmap().signals();
                let (index, signal) = signals
                    .iter()
                    .enumerate()
                    .find(|(_, signal)| signal.id == *id)?;
                card.title = signal.title().to_owned();
                card.status = state_label(&signal.status);
                card.icon = Icon::Signal;
                card.metadata = format!("{} · {}", signal.kind, signal.from);
                card.actions = vec![
                    ("Promote", Widget::signal_promote(index)),
                    ("Ignore", Widget::signal_ignore(index)),
                    ("Snooze", Widget::signal_snooze(index)),
                ];
                card.fields = vec![("Details".into(), signal.body.clone())];
                if let Some(findings) = &signal.findings {
                    card.fields.push(("Findings".into(), findings.clone()));
                }
                if model.inspector_open()
                    && let Some(confidence) = signal.confidence
                {
                    card.fields
                        .push(("Confidence".into(), format!("{confidence:.1}")));
                }
            }
            Selected::Step(number) => {
                let step = model.trace().step(*number)?;
                card.title = if step.purpose.is_empty() {
                    format!("Step {number}")
                } else {
                    step.purpose.clone()
                };
                card.status = if step.revised_by.is_some() {
                    "Updated"
                } else if step.is_final {
                    "Complete"
                } else {
                    "Recorded"
                }
                .into();
                card.icon = if step.pinned {
                    Icon::Pinned
                } else {
                    Icon::Done
                };
                card.metadata = format!(
                    "Step {number} · {}",
                    step.branch_name.as_deref().unwrap_or("Main investigation")
                );
                card.actions.push((
                    if step.pinned { "Unsave" } else { "Save" },
                    Widget::trace_pin(*number),
                ));
                card.actions.push(("Checkpoint", Widget::Checkpoint.id()));
                if step
                    .execution_ref
                    .as_deref()
                    .is_some_and(|r| model.roadmap().chunk_for_ref(r).is_some())
                {
                    card.actions
                        .push(("Related work", Widget::trace_chunk(*number)));
                }
                card.fields = step
                    .fields()
                    .into_iter()
                    .filter(|(label, _)| {
                        model.inspector_open()
                            || matches!(*label, "Reasoning" | "Result" | "Why" | "Next")
                    })
                    .map(|(label, value)| (label.into(), value))
                    .collect();
                if model.inspector_open()
                    && let Some(timestamp) = &step.timestamp
                {
                    card.fields.push(("Recorded".into(), timestamp.clone()));
                }
                if let Some(n) = step.revises {
                    card.fields.push(("Updates step".into(), n.to_string()));
                }
                if let Some(n) = step.revised_by {
                    card.fields.push(("Updated by step".into(), n.to_string()));
                }
            }
            Selected::Receipt(_) => {
                let item =
                    super::session_item::SessionItem::collect(model.trace(), model.changes())
                        .into_iter()
                        .find(|item| Some(&item.selection()) == model.selected())?;
                let super::session_item::SessionItem::Moment(_, moment) = item else {
                    return None;
                };
                match moment {
                    super::ShipMoment::Action {
                        task_id,
                        timestamp,
                        kind,
                        description,
                        id: _,
                        files,
                    } => {
                        card.title = description.clone();
                        card.status = state_label(kind);
                        card.icon = Icon::InProgress;
                        card.metadata = task_name(model, task_id);
                        card.fields = vec![
                            ("Recorded".into(), timestamp.clone()),
                            ("Files".into(), files.join("\n")),
                        ];
                    }
                    super::ShipMoment::Check {
                        task_id,
                        timestamp,
                        name,
                        passed,
                        verified,
                        exit_code,
                        duration_ms,
                    } => {
                        card.title = name.clone();
                        card.status = match (*passed, *verified) {
                            (true, true) => "Passed",
                            (false, true) => "Failed",
                            (true, false) => "Passed · self-reported",
                            (false, false) => "Failed · self-reported",
                        }
                        .into();
                        card.icon = Icon::check(*passed);
                        card.metadata = task_name(model, task_id);
                        card.fields.push(("Recorded".into(), timestamp.clone()));
                        card.fields.push((
                            "Evidence".into(),
                            if *verified {
                                "Magistr ran this check and recorded the result."
                            } else {
                                "The agent reported this result. Magistr has no recorded run."
                            }
                            .into(),
                        ));
                        if model.projection().freshness() == super::Freshness::Stale {
                            card.fields.push((
                                "Updates".into(),
                                "Waiting for current results. A newer result may be available."
                                    .into(),
                            ));
                        }
                        if let Some(code) = exit_code {
                            card.fields.push(("Exit code".into(), code.to_string()));
                        }
                        if let Some(ms) = duration_ms {
                            card.fields.push(("Duration".into(), format!("{ms} ms")));
                        }
                    }
                }
            }
            Selected::Run(index) => {
                let run = model.changes().runs()?.1.get(*index)?;
                card.title = run.name.clone();
                card.status = if run.passed { "Passed" } else { "Failed" }.into();
                card.icon = Icon::check(run.passed);
                card.metadata = "Latest check · time not recorded".into();
                card.fields.push(("Details".into(), run.detail.clone()));
            }
            Selected::Change(id) => {
                let (index, entry) = model
                    .changes()
                    .entries()
                    .iter()
                    .enumerate()
                    .find(|(_, entry)| entry.identity() == *id)?;
                card.title = entry.path.clone();
                card.status = if entry.parse_ok {
                    "Syntax checked"
                } else {
                    "Syntax needs attention"
                }
                .into();
                if model.changes().is_approved(&entry.identity()) {
                    card.status.push_str(" · approved");
                }
                if let Some(reason) = model.changes().refusal(&entry.id) {
                    card.status.push_str(" · undo refused");
                    card.fields.push(("Undo refused".into(), reason.to_owned()));
                }
                card.icon = Icon::check(entry.parse_ok);
                card.metadata = format!("{} lines added · {} removed", entry.added, entry.removed);
                card.actions = vec![
                    ("Open", Widget::change_open(index)),
                    ("Undo", Widget::change_undo(index)),
                    ("Approve", Widget::change_approve(index)),
                ];
                card.fields.insert(0, ("File".into(), entry.path.clone()));
                if model.inspector_open()
                    && let Some(at) = entry
                        .at_ms
                        .and_then(chrono::DateTime::from_timestamp_millis)
                {
                    card.fields.push(("Recorded".into(), at.to_rfc3339()));
                }
                card.diff = Some(entry.diff.clone());
            }
        }
        Some(card)
    }

    fn tool(&mut self, line: &crate::turn::Line, _id: &str) {
        use crate::turn::LineKind;
        use magistr_application::sessions::ToolStatus;
        let status = match line.kind() {
            LineKind::Tool {
                status,
                verb,
                duration_ms,
                ..
            } => {
                self.title = line.text().into();
                self.metadata = verb.label().to_owned();
                if let Some(ms) = duration_ms {
                    self.metadata.push_str(&format!(" · {ms} ms"));
                }
                *status
            }
            LineKind::Edit {
                path,
                status,
                undone,
                ..
            } => {
                self.title = path.clone();
                self.metadata = "File change".to_owned();
                if *undone {
                    self.metadata.push_str(" · undone");
                }
                self.actions.push(("Open", Widget::ToolOpen.id()));
                if !undone {
                    self.actions.push(("Undo", Widget::ToolUndo.id()));
                }
                self.diff = Some(line.text().into());
                *status
            }
            _ => return,
        };
        let (label, icon) = match status {
            ToolStatus::Pending => ("Waiting", Icon::Pending),
            ToolStatus::InProgress => ("Running", Icon::Running),
            ToolStatus::Completed => ("Completed", Icon::Done),
            ToolStatus::Failed => ("Failed", Icon::Failed),
        };
        self.status = label.into();
        self.icon = icon;
        let record = line.record();
        if let Some(size) = record.truncated_from {
            self.fields.push((
                "Saved output".into(),
                format!("Showing the beginning of {size} bytes."),
            ));
        }
        for (label, value) in [("Input", &record.input), ("Output", &record.output)] {
            if let Some(value) = value {
                self.fields.push((
                    label.into(),
                    value
                        .as_str()
                        .map_or_else(|| value.to_string(), str::to_owned),
                ));
            }
        }
        self.fields.extend(
            record
                .text
                .iter()
                .map(|text| ("Contents".into(), text.clone())),
        );
        self.fields
            .extend(record.locations.iter().map(|(path, line)| {
                (
                    "File".into(),
                    line.map_or_else(|| path.clone(), |line| format!("{path}:{line}")),
                )
            }));
        if record.terminal.is_some() {
            self.fields.push((
                "Terminal output".into(),
                "Available in the conversation.".into(),
            ));
        }
    }

    fn rows(&self, width: u16, full: bool, pen: &Pen) -> Vec<Node> {
        if let Some(lines) = &self.viewer {
            return lines
                .iter()
                .flat_map(|text| {
                    if text.is_empty() {
                        vec![line("", &pen.code)]
                    } else {
                        TextMetrics::new(text)
                            .wrap(width.max(1))
                            .into_iter()
                            .map(|wrapped| {
                                line(wrapped, &pen.code).text_role(crate::layout::TextRole::Data)
                            })
                            .collect()
                    }
                })
                .collect();
        }
        let mut rows = Vec::new();
        for (index, proposal) in &self.proposals {
            rows.extend(
                TextMetrics::new(proposal)
                    .wrap(width.max(1))
                    .into_iter()
                    .map(|text| line(text, &pen.permission)),
            );
            rows.push(
                Node::row()
                    .gap(Space::TIGHT)
                    .height(Length::Cells(1))
                    .shrink(0.0)
                    .child(line("Accept", &pen.allow).id(Widget::proposal_accept(*index)))
                    .child(line("Reject", &pen.deny).id(Widget::proposal_reject(*index))),
            );
        }
        for (label, value) in &self.fields {
            if value.trim().is_empty() {
                continue;
            }
            let text = format!("{label} · {value}");
            let metrics = TextMetrics::new(text);
            let wrapped = metrics.wrap(width.max(1));
            let limit = if full { wrapped.len() } else { 2 };
            rows.extend(wrapped.into_iter().take(limit).map(|text| {
                let row = line(text, &pen.stage);
                if matches!(label.as_str(), "Updates step" | "Updated by step") {
                    value.parse().map_or(row.clone(), |number| {
                        row.id(Widget::trace_link(number)).style(pen.link.clone())
                    })
                } else {
                    row
                }
            }));
        }
        if let Some(diff) = &self.diff {
            for text in diff.lines().take(if full { usize::MAX } else { 2 }) {
                rows.extend(
                    TextMetrics::new(text)
                        .wrap(width.max(1))
                        .into_iter()
                        .map(|wrapped| {
                            line(wrapped, pen.row(diff_kind(text)))
                                .text_role(crate::layout::TextRole::Data)
                        }),
                );
            }
        }
        rows
    }
}

fn line(text: &str, style: &Style) -> Node {
    Node::text(text)
        .style(style.clone())
        .height(Length::Cells(1))
        .shrink(0.0)
}
