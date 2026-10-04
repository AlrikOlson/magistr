//! Graphics geometry for the same selected document used by cells and native views.
use super::Inspector;
use crate::cockpit::session_item::SessionItem;
use crate::cockpit::{FlightStyle, Inspection, Model, Pane, Selected, ShipMoment, Widget};
use crate::layout::{Length, Node};
use crate::pixel::Typography;
use crate::theme::Pen;

impl Inspector {
    /// A linked work name and the exact recorded object it opens.
    #[must_use]
    pub fn linked_work(model: &Model, number: u32) -> Option<(String, Selected)> {
        let reference = model.trace().step(number)?.execution_ref.as_deref()?;
        Self::work_reference(model, reference).or_else(|| {
            model
                .roadmap()
                .chunk_for_ref(reference)
                .and_then(|chunk| Self::work_reference(model, &chunk.id))
        })
    }

    /// Resolve a recorded reference without guessing from a display name.
    pub(in crate::cockpit) fn work_reference(
        model: &Model,
        reference: &str,
    ) -> Option<(String, Selected)> {
        if let Some(number) = reference
            .strip_prefix("think:")
            .or_else(|| reference.strip_prefix("step:"))
        {
            let number = number.parse().ok()?;
            let step = model.trace().step(number)?;
            return Some((
                format!("Step {number} — {}", step.purpose),
                Selected::Step(number),
            ));
        }
        if let Some(id) = reference.strip_prefix("task:")
            && let Some(task) = model.rail().ship().task(id)
        {
            return Some((task.title.clone(), Selected::Task(task.id.clone())));
        }
        if let Some(id) = reference.strip_prefix("signal:")
            && let Some(signal) = model.roadmap().inbox().iter().find(|s| s.id == id)
        {
            return Some((signal.title().to_owned(), Selected::Signal(id.to_owned())));
        }
        let mut matches = SessionItem::collect(model.trace(), model.changes())
            .into_iter()
            .filter_map(|item| {
                let selection = item.selection();
                let Selected::Receipt(identity) = &selection else {
                    return None;
                };
                let (label, aliases) = match item {
                    SessionItem::Moment(
                        _,
                        ShipMoment::Action {
                            task_id,
                            id,
                            description,
                            ..
                        },
                    ) => (
                        description.clone(),
                        vec![format!("action:{task_id}:{id}"), format!("action:{id}")],
                    ),
                    SessionItem::Moment(_, ShipMoment::Check { task_id, name, .. }) => (
                        name.clone(),
                        vec![format!("check:{task_id}:{name}"), format!("check:{name}")],
                    ),
                    _ => return None,
                };
                (reference == identity || aliases.iter().any(|alias| alias == reference))
                    .then_some((label, selection))
            });
        let first = matches.next();
        if first.is_some() && matches.next().is_none() {
            return first;
        }
        model
            .roadmap()
            .chunk(reference.strip_prefix("chunk:").unwrap_or(reference))
            .map(|chunk| {
                (
                    if chunk.title.is_empty() {
                        chunk.name.clone()
                    } else {
                        chunk.title.clone()
                    },
                    Selected::Chunk(chunk.id.clone()),
                )
            })
    }

    #[must_use]
    pub fn flight_view(model: &Model, width: f32, height: f32) -> Node {
        let style = FlightStyle::for_model(model);
        let Some(card) = Inspection::of(model) else {
            return style
                .outlined(
                    style.inset(style.prose("Select a record to see details."), 16.0, 12.0),
                    style.panel,
                    style.line,
                )
                .id(Widget::Inspector.id());
        };
        let mut face = Typography::default();
        face.set_cell(model.cell_size());
        let rows = card.rows(
            model,
            (width - 34.0).max(1.0),
            u64::from(model.cell_size().width) << 32 | u64::from(model.cell_size().height),
            &mut |text, role| face.advance(text, role),
        );
        let visible = ((height - 48.0 - 3.0 * style.line_height).max(0.0)
            / style.line_height.max(1.0)) as usize;
        let top = Self::top(model, &rows, visible);
        let pen = Pen::for_theme(&model.theme());
        let fields = Node::column()
            .id(Widget::InspectorFields.id())
            .viewport()
            .grow(1.0)
            .min_height(Length::Px(0.0))
            .children(rows.iter().skip(top).take(visible).map(|r| {
                let mut node = Self::paint_row(r, model, &pen)
                    .height(Length::Px(style.line_height))
                    .shrink(0.0);
                node.restyle_all(&|s| s.clone().bg(style.panel));
                node
            }));
        let mut actions = Node::row().gap(8);
        for (label, id) in &card.actions {
            actions = actions.child(style.label(*label, style.accent).id(*id));
        }
        if card.viewer {
            actions = actions.child(
                style
                    .label("Close", style.accent)
                    .id(Widget::InspectorDetail.id()),
            );
        }
        let body = Node::column()
            .grow(1.0)
            .min_height(Length::Px(0.0))
            .child(
                crate::cockpit::flight_records::document_summary(
                    model,
                    &card.title_document(model),
                    (width - 34.0).max(1.0),
                    style.ink,
                )
                .id(Widget::InspectorTitle.id()),
            )
            .child(style.label(format!("{} · {}", card.status, card.metadata), style.muted))
            .child(fields)
            .child(actions.height(Length::Px(style.line_height)).shrink(0.0));
        let content = Node::column()
            .min_height(Length::Px(0.0))
            .child(style.section_heading(
                "Details",
                style.label("Copy", style.accent).id(Widget::detail_copy()),
            ))
            .child(style.inset(body, 16.0, 6.0).grow(1.0));
        let mut panel = style
            .outlined(
                content,
                style.panel,
                if model.pane() == Pane::Inspector {
                    style.accent
                } else {
                    style.line
                },
            )
            .id(Widget::Inspector.id())
            .min_height(Length::Px(0.0));
        if let Some(id) = model.hovered()
            && crate::cockpit::HoverRole::of(id).is_clickable()
        {
            panel.restyle_at(id, &|s| s.clone().bg(style.selected));
        }
        panel
    }

    pub fn flight_scroll(model: &mut Model, delta: isize) {
        let Some(card) = Inspection::of(model) else {
            return;
        };
        let style = FlightStyle::for_model(model);
        let layout = model.flight_layout();
        let (width, height) = if model.tab() == crate::cockpit::StageTab::Roadmap {
            (
                if layout.compact {
                    layout.width
                } else {
                    layout.right_width
                },
                crate::cockpit::FlightRoadmapPage::detail_height(model),
            )
        } else if layout.compact {
            (layout.width, layout.body_height)
        } else {
            (layout.right_width, layout.detail_height)
        };
        let mut face = Typography::default();
        face.set_cell(model.cell_size());
        let rows = card.rows(
            model,
            (width - 34.0).max(1.0),
            u64::from(model.cell_size().width) << 32 | u64::from(model.cell_size().height),
            &mut |text, role| face.advance(text, role),
        );
        let visible = ((height - 48.0 - 3.0 * style.line_height).max(0.0)
            / style.line_height.max(1.0)) as usize;
        let top = Self::top(model, &rows, visible);
        model.inspector_scroll = top
            .saturating_add_signed(delta)
            .min(rows.len().saturating_sub(visible));
        model.inspection_state.borrow_mut().anchor =
            rows.get(model.inspector_scroll).map(|r| r.position.clone());
    }
}

#[cfg(test)]
#[path = "flight_inspector.tests.rs"]
mod tests;
