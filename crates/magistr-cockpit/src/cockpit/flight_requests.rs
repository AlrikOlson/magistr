//! Request geometry and painting consume the shared request document.
use super::{FlightStyle as S, Model, RequestHistory, Widget};
use crate::layout::{Length, Node};
use crate::pixel::Typography;
use crate::theme::Pen;

/// A request and its replies stay beside the plan with independent reading state.
pub struct FlightRequests;

impl FlightRequests {
    fn ruler(model: &Model) -> u64 {
        1 + (u64::from(model.cell_size().width) << 16) + u64::from(model.cell_size().height)
    }
    fn cell_visible(model: &Model) -> usize {
        if !model.is_roadmap_drilldown() && model.inspector_stacked() {
            usize::from(model.inspector_stack_rows()).saturating_sub(4)
        } else {
            model.stage_rows().saturating_sub(4)
        }
    }
    #[must_use]
    pub fn cell_view(model: &Model, pen: &Pen) -> Node {
        let rows = RequestHistory::of(model).rows(
            f32::from(model.inspector_width().max(1)),
            0,
            &mut |text, _| f32::from(crate::render::width(text)),
        );
        let visible = Self::cell_visible(model);
        let top = RequestHistory::top(model, &rows, visible);
        let mut panel = Node::column()
            .id(Widget::RequestBody.id())
            .style(pen.stage.clone())
            .viewport()
            .min_width(Length::Cells(0))
            .frame(
                crate::layout::Frame::new(
                    pen.panel(model.pane() == super::Pane::Inspector).clone(),
                )
                .title("Requests"),
            );
        panel = if model.is_roadmap_drilldown() {
            panel.grow(1.0)
        } else if model.inspector_stacked() {
            panel
                .height(Length::Cells(model.inspector_stack_rows()))
                .shrink(0.0)
        } else {
            panel
                .width(Length::Cells(super::space::Space::INSPECTOR_COLS))
                .shrink(0.0)
        };
        let controls = Node::row()
            .child(
                Node::text("Copy ")
                    .style(pen.link.clone())
                    .id(Widget::request_copy()),
            )
            .child(
                Node::text("Previous ")
                    .style(pen.link.clone())
                    .id(Widget::request_jump(0)),
            )
            .child(
                Node::text("Next ")
                    .style(pen.link.clone())
                    .id(Widget::request_jump(1)),
            )
            .child(
                Node::text("Latest")
                    .style(pen.link.clone())
                    .id(Widget::RequestLatest.id()),
            )
            .height(Length::Cells(1))
            .shrink(0.0)
            .viewport();
        panel
            .child(controls)
            .children(rows.iter().skip(top).take(visible).map(|row| {
                let mut node = super::view::stage_row(&row.row, model, pen, None);
                if let Some(id) = row.target {
                    node = node.id(id);
                    if RequestHistory::is_focused_link(model, id) {
                        node.restyle_all(&|_| pen.raise(&pen.link));
                    }
                }
                node.height(Length::Cells(1)).shrink(0.0)
            }))
    }
    #[must_use]
    pub fn view(model: &Model, width: f32, height: f32) -> Node {
        let s = S::for_model(model);
        if model.roadmap_requests().is_empty() {
            return s.outlined(
                s.inset(Node::column().child(s.heading("What should change?"))
                    .child(s.prose("Ask to add work, change priorities, or revise the plan. Your requests and replies stay here."))
                    .child(s.prose("Updating the roadmap does not start an item.")), 16.0, 12.0),
                s.panel, s.line).height(Length::Px(height));
        }
        let body_height = (height - 2.0 * s.line_height - 50.0).max(0.0);
        let visible = (body_height / s.line_height).floor() as usize;
        let history = RequestHistory::of(model);
        let mut face = Typography::default();
        face.set_cell(model.cell_size());
        let rows = history.rows(
            (width - 34.0).max(1.0),
            Self::ruler(model),
            &mut |text, role| face.advance(text, role),
        );
        let top = RequestHistory::top(model, &rows, visible);
        let pen = Pen::for_theme(&model.theme());
        let body = Node::column()
            .id(Widget::RequestBody.id())
            .viewport()
            .height(Length::Px(body_height))
            .grow(1.0)
            .min_height(Length::Px(0.0))
            .children(rows.iter().skip(top).take(visible).map(|row| {
                let mut node = super::view::stage_row(&row.row, model, &pen, None);
                if let Some(id) = row.target {
                    node = node.id(id);
                    if RequestHistory::is_focused_link(model, id) {
                        node.restyle_all(&|_| pen.raise(&pen.link));
                    }
                }
                node.height(Length::Px(s.line_height)).shrink(0.0)
            }));
        let controls = Node::row()
            .child(s.label("Copy", s.accent).id(Widget::request_copy()))
            .child(s.label(" · Previous", s.accent).id(Widget::request_jump(0)))
            .child(s.label(" · Next", s.accent).id(Widget::request_jump(1)));
        s.outlined(
            Node::column()
                .min_height(Length::Px(0.0))
                .child(
                    s.section_heading(
                        "Requests",
                        s.label("Latest reply ↓", s.accent)
                            .id(Widget::RequestLatest.id()),
                    ),
                )
                .child(s.inset(controls, 16.0, 4.0))
                .child(s.inset(body, 16.0, 12.0).grow(1.0)),
            s.panel,
            s.line,
        )
        .height(Length::Px(height))
        .min_height(Length::Px(0.0))
    }

    pub fn scroll(model: &mut Model, delta: isize) {
        if model.surface().is_native()
            && let Some((total, visible)) = model.flight_panels.native_requests
        {
            let max = total.saturating_sub(visible);
            let top = model
                .roadmap_request_scroll
                .min(max)
                .saturating_add_signed(
                    delta.saturating_mul(isize::try_from(model.cell_size().height).unwrap_or(1)),
                )
                .min(max);
            if top == max && delta > 0 {
                RequestHistory::latest(model);
            } else {
                model.roadmap_request_scroll = top;
                let mut state = model.request_reading.borrow_mut();
                let position = state
                    .native_positions
                    .iter()
                    .take_while(|(_, y)| *y <= top)
                    .last()
                    .cloned();
                state.native_inset = position.as_ref().map_or(0, |(_, y)| top.saturating_sub(*y));
                state.anchor = position.map(|(at, _)| at);
            }
            return;
        }
        if !model.has_pixels() {
            let rows = RequestHistory::of(model).rows(
                f32::from(model.inspector_width().max(1)),
                0,
                &mut |text, _| f32::from(crate::render::width(text)),
            );
            let visible = Self::cell_visible(model);
            let max = rows.len().saturating_sub(visible);
            let top = RequestHistory::top(model, &rows, visible)
                .saturating_add_signed(delta)
                .min(max);
            if top == max && delta > 0 {
                RequestHistory::latest(model);
            } else {
                model.roadmap_request_scroll = top;
                model.request_reading.borrow_mut().anchor =
                    rows.get(top).map(|row| row.position.clone());
            }
            return;
        }
        let s = S::for_model(model);
        let layout = model.flight_layout();
        let width = if layout.compact {
            layout.width
        } else {
            layout.right_width
        };
        let height = super::FlightRoadmapPage::detail_height(model);
        let mut face = Typography::default();
        face.set_cell(model.cell_size());
        let rows = RequestHistory::of(model).rows(
            (width - 34.0).max(1.0),
            Self::ruler(model),
            &mut |text, role| face.advance(text, role),
        );
        let visible =
            ((height - 2.0 * s.line_height - 50.0).max(0.0) / s.line_height).floor() as usize;
        let max = rows.len().saturating_sub(visible);
        let top = RequestHistory::top(model, &rows, visible)
            .saturating_add_signed(delta)
            .min(max);
        if top == max && delta > 0 {
            RequestHistory::latest(model);
        } else {
            model.roadmap_request_scroll = top;
            model.request_reading.borrow_mut().anchor = rows.get(top).map(|r| r.position.clone());
        }
    }
}
