//! Native conversation, reasoning, nearby work and details in one workspace.
use crate::comp_composer::ComposerComposition;
use crate::comp_feed::FeedComposition;
use crate::comp_root::RootComposition;
use crate::comp_term_dock::TermDockComposition;
use crate::comp_workspace_details::DetailsComposition;
use crate::comp_workspace_reasoning::ReasoningComposition;
use crate::workspace_layout::WorkspaceLayout as L;
use crate::{Element, ElementId, Lexicon, Medium, Voice};
use magistr_cockpit::cockpit::{
    Model, Place, Selected, Widget, flight_divider::FlightDivider as D,
};
use magistr_cockpit::widget_id::WidgetId;

pub struct WorkspaceComposition;

impl WorkspaceComposition {
    // Between the charter's ranges and the feed's heads; reserve all WidgetId values.
    const BASE: u64 = 1 << 38;
    pub const BODY: ElementId = ElementId(Self::BASE + (1 << 32));
    pub const CONVERSATION: ElementId = ElementId(Self::BODY.0 + 1);
    pub const REASONING: ElementId = ElementId(Self::BODY.0 + 2);
    pub const NEARBY: ElementId = ElementId(Self::BODY.0 + 3);
    pub const DETAILS: ElementId = ElementId(Self::BODY.0 + 4);
    pub const DETAIL_CONTENT: ElementId = ElementId(Self::BODY.0 + 5);
    pub const LEFT: ElementId = ElementId(Self::BODY.0 + 6);
    pub const RIGHT: ElementId = ElementId(Self::BODY.0 + 7);

    pub fn active(model: &Model) -> bool {
        model.place() == Some(Place::Session)
    }

    pub const fn id(widget: WidgetId) -> ElementId {
        ElementId(Self::BASE + widget.raw() as u64)
    }

    pub fn target(id: ElementId) -> Option<WidgetId> {
        let raw = id.0.checked_sub(Self::BASE)?;
        u32::try_from(raw).ok().map(WidgetId::new)
    }

    pub fn el(model: &Model, medium: Medium, cell: (f32, f32), viewport: (f32, f32)) -> Element {
        let lex = Lexicon::of(medium);
        let l = L::of(model, viewport.0, viewport.1);
        let mut left = Element::surface()
            .width(l.left)
            .min_width(0.0)
            .no_shrink()
            .id(Self::LEFT)
            .child(conversation(model, medium, cell, l.conversation, &lex));
        if model.reasoning_expanded() {
            left = left.child(divider(model, D::Reasoning, &lex));
        }
        left = left.child(ReasoningComposition::el(model, l.reasoning, &lex));
        let body = if l.compact {
            Element::surface()
                .child(left)
                .child(Element::surface().height(L::GAP))
                .child(DetailsComposition::el(model, l.details, &lex))
        } else {
            let right = Element::surface()
                .width(l.right)
                .min_width(0.0)
                .no_shrink()
                .id(Self::RIGHT)
                .child(nearby(model, l.nearby, &lex))
                .child(divider(model, D::Roadmap, &lex))
                .child(DetailsComposition::el(model, l.details, &lex));
            Element::row()
                .child(left)
                .child(divider(model, D::Columns, &lex))
                .child(right)
        };
        let mut workspace = Element::surface();
        if l.status > 0.0 {
            let mut band = Element::row()
                .center()
                .height(l.status)
                .no_shrink()
                .child(crate::comp_band::BandComposition::el(model, medium));
            if let Some(breath) = crate::breath::Breath::of(model) {
                band = band.child(breath.dot(medium, &lex).pin_left(-14.0).pin_top(10.0));
            }
            workspace = workspace.child(band);
        }
        workspace
            .pad_x(l.margin)
            .pad_y(l.margin / 2.0)
            .min_height(0.0)
            .child(body.height(l.body).no_shrink().id(Self::BODY))
            .child(ComposerComposition::sized(
                model,
                medium,
                viewport.0 - 2.0 * l.margin,
                l.composer,
            ))
    }
}

fn conversation(
    model: &Model,
    medium: Medium,
    cell: (f32, f32),
    height: f32,
    lex: &Lexicon,
) -> Element {
    let mut feed = FeedComposition::el(model, medium, cell)
        .grow(1.0)
        .height(0.0)
        .min_height(0.0)
        .stretch()
        .id(RootComposition::STAGE);
    if RootComposition::feed_at_its_tail(model) {
        feed = feed.justify_end();
    }
    // The work this window can start or steer sits ahead of the two destinations: the
    // starts while no loop is live, the loop's verbs while one is.
    let mut head = heading("Conversation", lex).child(Element::row().grow(1.0));
    for (widget, label, _) in crate::work_controls::WorkControls::verbs(model) {
        head = head.child(control(label, widget.id(), lex));
    }
    let mut panel = panel(height, lex)
        .id(WorkspaceComposition::CONVERSATION)
        .child(
            head.child(control("Roadmap", Widget::TabRoadmap.id(), lex))
                .child(control("Commands", Widget::Commands.id(), lex)),
        )
        .child(feed);
    if let Some(dock) = TermDockComposition::el(model, medium) {
        panel = panel.child(dock);
    }
    panel
}

pub(crate) fn panel(height: f32, lex: &Lexicon) -> Element {
    Element::surface()
        .height(height)
        .no_shrink()
        .min_height(0.0)
        .min_width(0.0)
        .fill(lex.mantle)
        .radius(9.0)
        .pad_x(14.0)
}

pub(crate) fn heading(title: &str, lex: &Lexicon) -> Element {
    Element::row().height(36.0).no_shrink().center().child(
        Element::text(Voice::LEDGER_NAME, lex.tx, title)
            .claim(crate::Component::Title)
            .nowrap(),
    )
}

pub(crate) fn control(label: &str, widget: WidgetId, lex: &Lexicon) -> Element {
    Element::text(Voice::HINT, lex.mauve, label)
        .claim(crate::Component::Button)
        .nowrap()
        .pad_x(6.0)
        .pad_y(5.0)
        .id(WorkspaceComposition::id(widget))
        .focusable()
        .focus_ring(lex.mauve)
        .label(label)
}

pub(crate) fn divider(model: &Model, divider: D, lex: &Lexicon) -> Element {
    let widget = divider.widget();
    let active = model.active_panel() == Some(divider) || model.hovered() == Some(widget.id());
    let ink = if active { lex.mauve } else { lex.ov0 };
    let columns = divider == D::Columns;
    let grip = Element::surface()
        .width(if columns { 2.0 } else { 30.0 })
        .height(if columns { 30.0 } else { 2.0 })
        .fill(ink);
    let area = if columns {
        Element::surface().width(L::GAP).stretch()
    } else {
        Element::row().height(L::GAP).stretch()
    };
    area.no_shrink()
        .center()
        .justify_center()
        .child(grip)
        .id(WorkspaceComposition::id(widget.id()))
        .focusable()
        .focus_ring(lex.mauve)
        .label(widget.label())
}

fn nearby(model: &Model, height: f32, lex: &Lexicon) -> Element {
    let rows = model.roadmap().rows();
    let head = heading("Nearby work", lex);
    let mut panel = panel(height, lex)
        .id(WorkspaceComposition::NEARBY)
        .child(head);
    if rows.is_empty() {
        return panel.child(
            Element::text(Voice::STEER, lex.sb0, "No work planned yet.")
                .claim(crate::Component::Prose),
        );
    }
    let selected = model.roadmap().selected();
    let current = selected
        .or_else(|| rows.iter().copied().find(|c| c.state.is_working()))
        .or_else(|| model.roadmap().next_ready());
    let mut nearby: Vec<_> = rows.iter().enumerate().collect();
    nearby.sort_by_key(|(_, chunk)| {
        if current.is_some_and(|c| c.id == chunk.id) {
            0
        } else if current.is_some_and(|c| c.deps.contains(&chunk.id) || chunk.deps.contains(&c.id))
        {
            1
        } else {
            2
        }
    });
    let capacity = ((height - 40.0).max(0.0) / 48.0) as usize;
    for (index, chunk) in nearby.into_iter().take(capacity) {
        let title = if !chunk.title.trim().is_empty() {
            &chunk.title
        } else if !chunk.name.trim().is_empty() && chunk.name != chunk.id {
            &chunk.name
        } else {
            "Untitled work item"
        };
        let selected = model.selected() == Some(&Selected::Chunk(chunk.id.clone()));
        let row = Element::surface()
            .height(48.0)
            .no_shrink()
            .justify_center()
            .child(
                Element::text(Voice::LEDGER_NAME, lex.tx, title)
                    .claim(crate::Component::Title)
                    .nowrap(),
            )
            .child(
                Element::text(Voice::HINT, lex.sb0, chunk.state.word().replace('_', " ")).nowrap(),
            )
            .id(WorkspaceComposition::id(Widget::roadmap_row(index)))
            .focusable()
            .focus_ring(lex.mauve)
            .label(title);
        panel = panel.child(if selected { row.fill(lex.s0) } else { row });
    }
    panel.claim(crate::Component::StatusRow)
}
