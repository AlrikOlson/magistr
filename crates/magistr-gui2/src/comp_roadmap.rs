//! Full native roadmap: dependency graph, work list and independently selected details.
use crate::comp_composer::ComposerComposition;
use crate::comp_roadmap_requests::RequestsComposition;
use crate::comp_sky::SkyComposition;
use crate::comp_workspace::{WorkspaceComposition as W, control, divider, heading, panel};
use crate::comp_workspace_details::DetailsComposition;
use crate::roadmap_layout::RoadmapLayout as L;
use crate::{Component, Element, ElementId, Lexicon, Medium, Voice};
use magistr_cockpit::cockpit::{
    Model, Pane, Place, RoadmapSort, Selected, Widget, flight_divider::FlightDivider as D,
};

pub struct RoadmapComposition;
impl RoadmapComposition {
    pub const BODY: ElementId = ElementId(W::RIGHT.0 + 16);
    pub const PLAN: ElementId = ElementId(Self::BODY.0 + 1);
    pub const GRAPH: ElementId = ElementId(Self::BODY.0 + 2);
    pub const LIST: ElementId = ElementId(Self::BODY.0 + 3);
    pub const REQUESTS: ElementId = ElementId(Self::BODY.0 + 4);
    pub const REQUEST_CONTENT: ElementId = ElementId(Self::BODY.0 + 5);
    pub const INPUT: ElementId = ElementId(Self::BODY.0 + 6);
    pub fn active(model: &Model) -> bool {
        model.place() == Some(Place::Sky) && model.bad_day().is_none()
    }
    pub fn el(model: &Model, medium: Medium, cell: (f32, f32), viewport: (f32, f32)) -> Element {
        let lex = Lexicon::of(medium);
        let l = L::of(model, viewport.0, viewport.1);
        let navigation = Element::row()
            .height(L::NAV)
            .no_shrink()
            .center()
            .child(control("Conversation", Widget::TabTurn.id(), &lex))
            .child(Element::row().grow(1.0))
            .child(control("Commands", Widget::Commands.id(), &lex));
        let tabs = |compact| {
            let mut tabs = Element::row().height(36.0).no_shrink().center().gap(4.0);
            if compact {
                tabs = tabs.child(control("Plan", Widget::TabRoadmap.id(), &lex));
            }
            for (label, widget, selected) in [
                (
                    "Requests",
                    Widget::RoadmapRequests,
                    model.roadmap_requests_open(),
                ),
                (
                    "Item details",
                    Widget::RoadmapDetails,
                    !model.roadmap_requests_open() && (!compact || model.pane() != Pane::Stage),
                ),
            ] {
                let tab = control(label, widget.id(), &lex);
                tabs = tabs.child(if selected {
                    Element::surface().fill(lex.s0).radius(5.0).child(tab)
                } else {
                    tab
                });
            }
            tabs
        };
        let detail = |height| {
            if model.roadmap_requests_open() {
                RequestsComposition::el(model, height, &lex)
            } else {
                DetailsComposition::el(model, height, &lex)
            }
        };
        let plan = |height, graph_height, list_height| {
            Element::surface()
                .width(l.left)
                .height(height)
                .no_shrink()
                .min_height(0.0)
                .id(Self::PLAN)
                .child(
                    panel(graph_height, &lex)
                        .id(Self::GRAPH)
                        .child(
                            heading("Dependencies", &lex)
                                .child(Element::row().grow(1.0))
                                .child(control("Reset view", Widget::RoadmapReset.id(), &lex)),
                        )
                        .child(graph_field(
                            model,
                            medium,
                            cell,
                            ((l.left - 28.0).max(0.0), (graph_height - 36.0).max(0.0)),
                            &lex,
                        )),
                )
                .child(divider(model, D::Graph, &lex))
                .child(work_list(model, list_height, &lex))
        };
        let body = if l.compact {
            let remaining = (l.body - 36.0).max(0.0);
            let content = if model.roadmap_requests_open() || model.pane() == Pane::Inspector {
                detail(remaining)
            } else {
                let graph = (l.graph - 18.0).max(0.0);
                plan(remaining, graph, (remaining - L::GAP - graph).max(0.0))
            };
            Element::surface().child(tabs(true)).child(content)
        } else {
            Element::row()
                .child(plan(l.body, l.graph, l.list))
                .child(divider(model, D::Columns, &lex))
                .child(
                    Element::surface()
                        .width(l.right)
                        .no_shrink()
                        .min_width(0.0)
                        .child(tabs(false))
                        .child(detail((l.body - 36.0).max(0.0))),
                )
        };
        Element::surface()
            .min_height(0.0)
            .pad_x(l.margin)
            .pad_y(l.margin / 2.0)
            .child(navigation)
            .child(body.height(l.body).no_shrink().id(Self::BODY))
            .child(ComposerComposition::sized(
                model,
                medium,
                viewport.0 - 2.0 * l.margin,
                l.composer,
            ))
            .claim(Component::Prose)
    }
}

fn graph_field(
    model: &Model,
    medium: Medium,
    cell: (f32, f32),
    size: (f32, f32),
    lex: &Lexicon,
) -> Element {
    let mut field = Element::surface()
        .grow(1.0)
        .height(0.0)
        .min_height(0.0)
        .id(SkyComposition::FIELD);
    let nodes = model.sky_nodes(model.now());
    let sky = crate::Sky::new(
        magistr_cockpit::layout::PxRect::new(0.0, 0.0, size.0, size.1),
        cell,
        1.0,
        model.theme().palette(),
        medium,
    )
    .panned(model.roadmap().pan());
    for (index, node) in nodes.iter().enumerate() {
        let Some((x, y)) = sky.position(&nodes, index) else {
            continue;
        };
        let Some(chunk) = model.roadmap().graph_chunk(index) else {
            continue;
        };
        if x < 0.0 || y < 0.0 || x > size.0 || y > size.1 {
            continue;
        }
        field = field.child(
            Element::text(
                Voice::HINT,
                if node.selected { lex.tx } else { lex.sb0 },
                &chunk.name,
            )
            .claim(Component::Label)
            .nowrap()
            .width(116.0)
            .pin_left(if x + 128.0 > size.0 {
                (x - 116.0).max(0.0)
            } else {
                x + 12.0
            })
            .pin_top(if x + 128.0 > size.0 {
                y + 10.0
            } else {
                y - 8.0
            })
            .id(W::id(Widget::sky_star(index)))
            .focusable()
            .focus_ring(lex.mauve)
            .label(&chunk.name),
        );
    }
    field
}

fn work_list(model: &Model, height: f32, lex: &Lexicon) -> Element {
    let roadmap = model.roadmap();
    let mut list = panel(height, lex).child(
        heading("Work items", lex)
            .child(Element::row().grow(1.0))
            .child(control(
                &format!("Status: {}", roadmap.filter_label()),
                Widget::RoadmapFilter.id(),
                lex,
            )),
    );
    let mut sort = Element::row().height(30.0).no_shrink().center();
    for column in RoadmapSort::ALL {
        let name = match column {
            RoadmapSort::Workstream => "Group",
            _ => column.label(),
        };
        let suffix = if roadmap.sort() == column {
            if roadmap.descending() { " ↓" } else { " ↑" }
        } else {
            ""
        };
        sort = sort.child(control(&format!("{name}{suffix}"), column.widget().id(), lex).grow(1.0));
    }
    list = list.child(sort);
    let mut rows = Element::surface()
        .grow(1.0)
        .height(0.0)
        .min_height(0.0)
        .id(RoadmapComposition::LIST);
    let chunks = roadmap.rows();
    let total = chunks.len() + roadmap.signals().len();
    if total == 0 {
        rows = rows.child(
            Element::text(Voice::STEER, lex.sb0, "No work matches this status.").min_width(0.0),
        );
    }
    let capacity = ((height - 66.0).max(0.0) / L::ROW).floor().max(1.0) as usize;
    for (index, chunk) in chunks
        .iter()
        .enumerate()
        .skip(model.roadmap_top())
        .take(capacity)
    {
        let title = if !chunk.title.trim().is_empty() {
            &chunk.title
        } else {
            &chunk.name
        };
        let selected = model.selected() == Some(&Selected::Chunk(chunk.id.clone()));
        let status = format!(
            "{} · {}{}",
            if chunk.blocker.is_some() {
                "blocked".to_owned()
            } else {
                chunk.state.word().replace('_', " ")
            },
            chunk.band(),
            if chunk.proposals.is_empty() {
                ""
            } else {
                " · Suggested change"
            }
        );
        rows = rows.child(work_row(
            title,
            &status,
            W::id(Widget::roadmap_row(index)),
            selected,
            lex,
        ));
    }
    let start = model.roadmap_top().saturating_sub(chunks.len());
    let available = capacity.saturating_sub(
        chunks
            .len()
            .saturating_sub(model.roadmap_top())
            .min(capacity),
    );
    for (index, signal) in roadmap
        .signals()
        .iter()
        .enumerate()
        .skip(start)
        .take(available)
    {
        rows = rows.child(work_row(
            signal.title(),
            "Feedback",
            W::id(Widget::signal_row(index)),
            model.selected() == Some(&Selected::Signal(signal.id.clone())),
            lex,
        ));
    }
    list.child(rows)
}

fn work_row(title: &str, status: &str, id: ElementId, selected: bool, lex: &Lexicon) -> Element {
    let row = Element::surface()
        .height(L::ROW)
        .no_shrink()
        .justify_center()
        .min_width(0.0)
        .child(
            Element::text(Voice::LEDGER_NAME, lex.tx, title)
                .nowrap()
                .claim(Component::Title),
        )
        .child(
            Element::text(Voice::HINT, lex.sb0, status)
                .claim(Component::Label)
                .nowrap(),
        )
        .id(id)
        .focusable()
        .focus_ring(lex.mauve)
        .label(title);
    if selected { row.fill(lex.s0) } else { row }
}
