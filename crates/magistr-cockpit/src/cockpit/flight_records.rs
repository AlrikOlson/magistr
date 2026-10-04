//! Browsable recorded work and roadmap items beside the conversation.
use super::session_item::SessionItem;
use super::{FlightStyle as S, Model, RoadmapSort, Selected, ShipMoment, StageTab, Widget};
use crate::layout::{Align, Length, Node};

/// Pixel list rows share their height with keyboard and wheel navigation.
pub struct FlightRecords;
impl FlightRecords {
    #[must_use]
    pub fn row_height(model: &Model) -> f32 {
        f32::from(model.cell_size().height) * 2.0 + 24.0
    }
    #[must_use]
    pub fn shown(model: &Model, height: f32, roadmap: bool) -> usize {
        let line = f32::from(model.cell_size().height);
        let controls = if roadmap {
            (line + 4.0) * if wide_controls(model) { 1.0 } else { 3.0 } + 12.0
        } else {
            0.0
        };
        ((height - (line + 14.0).max(34.0) - 22.0 - controls).max(0.0) / Self::row_height(model))
            .floor() as usize
    }
    #[must_use]
    pub fn view(model: &Model, width: f32, height: f32) -> Node {
        let s = S::for_model(model);
        let roadmap = model.tab() == StageTab::Roadmap;
        let title = if roadmap {
            "Work items"
        } else {
            "Recorded work"
        };
        let mut body = Node::column().min_height(Length::Px(0.0));
        if roadmap {
            body = body.child(controls(model, &s));
        }
        let shown = Self::shown(model, height, roadmap);
        let rows = if roadmap {
            work(model, &s, width)
        } else {
            records(model, &s, width)
        };
        let top = if roadmap {
            model.roadmap_top()
        } else {
            model.session_top()
        };
        body = body.child(
            Node::column()
                .id(if roadmap {
                    Widget::RoadmapTable.id()
                } else {
                    Widget::InspectorFields.id()
                })
                .viewport()
                .grow(1.0)
                .min_height(Length::Px(0.0))
                .children(rows.into_iter().skip(top).take(shown.max(1))),
        );
        s.outlined(
            Node::column()
                .min_height(Length::Px(0.0))
                .child(s.section_heading(
                    title,
                    if roadmap {
                        s.label(format!("{} shown", model.roadmap().rows().len()), s.muted)
                    } else {
                        s.label("Close", s.accent).id(Widget::TabTurn.id())
                    },
                ))
                .child(s.inset(body, 16.0, 10.0).grow(1.0)),
            s.panel,
            s.line,
        )
    }
}

fn controls(model: &Model, s: &S) -> Node {
    let roadmap = model.roadmap();
    let filter = s
        .chip(
            format!("Status: {} ▾", roadmap.filter_label()),
            s.accent,
            s.panel,
        )
        .id(Widget::RoadmapFilter.id());
    let wide = wide_controls(model);
    let mut root = Node::column().shrink(0.0);
    let mut all = Node::row().align_items(Align::Center);
    if wide {
        all = all.child(filter).child(S::flex());
    } else {
        root = root.child(filter);
    }
    for pair in RoadmapSort::ALL.chunks(if wide { 4 } else { 2 }) {
        let mut row = Node::row().align_items(Align::Center);
        for &column in pair {
            let name = match column {
                RoadmapSort::Priority => "Priority",
                RoadmapSort::Workstream => "Group",
                _ => column.label(),
            };
            let active = roadmap.sort() == column;
            let suffix = if active {
                if roadmap.descending() { " ↓" } else { " ↑" }
            } else {
                ""
            };
            row = row.child(
                s.chip(
                    format!("{name}{suffix}"),
                    if active { s.accent } else { s.muted },
                    if active { s.selected } else { s.panel },
                )
                .id(column.widget().id())
                .grow(1.0),
            );
        }
        if wide {
            all = all.child(row);
        } else {
            root = root.child(row);
        }
    }
    if wide {
        root = root.child(all);
    }
    root.child(S::vspace(12.0))
}

fn wide_controls(model: &Model) -> bool {
    let mut face = crate::pixel::Typography::default();
    face.set_cell(model.cell_size());
    let labels = [
        format!("Status: {} ▾", model.roadmap().filter_label()),
        "Name ↑".into(),
        "Status ↑".into(),
        "Priority ↑".into(),
        "Group ↑".into(),
    ];
    let required = labels
        .iter()
        .map(|text| face.advance(text, crate::layout::TextRole::Label) + 20.0)
        .sum::<f32>()
        + 24.0;
    model.flight_layout().left_width - 34.0 >= required
}

fn card(
    model: &Model,
    s: &S,
    width: f32,
    title: String,
    note: String,
    id: crate::widget_id::WidgetId,
    selected: bool,
) -> Node {
    let line = s.line_height;
    let fill = if selected { s.selected } else { s.panel };
    s.ground(
        s.inset(
            Node::column()
                .child(
                    s.label(
                        fit(model, &title, width - 56.0),
                        if selected { s.accent } else { s.ink },
                    )
                    .height(Length::Px(line))
                    .viewport(),
                )
                .child(S::vspace(4.0))
                .child(
                    s.label(fit(model, &note, width - 56.0), s.muted)
                        .height(Length::Px(line))
                        .viewport(),
                ),
            10.0,
            10.0,
        ),
        fill,
    )
    .id(id)
    .height(Length::Px(FlightRecords::row_height(model)))
    .shrink(0.0)
}
fn fit(model: &Model, text: &str, width: f32) -> String {
    use crate::layout::TextRole;
    let mut face = crate::pixel::Typography::default();
    face.set_cell(model.cell_size());
    let measure = |t: &str| face.advance(t, TextRole::Label);
    if measure(text) <= width {
        return text.to_owned();
    }
    let mut out = text.to_owned();
    while !out.is_empty() && measure(&format!("{out}…")) > width {
        out.pop();
    }
    format!("{out}…")
}

fn work(model: &Model, s: &S, width: f32) -> Vec<Node> {
    let mut rows: Vec<_> = model
        .roadmap()
        .rows()
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let status = model.roadmap().status_label(item);
            let note = item
                .group
                .as_ref()
                .map_or_else(|| status.to_owned(), |g| format!("{status} · {g}"));
            card(
                model,
                s,
                width,
                item.name.clone(),
                note,
                Widget::roadmap_row(index),
                model.selected() == Some(&Selected::Chunk(item.id.clone())),
            )
        })
        .collect();
    if rows.is_empty() {
        rows.push(s.prose("No work items match this status. Choose another status to see more."));
    }
    for (index, signal) in model.roadmap().signals().into_iter().enumerate() {
        rows.push(card(
            model,
            s,
            width,
            signal.title().to_owned(),
            "Feedback".into(),
            Widget::signal_row(index),
            model.selected() == Some(&Selected::Signal(signal.id.clone())),
        ));
    }
    rows
}

fn records(model: &Model, s: &S, width: f32) -> Vec<Node> {
    let items = SessionItem::collect(model.trace(), model.changes());
    if items.is_empty() {
        return vec![s.prose("Reasoning, checks and changes will appear here as work progresses.")];
    }
    items
        .into_iter()
        .enumerate()
        .map(|(row, item)| {
            let (title, kind) = match item {
                SessionItem::Step(_, step) => {
                    (step.purpose.clone(), format!("Step {}", step.number))
                }
                SessionItem::Moment(_, ShipMoment::Action { description, .. }) => {
                    (description.clone(), "Action".into())
                }
                SessionItem::Moment(
                    _,
                    ShipMoment::Check {
                        name,
                        passed,
                        verified,
                        ..
                    },
                ) => (
                    name.clone(),
                    match (*passed, *verified) {
                        (true, true) => "Check passed".into(),
                        (false, true) => "Check failed".into(),
                        (true, false) => "Check passed · self-reported".into(),
                        (false, false) => "Check failed · self-reported".into(),
                    },
                ),
                SessionItem::Change(_, change) => (
                    change.path.clone(),
                    format!("Change · +{} −{}", change.added, change.removed),
                ),
                SessionItem::Run(_, run) => (
                    run.name.clone(),
                    if run.passed {
                        "Run passed".into()
                    } else {
                        "Run failed".into()
                    },
                ),
            };
            let time = if item.instant().is_some() {
                format!("{} UTC", item.time_label())
            } else {
                "Time not recorded".into()
            };
            card(
                model,
                s,
                width,
                title,
                format!("{kind} · {time}"),
                Widget::session_row(row),
                model.selected() == Some(&item.selection()),
            )
        })
        .collect()
}
