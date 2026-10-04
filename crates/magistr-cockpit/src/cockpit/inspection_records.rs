//! Typed records decide the selected document's hierarchy and recorded evidence.
use super::{Inspection, Model, RecordLink, RecordTarget, Selected, Widget};
use crate::cockpit::inspector::Inspector;
use crate::cockpit::{ChunkState, Freshness, ShipMoment, TaskState};
use crate::pixel::Icon;
use crate::turn::LineKind;
use std::hash::{Hash, Hasher};

/// Only selected data and its displayed relationships enter the content key.
pub(super) fn fingerprint(model: &Model) -> Option<u64> {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let mut write = |value: String| value.hash(&mut hash);
    write(format!("{:?}", model.theme().mode()));
    if let Some(lines) = model.trace().viewer() {
        write(format!("{lines:?}"));
        return Some(hash.finish());
    }
    if let Some(viewer) = model.changes().viewer() {
        write(format!("{viewer:?}"));
        return Some(hash.finish());
    }
    let selected = model.selected()?;
    write(format!("{selected:?}"));
    let references: Vec<String> = match selected {
        Selected::Chunk(id) => {
            let chunk = model.roadmap().chunk(id)?;
            write(format!(
                "{chunk:?}:{}:{}:{}",
                model.roadmap().can_start(chunk),
                model.roadmap().can_complete(chunk),
                model.roadmap().progress_detail(chunk)
            ));
            write(format!(
                "{:?}",
                model
                    .roadmap()
                    .proposals()
                    .into_iter()
                    .enumerate()
                    .filter(|(_, (owner, _, _, _))| owner.id == *id)
                    .collect::<Vec<_>>()
            ));
            chunk
                .deps
                .iter()
                .chain(&chunk.cross_refs)
                .cloned()
                .collect()
        }
        Selected::Step(number) => {
            let step = model.trace().step(*number)?;
            write(format!("{step:?}"));
            write(format!(
                "{:?}",
                model
                    .trace()
                    .investigations()
                    .into_iter()
                    .filter(|b| b.contains(*number))
                    .map(|b| (b.question, b.status_label(), b.relationship()))
                    .collect::<Vec<_>>()
            ));
            step.dependencies
                .iter()
                .chain(&step.revises)
                .chain(&step.revised_by)
                .map(|number| format!("think:{number}"))
                .chain(step.execution_ref.iter().cloned())
                .collect()
        }
        Selected::Task(id) => {
            write(format!("{:?}", model.rail().ship().task(id)?));
            for moment in model
                .trace()
                .moments()
                .iter()
                .filter(|moment| task_id(moment) == id)
            {
                write(format!("{moment:?}"));
            }
            Vec::new()
        }
        Selected::Receipt(id) => {
            write(format!("{:?}", selected_moment(model, id)?));
            write(format!("{:?}", model.projection().freshness()));
            Vec::new()
        }
        Selected::Tool(id) => {
            let line = model.turn().tool(id)?;
            write(format!("{:?}:{:?}", line.kind(), line.record()));
            if let Some(terminal) = line
                .record()
                .terminal
                .as_deref()
                .and_then(|id| model.term(id))
            {
                write(format!(
                    "{}:{:?}",
                    terminal.is_killed(),
                    terminal.transcript()
                ));
            }
            Vec::new()
        }
        Selected::Change(id) => {
            let entry = model
                .changes()
                .entries()
                .iter()
                .find(|e| e.identity() == *id)?;
            write(format!(
                "{:?}",
                model
                    .changes()
                    .entries()
                    .iter()
                    .position(|row| row.identity() == *id)
            ));
            write(format!(
                "{entry:?}:{:?}:{}",
                model.changes().refusal(&entry.id),
                model.changes().is_approved(id)
            ));
            Vec::new()
        }
        Selected::Signal(id) => {
            let signals = model.roadmap().inbox();
            let signal = signals.iter().find(|s| s.id == *id)?;
            write(format!(
                "{signal:?}:{:?}",
                model
                    .roadmap()
                    .signals()
                    .iter()
                    .position(|row| row.id == *id)
            ));
            signal.cross_refs.clone()
        }
        Selected::Run(index) => {
            write(format!("{:?}", model.changes().runs()?.1.get(*index)?));
            Vec::new()
        }
    };
    for reference in references {
        write(format!(
            "{:?}",
            Inspector::work_reference(model, &reference)
        ));
    }
    Some(hash.finish())
}

fn task_id(moment: &ShipMoment) -> &str {
    match moment {
        ShipMoment::Action { task_id, .. } | ShipMoment::Check { task_id, .. } => task_id,
    }
}

fn selected_moment<'a>(model: &'a Model, id: &str) -> Option<&'a ShipMoment> {
    model
        .trace()
        .moments()
        .iter()
        .enumerate()
        .find_map(|(index, moment)| {
            let item = crate::cockpit::session_item::SessionItem::Moment(index, moment);
            (item.selection() == Selected::Receipt(id.to_owned())).then_some(moment)
        })
}

pub(super) fn link(model: &Model, reference: &str, authored: &str, index: usize) -> RecordLink {
    let resolved = Inspector::work_reference(model, reference);
    let (label, target) = if let Some((name, selected)) = resolved {
        (name, RecordTarget::Record(selected))
    } else if reference.starts_with("https://")
        || reference.starts_with("http://")
        || reference.starts_with("mailto:")
    {
        (
            if authored.is_empty() {
                reference.to_owned()
            } else {
                authored.to_owned()
            },
            RecordTarget::Address(reference.to_owned()),
        )
    } else {
        ("Unavailable work item".into(), RecordTarget::Unavailable)
    };
    RecordLink {
        label,
        target,
        reference: reference.to_owned(),
        widget: Widget::detail_link(index),
    }
}

pub(super) fn build(model: &Model) -> Option<Inspection> {
    let mut card = Inspection::blank();
    if let Some(lines) = model.trace().viewer() {
        card.title = "Checkpoint".into();
        card.status = "Saved reasoning".into();
        card.viewer = true;
        card.code(model, "Saved reasoning", &lines.join("\n"), "", false);
        return Some(card);
    }
    if let Some((title, lines)) = model.changes().viewer() {
        card.title = title.into();
        card.status = "Read only".into();
        card.viewer = true;
        card.code(model, "File contents", &lines.join("\n"), "", false);
        return Some(card);
    }
    match model.selected()? {
        Selected::Chunk(id) => {
            let chunk = model.roadmap().chunk(id)?;
            card.title = work_name(&chunk.title, &chunk.name);
            card.status = model.roadmap().status_label(chunk).into();
            card.icon = match chunk.state {
                ChunkState::Done => Icon::Done,
                ChunkState::InProgress => Icon::InProgress,
                ChunkState::Blocked | ChunkState::Stuck => Icon::Blocked,
                _ => Icon::Pending,
            };
            card.metadata = format!(
                "{} · {}",
                chunk.band(),
                chunk.group.as_deref().unwrap_or("Work item")
            );
            if let Some(reason) = &chunk.blocker {
                card.section(model, "Blocked", reason, false);
            } else if matches!(
                model.roadmap().readiness(chunk),
                magistr_workflow_contracts::roadmap::Readiness::Waiting(_)
                    | magistr_workflow_contracts::roadmap::Readiness::Inactive(
                        magistr_workflow_contracts::ChunkStatus::Blocked
                    )
            ) {
                card.section(
                    model,
                    model.roadmap().status_label(chunk),
                    &model.roadmap().progress_detail(chunk),
                    false,
                );
            }
            structured(&mut card, model, chunk.content.as_ref());
            if !represented(&card, &chunk.description) {
                card.section(model, "Proposed work", &chunk.description, false);
            }
            let requirements = chunk
                .acceptance
                .iter()
                .enumerate()
                .map(|(at, text)| format!("{}. {}", at + 1, text.replace('\n', "\n   ")))
                .collect::<Vec<_>>()
                .join("\n");
            card.section(model, "Acceptance criteria", &requirements, false);
            card.references(model, "Dependencies", &chunk.deps);
            card.references(model, "Related work", &chunk.cross_refs);
            card.section(model, "Context", &chunk.notes, true);
            for (index, (owner, kind, suggestion, reason)) in
                model.roadmap().proposals().into_iter().enumerate()
            {
                if owner.id != *id {
                    continue;
                }
                card.section(
                    model,
                    "Suggested change",
                    &format!("{} → {suggestion}\n\n{reason}", kind.label()),
                    false,
                );
                if let Some(section) = card.sections.last_mut() {
                    section.actions = vec![
                        ("Accept", Widget::proposal_accept(index)),
                        ("Reject", Widget::proposal_reject(index)),
                    ];
                }
            }
            if model.roadmap().can_complete(chunk) {
                card.actions
                    .push(("Complete", Widget::RoadmapComplete.id()));
            } else if model.roadmap().can_start(chunk) {
                card.actions.push(("Start", Widget::RoadmapStart.id()));
            }
            card.actions
                .push(("Request a change", Widget::RoadmapRequest.id()));
        }
        Selected::Signal(id) => {
            let signals = model.roadmap().inbox();
            let signal = signals.iter().find(|s| s.id == *id)?;
            card.title = signal.title().to_owned();
            card.status = state_label(&signal.status);
            card.icon = Icon::Signal;
            card.metadata = format!("{} · {}", signal.kind, signal.from);
            // A title already carries the summary. Keep every authored section and fact.
            structured(&mut card, model, signal.content.as_ref());
            if let Some(first) = card.sections.first()
                && first.heading == "Summary"
                && first.text.source() == card.title
            {
                card.sections.remove(0);
            }
            if !represented(&card, &signal.body) && signal.body != card.title {
                card.section(model, "Original feedback", &signal.body, false);
            }
            for enrichment in &signal.enrichment {
                card.section(model, "Findings", &enrichment.summary, false);
                let mut references = enrichment.sources.clone();
                if let Some(number) = enrichment.think_step {
                    references.push(format!("think:{number}"));
                }
                card.references(model, "Research sources", &references);
                card.section(
                    model,
                    "Research details",
                    &format!(
                        "**Agent confidence:** {:.0}%\n**Recorded:** {}",
                        enrichment.confidence * 100.0,
                        enrichment.at
                    ),
                    true,
                );
            }
            card.references(model, "Evidence and related work", &signal.cross_refs);
            if let Some(index) = model.roadmap().signals().iter().position(|s| s.id == *id) {
                card.actions = vec![
                    ("Promote", Widget::signal_promote(index)),
                    ("Research", Widget::signal_research(index)),
                    ("Ignore", Widget::signal_ignore(index)),
                    ("Snooze", Widget::signal_snooze(index)),
                ];
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
            card.section(
                model,
                "Result",
                if step.outcome.is_empty() {
                    "No result recorded yet."
                } else {
                    &step.outcome
                },
                false,
            );
            card.section(model, "Findings", &step.thought, false);
            card.section(model, "Why", &step.rationale, false);
            card.section(model, "Next", &step.next_action, false);
            card.section(model, "Context", &step.context, true);
            card.section(model, "Uncertainty", &step.uncertainty, true);
            card.section(model, "Reason for the update", &step.revision_reason, true);
            if let Some(value) = step.confidence {
                card.section(model, "Confidence", &format!("{:.0}%", value * 100.0), true);
            }
            if let Some(timestamp) = &step.timestamp {
                card.section(model, "Recorded", timestamp, true);
            }
            let dependencies = step
                .dependencies
                .iter()
                .filter(|n| !step.refutes.contains(n))
                .map(|n| format!("think:{n}"))
                .collect::<Vec<_>>();
            card.references(model, "Evidence", &dependencies);
            card.references(
                model,
                "Refuted findings",
                &step
                    .refutes
                    .iter()
                    .map(|n| format!("think:{n}"))
                    .collect::<Vec<_>>(),
            );
            if let Some(n) = step.revises {
                card.references(model, "Updates step", &[format!("think:{n}")]);
            }
            if let Some(n) = step.revised_by {
                card.references(model, "Updated by step", &[format!("think:{n}")]);
            }
            if let Some(reference) = &step.execution_ref {
                let reference = Inspector::linked_work(model, *number)
                    .map_or(reference.clone(), |(_, selected)| {
                        selection_reference(&selected)
                    });
                card.references(model, "Related work", &[reference]);
            }
            for branch in model
                .trace()
                .investigations()
                .into_iter()
                .filter(|b| b.contains(*number))
            {
                card.section(
                    model,
                    "Investigation",
                    &format!(
                        "{}\n\n{} · {}",
                        branch.question,
                        branch.status_label(),
                        branch.relationship()
                    ),
                    true,
                );
            }
            card.actions.push((
                if step.pinned { "Unsave" } else { "Save" },
                Widget::trace_pin(*number),
            ));
            card.actions.push(("Checkpoint", Widget::Checkpoint.id()));
        }
        Selected::Task(id) => {
            let task = model.rail().ship().task(id)?;
            card.title = task.title.clone();
            card.status = match task.state {
                TaskState::Planned => "Planned",
                TaskState::Active => "In progress",
                TaskState::Completed => "Completed",
                TaskState::Blocked => "Blocked",
                TaskState::Skipped => "Skipped",
            }
            .into();
            card.metadata = format!(
                "{} checks passed · {} failed",
                task.checks_passed, task.checks_failed
            );
            if let Some(reason) = &task.blocked_reason {
                card.section(model, "Blocked", reason, false);
            }
            let mut actions = Vec::new();
            let mut checks = Vec::new();
            for (index, moment) in model
                .trace()
                .moments()
                .iter()
                .enumerate()
                .filter(|(_, m)| task_id(m) == id)
            {
                let reference = selection_reference(
                    &crate::cockpit::session_item::SessionItem::Moment(index, moment).selection(),
                );
                if matches!(moment, ShipMoment::Action { .. }) {
                    actions.push(reference);
                } else {
                    checks.push(reference);
                }
            }
            if actions.is_empty() && checks.is_empty() {
                card.section(
                    model,
                    "Updates",
                    "No actions or checks recorded yet.",
                    false,
                );
            }
            card.references(model, "Recorded actions", &actions);
            card.references(model, "Recorded checks", &checks);
            for (reference, description) in &task.artifacts {
                card.section(
                    model,
                    "Artifact",
                    &format!("{description}\n\n`{reference}`"),
                    false,
                );
            }
            if let Some(at) = &task.started_at {
                card.section(model, "Started", at, true);
            }
            if let Some(at) = &task.completed_at {
                card.section(model, "Completed", at, true);
            }
            if let Some(estimate) = &task.estimate {
                card.section(model, "Estimate", estimate, true);
            }
        }
        Selected::Receipt(id) => receipt(&mut card, model, selected_moment(model, id)?),
        Selected::Run(index) => {
            let run = model.changes().runs()?.1.get(*index)?;
            card.title = run.name.clone();
            card.status = if run.passed { "Passed" } else { "Failed" }.into();
            card.icon = Icon::check(run.passed);
            card.metadata = "Latest check · time not recorded".into();
            card.section(model, "Result", &run.detail, false);
        }
        Selected::Change(id) => {
            let (index, entry) = model
                .changes()
                .entries()
                .iter()
                .enumerate()
                .find(|(_, e)| e.identity() == *id)?;
            card.title = entry.path.clone();
            card.status = if entry.parse_ok {
                "Syntax checked"
            } else {
                "Syntax needs attention"
            }
            .into();
            if model.changes().is_approved(id) {
                card.status.push_str(" · approved");
            }
            card.icon = Icon::check(entry.parse_ok);
            card.metadata = format!("{} lines added · {} removed", entry.added, entry.removed);
            if let Some(reason) = model.changes().refusal(&entry.id) {
                card.section(model, "Undo refused", reason, false);
            }
            card.code(model, "Changes", &entry.diff, "diff", false);
            if let Some(at) = entry
                .at_ms
                .and_then(chrono::DateTime::from_timestamp_millis)
            {
                card.section(model, "Recorded", &at.to_rfc3339(), true);
            }
            card.actions = vec![
                ("Open", Widget::change_open(index)),
                ("Undo", Widget::change_undo(index)),
                ("Approve", Widget::change_approve(index)),
            ];
        }
        Selected::Tool(id) => tool(&mut card, model, model.turn().tool(id)?),
    }
    Some(card)
}

fn structured(
    card: &mut Inspection,
    model: &Model,
    content: Option<&magistr_workflow_contracts::StructuredContent>,
) {
    let Some(content) = content else { return };
    card.section(model, "Summary", &content.summary, false);
    if let Some(facts) = &content.facts {
        card.section(
            model,
            "Facts",
            &facts
                .iter()
                .map(|f| format!("**{}:** {}", f.label, f.value))
                .collect::<Vec<_>>()
                .join("\n"),
            false,
        );
    }
    for section in content.sections.iter().flatten() {
        let mut source = section.prose.clone().unwrap_or_default();
        for item in section.list.iter().flatten() {
            if !source.is_empty() {
                source.push('\n');
            }
            source.push_str(&match item.done {
                Some(true) => format!("- [x] {}", item.text),
                Some(false) => format!("- [ ] {}", item.text),
                None => format!("- {}", item.text),
            });
        }
        card.section(model, &section.heading, &source, false);
    }
}

fn represented(card: &Inspection, source: &str) -> bool {
    source.trim().is_empty()
        || card
            .sections
            .iter()
            .any(|s| s.text.source().trim() == source.trim())
}

fn receipt(card: &mut Inspection, model: &Model, moment: &ShipMoment) {
    card.metadata = model
        .rail()
        .ship()
        .task(task_id(moment))
        .map_or("Recorded work".into(), |task| task.title.clone());
    match moment {
        ShipMoment::Action {
            description,
            kind,
            timestamp,
            files,
            result,
            tools,
            think_step,
            ..
        } => {
            card.title = description.clone();
            card.status = state_label(kind);
            card.icon = Icon::InProgress;
            card.section(model, "Result", result, false);
            card.section(model, "Recorded action", description, false);
            card.section(
                model,
                "Files",
                &files
                    .iter()
                    .map(|f| format!("- `{f}`"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                false,
            );
            card.section(
                model,
                "Tools reported by the agent",
                &tools.join(", "),
                true,
            );
            if let Some(number) = think_step {
                card.references(model, "Reasoning", &[format!("think:{number}")]);
            }
            card.section(model, "Recorded", timestamp, true);
        }
        ShipMoment::Check {
            name,
            passed,
            verified,
            exit_code,
            duration_ms,
            command,
            details,
            required,
            timestamp,
            ..
        } => {
            card.title = name.clone();
            card.status = format!(
                "{}{}",
                if *passed { "Passed" } else { "Failed" },
                if *verified { "" } else { " · self-reported" }
            );
            card.icon = Icon::check(*passed);
            card.section(
                model,
                "Evidence",
                if *verified {
                    "Magistr ran this check and recorded the result."
                } else {
                    "The agent reported this result. Magistr has no recorded run."
                },
                false,
            );
            if model.projection().freshness() == Freshness::Stale {
                card.section(
                    model,
                    "Updates",
                    "Waiting for current results. A newer result may be available.",
                    false,
                );
            }
            let mut facts = vec![format!(
                "**Required:** {}",
                if *required { "Yes" } else { "No" }
            )];
            if let Some(code) = exit_code {
                facts.push(format!("**Exit code:** {code}"));
            }
            if let Some(ms) = duration_ms {
                facts.push(format!("**Duration:** {ms} ms"));
            }
            card.section(model, "Result", &facts.join("\n"), false);
            if let Some(command) = command {
                card.code(model, "Command", command, "sh", false);
            }
            // Failed diagnostics remain available immediately. Large successful logs are secondary.
            card.code(model, "Recorded output", details, "", *passed);
            card.section(model, "Recorded", timestamp, true);
        }
    }
}

fn tool(card: &mut Inspection, model: &Model, line: &crate::turn::Line) {
    use magistr_application::sessions::ToolStatus;
    let status = match line.kind() {
        LineKind::Tool {
            status,
            verb,
            duration_ms,
            ..
        } => {
            card.title = line.text().into();
            card.metadata = verb.label().into();
            if let Some(ms) = duration_ms {
                card.metadata.push_str(&format!(" · {ms} ms"));
            }
            *status
        }
        LineKind::Edit {
            path,
            status,
            undone,
            ..
        } => {
            card.title = path.clone();
            card.metadata = "File change".into();
            card.actions.push(("Open", Widget::ToolOpen.id()));
            if !undone {
                card.actions.push(("Undo", Widget::ToolUndo.id()));
            }
            card.code(model, "Changes", line.text(), "diff", false);
            *status
        }
        _ => return,
    };
    let stopped = line
        .record()
        .terminal
        .as_deref()
        .and_then(|id| model.term(id))
        .is_some_and(crate::cockpit::Term::is_killed);
    let (label, icon) = if stopped {
        ("Stopped by you", Icon::Stopped)
    } else {
        match status {
            ToolStatus::Pending => ("Waiting", Icon::Pending),
            ToolStatus::InProgress => ("Running", Icon::Running),
            ToolStatus::Completed => ("Completed", Icon::Done),
            ToolStatus::Failed => ("Failed", Icon::Failed),
        }
    };
    card.status = label.into();
    card.icon = icon;
    let record = line.record();
    if let Some(size) = record.truncated_from {
        card.section(
            model,
            "Saved output",
            &format!("Showing the beginning of {size} bytes."),
            false,
        );
    }
    for (heading, value) in [("Input", &record.input), ("Output", &record.output)] {
        if let Some(value) = value {
            card.code(
                model,
                heading,
                &value
                    .as_str()
                    .map_or_else(|| value.to_string(), str::to_owned),
                if value.as_str().is_some() { "" } else { "json" },
                false,
            );
        }
    }
    card.section(model, "Contents", &record.text.join("\n"), false);
    for (path, line) in &record.locations {
        card.section(
            model,
            "File",
            &line.map_or_else(|| format!("`{path}`"), |line| format!("`{path}:{line}`")),
            false,
        );
    }
    if let Some(term) = record.terminal.as_deref().and_then(|id| model.term(id)) {
        card.code(model, "Terminal output", &term.transcript(), "", true);
    }
}

fn state_label(value: &str) -> String {
    let text = value.replace('_', " ");
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + chars.as_str()
    })
}

fn work_name(title: &str, name: &str) -> String {
    if !title.trim().is_empty() {
        title.to_owned()
    } else if !name.trim().is_empty() {
        name.to_owned()
    } else {
        "Untitled work item".into()
    }
}

fn selection_reference(selected: &Selected) -> String {
    match selected {
        Selected::Chunk(id) => format!("chunk:{id}"),
        Selected::Task(id) => format!("task:{id}"),
        Selected::Step(n) => format!("think:{n}"),
        Selected::Signal(id) => format!("signal:{id}"),
        Selected::Receipt(id) => id.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
#[path = "inspection_records.tests.rs"]
mod tests;
