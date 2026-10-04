//! The selected document's controls and cell geometry change together.
use super::space::Space;
use super::{Inspection, Model, Pane, ReadingPosition, ReadingRow, RecordTarget, Widget};
use crate::input::{KeyCode, KeyEvent, Modifiers};
use crate::layout::{Frame, Length, Node, TextMetrics};
use crate::theme::Pen;
use crate::widget_id::WidgetId;
use std::rc::Rc;

#[path = "flight_inspector.rs"]
mod flight;

/// One document reader, with independent cell, graphics and native geometry.
pub struct Inspector;

impl Inspector {
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
        panel = if model.is_roadmap_drilldown() {
            panel
                .grow(1.0)
                .child(line("Back to roadmap (Esc)", &pen.link).id(Widget::TabRoadmap.id()))
        } else if model.inspector_stacked() {
            panel
                .height(Length::Cells(model.inspector_stack_rows()))
                .shrink(0.0)
        } else {
            panel
                .width(Length::Cells(Space::INSPECTOR_COLS))
                .shrink(0.0)
        };
        let Some(card) = Inspection::of(model) else {
            return panel.child(line("Select a record to see details.", &pen.note));
        };
        let rows = Self::cell_rows(model, &card);
        let height = Self::field_height(model);
        let top = Self::top(model, &rows, height);
        let mut actions = Node::row().gap(Space::TIGHT);
        for (label, id) in &card.actions {
            actions = actions.child(line(label, &pen.link).id(*id));
        }
        actions = actions.child(line("Copy", &pen.link).id(Widget::detail_copy()));
        if card.viewer {
            actions = actions.child(line("Close", &pen.link).id(Widget::InspectorDetail.id()));
        }
        let body = Node::column()
            .grow(1.0)
            .min_width(Length::Cells(0))
            .viewport()
            .child(line(&card.title, &pen.heading).id(Widget::InspectorTitle.id()))
            .child(
                super::marked::Marked::new(model, pen)
                    .icon(card.icon)
                    .text(" ")
                    .text(&card.status)
                    .node()
                    .style(pen.link.clone())
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
                    .children(
                        rows.iter()
                            .skip(top)
                            .take(height)
                            .map(|r| Self::paint_row(r, model, pen)),
                    ),
            );
        panel.child(
            Node::row()
                .grow(1.0)
                .viewport()
                .child(Node::leaf().width(Length::Cells(Space::PAD)).shrink(0.0))
                .child(body)
                .child(Node::leaf().width(Length::Cells(Space::PAD)).shrink(0.0)),
        )
    }

    fn paint_row(row: &ReadingRow, model: &Model, pen: &Pen) -> Node {
        let mut node = super::view::stage_row(&row.row, model, pen, None);
        if let Some(id) = row.target {
            node = node.id(id);
            node.restyle_all(&|s| s.clone().fg(pen.link.fg));
            if model
                .inspection_state
                .borrow()
                .focused_link
                .is_some_and(|index| Widget::detail_link(index) == id)
            {
                node.restyle_all(&|s| s.clone().attrs(crate::render::Attrs::UNDERLINE));
            }
        }
        node
    }

    fn cell_rows(model: &Model, card: &Inspection) -> Rc<Vec<ReadingRow>> {
        card.rows(
            model,
            f32::from(model.inspector_width().max(1)),
            0,
            &mut |s, _| f32::from(TextMetrics::new(s).max_content_width()),
        )
    }

    fn top(model: &Model, rows: &[ReadingRow], height: usize) -> usize {
        let top = model
            .inspection_state
            .borrow()
            .anchor
            .as_ref()
            .map_or(model.inspector_scroll, |position| {
                Inspection::row_of(rows, position)
            });
        top.min(rows.len().saturating_sub(height))
    }

    /// All controls in the document belong to its scrolling and focus region.
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
        ) || Widget::detail_link_index(id).is_some()
            || Widget::detail_section_index(id).is_some()
            || id == Widget::detail_copy()
            || Widget::detail_block_index(id).is_some()
            || Widget::trace_pin_step(id).is_some()
            || Widget::trace_link_step(id).is_some()
            || Widget::change_open_index(id).is_some()
            || Widget::change_undo_index(id).is_some()
            || Widget::change_approve_index(id).is_some()
            || Widget::proposal_accept_index(id).is_some()
            || Widget::proposal_reject_index(id).is_some()
            || Widget::signal_promote_index(id).is_some()
            || Widget::signal_ignore_index(id).is_some()
            || Widget::signal_snooze_index(id).is_some()
    }

    pub(super) fn activate(model: &mut Model, id: WidgetId) -> Option<Vec<super::Cmd>> {
        let card = Inspection::of(model)?;
        if id == Widget::detail_copy() {
            return Some(vec![super::Cmd::Clipboard(card.original())]);
        }
        if let Some(index) = Widget::detail_section_index(id) {
            let section = card.sections.get(index)?;
            let mut state = model.inspection_state.borrow_mut();
            if !state.expanded.remove(&section.key) {
                state.expanded.insert(section.key.clone());
            }
            state.anchor = Some(ReadingPosition {
                section: section.key.clone(),
                source: Default::default(),
            });
            return Some(Vec::new());
        }
        if let Some(index) = Widget::detail_link_index(id) {
            let link = card
                .links()
                .into_iter()
                .find(|link| link.widget == Widget::detail_link(index))?;
            return Some(match &link.target {
                RecordTarget::Record(selected) => {
                    super::nav::open_record(model, selected.clone());
                    Vec::new()
                }
                RecordTarget::Address(address) => vec![super::Cmd::Clipboard(address.clone())],
                RecordTarget::Unavailable => Vec::new(),
            });
        }
        None
    }

    /// Reading keys work while an unsent message remains untouched.
    pub(super) fn key(model: &mut Model, key: KeyEvent) -> Option<Vec<super::Cmd>> {
        if model.pane() != Pane::Inspector {
            return None;
        }
        let card = Inspection::of(model)?;
        if key.mods == Modifiers::ALT && matches!(key.code, KeyCode::Up | KeyCode::Down) {
            let current = model
                .inspection_state
                .borrow()
                .anchor
                .as_ref()
                .and_then(|at| card.sections.iter().position(|s| s.key == at.section))
                .unwrap_or(0);
            let next = if key.code == KeyCode::Down {
                (current + 1).min(card.sections.len().saturating_sub(1))
            } else {
                current.saturating_sub(1)
            };
            if let Some(section) = card.sections.get(next) {
                model.inspection_state.borrow_mut().anchor = Some(ReadingPosition {
                    section: section.key.clone(),
                    source: Default::default(),
                });
            }
            return Some(Vec::new());
        }
        if !key.mods.is_empty() {
            return None;
        }
        match key.code {
            KeyCode::Left | KeyCode::Right => {
                let links = card
                    .sections
                    .iter()
                    .filter(|s| card.is_expanded(model, s))
                    .flat_map(|s| &s.links)
                    .filter(|l| l.target != RecordTarget::Unavailable)
                    .collect::<Vec<_>>();
                if links.is_empty() {
                    return Some(Vec::new());
                }
                let current = model.inspection_state.borrow().focused_link.and_then(|i| {
                    links
                        .iter()
                        .position(|l| l.widget == Widget::detail_link(i))
                });
                let at = match current {
                    Some(i) if key.code == KeyCode::Right => (i + 1) % links.len(),
                    Some(i) => (i + links.len() - 1) % links.len(),
                    None => 0,
                };
                model.inspection_state.borrow_mut().focused_link =
                    Widget::detail_link_index(links[at].widget);
                if let Some(section) = card
                    .sections
                    .iter()
                    .find(|s| s.links.iter().any(|l| l.widget == links[at].widget))
                {
                    model.inspection_state.borrow_mut().anchor = Some(ReadingPosition {
                        section: section.key.clone(),
                        source: Default::default(),
                    });
                }
                Some(Vec::new())
            }
            KeyCode::Enter => {
                let focused = model.inspection_state.borrow().focused_link;
                if let Some(index) = focused {
                    return Self::activate(model, Widget::detail_link(index));
                }
                let current = model
                    .inspection_state
                    .borrow()
                    .anchor
                    .as_ref()
                    .and_then(|at| card.sections.iter().position(|s| s.key == at.section))
                    .or_else(|| card.sections.iter().position(|s| s.secondary))?;
                Self::activate(model, Widget::detail_section(current))
            }
            KeyCode::Up
            | KeyCode::Down
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Home
            | KeyCode::End => {
                let delta = match key.code {
                    KeyCode::Home => isize::MIN,
                    KeyCode::End => isize::MAX,
                    KeyCode::Up => -1,
                    KeyCode::Down => 1,
                    KeyCode::PageUp => -(model.stage_rows().max(1) as isize),
                    _ => model.stage_rows().max(1) as isize,
                };
                Self::scroll(model, delta);
                Some(Vec::new())
            }
            _ => None,
        }
    }

    pub fn scroll(model: &mut Model, delta: isize) {
        if model.surface().is_native()
            && let Some((total, visible)) = model.flight_panels.native_details
        {
            model.inspector_scroll = model
                .inspector_scroll
                .saturating_add_signed(
                    delta.saturating_mul(isize::try_from(model.cell_size().height).unwrap_or(1)),
                )
                .min(total.saturating_sub(visible));
            let mut state = model.inspection_state.borrow_mut();
            state.anchor = state
                .native_positions
                .iter()
                .take_while(|(_, y)| *y <= model.inspector_scroll)
                .last()
                .map(|(at, _)| at.clone());
            return;
        }
        if model.has_pixels() {
            Self::flight_scroll(model, delta);
            return;
        }
        let Some(card) = Inspection::of(model) else {
            return;
        };
        let rows = Self::cell_rows(model, &card);
        let top = Self::top(model, &rows, Self::field_height(model));
        model.inspector_scroll = top
            .saturating_add_signed(delta)
            .min(rows.len().saturating_sub(Self::field_height(model)));
        model.inspection_state.borrow_mut().anchor =
            rows.get(model.inspector_scroll).map(|r| r.position.clone());
    }

    /// Native layout reports its own source positions after font layout.
    pub fn report_native_positions(model: &mut Model, positions: Vec<(ReadingPosition, usize)>) {
        let mut state = model.inspection_state.borrow_mut();
        if positions == state.native_positions {
            return;
        }
        if let Some(anchor) = &state.anchor {
            let mut same = positions
                .iter()
                .filter(|(p, _)| p.section == anchor.section);
            let target = if let Some(source) = anchor.source {
                same.rfind(|(p, _)| {
                    p.source.is_some_and(|at| {
                        at.line < source.line || at.line == source.line && at.byte <= source.byte
                    })
                })
            } else {
                same.next()
            };
            if let Some((_, y)) = target {
                model.inspector_scroll = *y;
            }
        }
        state.native_positions = positions;
    }

    fn field_height(model: &Model) -> usize {
        if model.is_roadmap_drilldown() {
            model.stage_rows().saturating_sub(5)
        } else if model.inspector_stacked() {
            usize::from(model.inspector_stack_rows()).saturating_sub(6)
        } else {
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

fn line(text: &str, style: &crate::render::Style) -> Node {
    Node::text(text)
        .style(style.clone())
        .height(Length::Cells(1))
        .shrink(0.0)
}

#[cfg(test)]
#[path = "inspector.tests.rs"]
mod tests;
