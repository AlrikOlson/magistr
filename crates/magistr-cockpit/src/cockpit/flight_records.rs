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
    pub fn track_row_height(model: &Model) -> f32 {
        f32::from(model.cell_size().height) * 3.0 + 12.0
    }
    #[must_use]
    pub fn shown(model: &Model, height: f32, roadmap: bool) -> usize {
        let line = f32::from(model.cell_size().height);
        let controls = if roadmap { controls_height(model) } else { 0.0 };
        ((height - (line + 14.0).max(34.0) - 22.0 - controls).max(0.0)
            / if roadmap {
                Self::track_row_height(model)
            } else {
                Self::row_height(model)
            })
        .floor() as usize
    }
    #[must_use]
    pub fn view(model: &Model, width: f32, height: f32) -> Node {
        let s = S::for_model(model);
        let roadmap = model.tab() == StageTab::Roadmap;
        let title = if roadmap { "Tracks" } else { "Recorded work" };
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
        let top = if roadmap { 0 } else { model.session_top() };
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
    let compact = model.flight_layout().compact;
    let chip = |label: String, widget: Widget| {
        let active = widget == Widget::RoadmapTracks && !roadmap.is_dependencies_view()
            || widget == Widget::RoadmapDependencies && roadmap.is_dependencies_view();
        s.chip(label, s.accent, if active { s.selected } else { s.panel })
            .id(widget.id())
    };
    let status = || {
        chip(
            format!("Status: {}", roadmap.filter_label()),
            Widget::RoadmapFilter,
        )
    };
    let sorts = || {
        RoadmapSort::ALL
            .into_iter()
            .filter(|c| *c != RoadmapSort::Workstream)
            .map(|column| {
                let suffix = if roadmap.sort() == column {
                    if roadmap.descending() { " ↓" } else { " ↑" }
                } else {
                    ""
                };
                chip(format!("{}{suffix}", column.label()), column.widget())
            })
    };
    let mut navigation = Node::row()
        .align_items(Align::Center)
        .child(chip("Tracks".into(), Widget::RoadmapTracks))
        .child(chip("Dependencies".into(), Widget::RoadmapDependencies))
        .child(chip(
            roadmap.track_filter_label().into(),
            Widget::RoadmapTrackFilter,
        ));
    if compact {
        navigation = navigation.child(status());
    }
    let mut search = Node::row()
        .align_items(Align::Center)
        .child(chip(
            if roadmap.track_query().is_empty() {
                "Search roadmap…".into()
            } else {
                format!("Search: {}", roadmap.track_query())
            },
            Widget::RoadmapSearch,
        ))
        .child(chip("Clear".into(), Widget::RoadmapSearchClear));
    let mut root = Node::column().shrink(0.0).child(navigation);
    if compact {
        search = search.children(sorts());
    } else {
        root = root.child(
            Node::row()
                .align_items(Align::Center)
                .child(status())
                .children(sorts()),
        );
    }
    if !roadmap.is_selected_visible() {
        search = search.child(chip("Reveal selected item".into(), Widget::RoadmapReveal));
    }
    root = root.child(search);
    root.child(S::vspace(12.0))
}

fn controls_height(model: &Model) -> f32 {
    let rows = if model.flight_layout().compact {
        2.0
    } else {
        3.0
    };
    (f32::from(model.cell_size().height) + 4.0) * rows + 12.0
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
                    summary(
                        model,
                        &title,
                        width - 56.0,
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
pub(super) fn summary(model: &Model, text: &str, width: f32, ink: crate::render::Color) -> Node {
    let doc = crate::turn::TextDocument::markdown(text, model.theme().mode());
    document_summary(model, &doc, width, ink)
}
pub(super) fn document_summary(
    model: &Model,
    doc: &crate::turn::TextDocument,
    width: f32,
    ink: crate::render::Color,
) -> Node {
    let mut face = crate::pixel::Typography::default();
    face.set_cell(model.cell_size());
    let row = doc.summary(width.max(1.0), &mut |text, role| face.advance(text, role));
    let pen = crate::theme::Pen::for_theme(&model.theme());
    let mut node = super::view::stage_row(&row, model, &pen, None);
    node.restyle_all(&|style| style.clone().fg(ink));
    node
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
    use super::{Marked, TrackRow};
    use crate::{pixel::Icon, theme::Pen};
    let pen = Pen::for_theme(&model.theme());
    let roadmap = model.roadmap();
    let rows = roadmap.track_rows();
    if rows.is_empty() {
        return vec![s.prose("No work matches. Change the status or search to see more.")];
    }
    let shown =
        FlightRecords::shown(model, super::FlightRoadmapPage::list_height(model), true).max(1);
    rows.into_iter()
        .enumerate()
        .skip(model.roadmap_top())
        .take(shown)
        .map(|(index, row)| {
            let focused = Some(&row.focus()) == roadmap.track_focus();
            let (title, note, selected, fold) = match &row {
                TrackRow::Heading {
                    expanded, counts, ..
                } => {
                    let lines = counts.lines();
                    (
                        row.label().to_owned(),
                        format!("{}\n{}", lines[0], lines[1]),
                        false,
                        Some(Icon::fold(*expanded)),
                    )
                }
                TrackRow::Item(chunk, _) => (
                    format!("  {}", chunk.name),
                    format!("{} · {}", roadmap.status_label(chunk), chunk.band()),
                    model.selected() == Some(&Selected::Chunk(chunk.id.clone())),
                    None,
                ),
                TrackRow::Signal(_, signal) => (
                    signal.title().into(),
                    "Feedback".into(),
                    model.selected() == Some(&Selected::Signal(signal.id.clone())),
                    None,
                ),
            };
            let title = fit(model, &title, width - 56.0);
            let mut marked = Marked::new(model, &pen).text(if focused { "> " } else { "" });
            if let Some(icon) = fold {
                marked = marked.icon(icon).text(" ");
            }
            let mut body = Node::column().child(
                marked
                    .text(&title)
                    .node()
                    .style(s.style(if focused { s.accent } else { s.ink }))
                    .height(Length::Px(s.line_height))
                    .viewport(),
            );
            for line in note.lines() {
                body = body.child(s.label(fit(model, line, width - 56.0), s.muted));
            }
            s.ground(
                s.inset(body, 10.0, 6.0),
                if selected { s.selected } else { s.panel },
            )
            .id(row.widget(index))
            .height(Length::Px(FlightRecords::track_row_height(model)))
            .shrink(0.0)
        })
        .collect()
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
