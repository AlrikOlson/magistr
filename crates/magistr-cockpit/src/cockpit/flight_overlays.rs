//! Pixel-native menus, approval cards and hover help.

#[cfg(test)]
#[path = "flight_overlays.tests.rs"]
mod tests;

use crate::agent::PermissionKind;
use crate::cockpit::hover;
use crate::cockpit::{FlightStyle, Model, Permission, Tooltip, Widget};
use crate::layout::{Inset, Length, Node, TextMetrics, TextRole};
use crate::pixel::Typography;
use crate::theme::Pen;

#[path = "flight_secondary.rs"]
mod secondary;
use secondary::FlightSecondary;

/// Common secondary surfaces above the stable conversation.
pub(super) struct FlightOverlays;

impl FlightOverlays {
    pub(super) fn scroll_permission(model: &mut Model, delta: isize) {
        let Some(permission) = model.moment() else {
            return;
        };
        let (lines, visible, _) = permission_window(model, permission);
        let maximum = lines.len().saturating_sub(visible);
        model.permission_scroll = model
            .permission_scroll
            .saturating_add_signed(delta)
            .min(maximum);
    }

    pub(super) fn finish(mut root: Node, model: &Model, pen: &Pen) -> Node {
        let style = FlightStyle::for_model(model);
        if let Some(permission) = model.moment() {
            root.restyle_all(&|s| pen.fade(s, super::MOMENT_DIM));
            root = root.child(Node::centered_overlay(permission_card(
                model, permission, &style,
            )));
        }
        if model.is_sheet_open() {
            root = root.child(Node::centered_overlay(FlightSecondary::help(model, &style)));
        }
        if model.settings().is_some() {
            root = root.child(Node::centered_overlay(FlightSecondary::settings(
                model, &style,
            )));
        }
        if model.search().is_some() {
            root = root.child(Node::centered_overlay(FlightSecondary::search(
                model, &style,
            )));
        }
        if model.palette().is_some() {
            root = root.child(Node::centered_overlay(palette(model, &style)));
        }
        if let Some(help) = model.tooltip() {
            root = root.child(tooltip(model, &style, &help));
        }
        // Dividers paint the same feedback for keyboard focus and captured dragging.
        let hovered = model
            .hovered()
            .filter(|id| crate::cockpit::flight_divider::FlightDivider::from_id(*id).is_none());
        hover::answer(&mut root, hovered, model.pressed(), pen);
        root
    }
}

fn palette(model: &Model, style: &FlightStyle) -> Node {
    let state = model.palette().expect("open palette");
    let layout = model.flight_layout();
    let width = (layout.screen_width - 40.0).clamp(120.0, 640.0);
    let row_height = style.line_height + 14.0;
    let available = ((layout.screen_height - 150.0).max(row_height) / row_height).floor() as usize;
    let rows = crate::cockpit::palette::Palette::rows(model);
    let top = (state.chosen() + 1).saturating_sub(available.max(1));
    let editor = Node::editor(state.query(), state.query().len())
        .id(Widget::OverlayQuery.id())
        .text_role(TextRole::Steer)
        .style(style.style(style.ink).bg(style.raised))
        .height(Length::Px(style.line_height + 8.0))
        .min_width(Length::Px(0.0));
    let mut body = Node::column()
        .child(style.ground(style.inset(editor, 12.0, 6.0), style.raised))
        .child(FlightStyle::vspace(8.0));
    if rows.is_empty() {
        body = body.child(style.label("No matching commands or work items.", style.muted));
    }
    for (index, row) in rows.iter().enumerate().skip(top).take(available.max(1)) {
        let name = row
            .name
            .as_ref()
            .map_or_else(|| row.verb.clone(), |name| format!("{} {name}", row.verb));
        let ground = if index == state.chosen() {
            style.selected
        } else {
            style.panel
        };
        body = body.child(
            style
                .ground(style.inset(style.label(name, style.ink), 12.0, 6.0), ground)
                .id(Widget::palette_row(index))
                .height(Length::Px(row_height))
                .shrink(0.0),
        );
    }
    style
        .outlined(
            Node::column()
                .child(style.section_heading("Commands", style.label("Esc to close", style.muted)))
                .child(style.inset(body, 12.0, 12.0)),
            style.panel,
            style.line,
        )
        .width(Length::Px(width))
        .max_height(Length::Px((layout.screen_height - 24.0).max(80.0)))
}

fn permission_window(model: &Model, permission: &Permission) -> (Vec<String>, usize, f32) {
    let layout = model.flight_layout();
    let width = (layout.screen_width - 40.0).clamp(180.0, 660.0);
    let line = f32::from(model.cell_size().height);
    let mut face = Typography::default();
    face.set_cell(model.cell_size());
    let mut paragraphs = vec![permission.title().to_owned()];
    paragraphs.extend(
        permission
            .steps()
            .enumerate()
            .map(|(i, step)| format!("{}. {step}", i + 1)),
    );
    if let Some(scope) = permission.scope() {
        paragraphs.push(format!("Applies to: {scope}"));
    }
    let mut lines = Vec::new();
    for paragraph in paragraphs {
        lines.extend(
            TextMetrics::new(paragraph)
                .wrap_by((width - 34.0).max(1.0), &mut |text| {
                    face.advance(text, TextRole::Prose)
                })
                .into_iter()
                .map(str::to_owned),
        );
        lines.push(String::new());
    }
    let body_height = ((layout.screen_height - 144.0 - line * 4.0).max(line))
        .min(400.0)
        .min(lines.len() as f32 * line);
    let visible = (body_height / line).floor().max(1.0) as usize;
    (lines, visible, body_height)
}

fn permission_card(model: &Model, permission: &Permission, style: &FlightStyle) -> Node {
    let width = (model.flight_layout().screen_width - 40.0).clamp(180.0, 660.0);
    let line = f32::from(model.cell_size().height);
    let (lines, visible, body_height) = permission_window(model, permission);
    let top = model
        .permission_scroll
        .min(lines.len().saturating_sub(visible));
    let body = Node::column()
        .viewport()
        .height(Length::Px(body_height))
        .children(
            lines
                .into_iter()
                .skip(top)
                .take(visible)
                .map(|text| style.prose(text).height(Length::Px(line)).shrink(0.0)),
        );
    let mut buttons = Node::row().wrap();
    for (kind, _) in permission.buttons() {
        let (name, id) = match kind {
            PermissionKind::AllowOnce => ("Allow", Widget::Allow),
            PermissionKind::AllowAlways => ("Always allow", Widget::Always),
            PermissionKind::RejectOnce => ("Deny", Widget::Deny),
            PermissionKind::RejectAlways => ("Always deny", Widget::Never),
        };
        buttons = buttons
            .child(
                style
                    .chip(
                        name,
                        style.ink,
                        if kind.is_allow() {
                            style.selected
                        } else {
                            style.raised
                        },
                    )
                    .id(id.id())
                    .height(Length::Px(style.line_height + 8.0))
                    .shrink(0.0),
            )
            .child(FlightStyle::hspace(8.0));
    }
    if !permission.hidden_images().is_empty() {
        buttons = buttons.child(
            style
                .chip("Show images", style.accent, style.raised)
                .id(Widget::ShowImages.id())
                .height(Length::Px(style.line_height + 8.0)),
        );
    }
    let editor = if model.condition().is_empty() {
        style.label("Add a condition, or choose an answer above.", style.muted)
    } else {
        Node::editor(model.condition().text(), model.condition().cursor())
            .text_role(TextRole::Steer)
            .style(style.style(style.ink))
    }
    .height(Length::Px(line * 2.0))
    .min_width(Length::Px(0.0));
    let content = Node::column()
        .child(
            style.section_heading(
                "Permission needed",
                style.label(
                    permission
                        .checkpoint()
                        .and_then(|id| model.rail().spine().get(id))
                        .map_or("Waiting for you", |checkpoint| match checkpoint.state() {
                            crate::cockpit::CheckpointState::Saved { .. } => "Checkpoint saved",
                            crate::cockpit::CheckpointState::Failed(_) => "Checkpoint failed",
                            _ => "Checkpoint requested",
                        }),
                    style.muted,
                ),
            ),
        )
        .child(
            style.inset(
                Node::column()
                    .child(body)
                    .child(FlightStyle::vspace(12.0))
                    .child(buttons)
                    .child(FlightStyle::vspace(12.0))
                    .child(style.outlined(
                        style.ground(style.inset(editor, 10.0, 6.0), style.raised),
                        style.raised,
                        style.line,
                    )),
                16.0,
                12.0,
            ),
        );
    style
        .outlined(content, style.panel, style.accent)
        .width(Length::Px(width))
}

fn tooltip(model: &Model, style: &FlightStyle, help: &Tooltip) -> Node {
    let layout = model.flight_layout();
    let mut face = Typography::default();
    face.set_cell(model.cell_size());
    let width = (face.advance(help.text, TextRole::Label) + 24.0)
        .min((layout.screen_width - 16.0).max(1.0));
    let pointer_x = f32::from(help.col) * f32::from(model.cell_size().width);
    let pointer_y = f32::from(help.row) * f32::from(model.cell_size().height);
    let x = if pointer_x + 12.0 + width < layout.screen_width {
        pointer_x + 12.0
    } else {
        (pointer_x - width - 8.0).max(0.0)
    };
    let y = if pointer_y + 48.0 < layout.screen_height {
        pointer_y + 24.0
    } else {
        (pointer_y - 36.0).max(0.0)
    };
    style
        .outlined(
            style.inset(style.label(help.text, style.ink), 10.0, 6.0),
            style.panel,
            style.line,
        )
        .width(Length::Px(width))
        .overlay(Inset::none().left(Length::Px(x)).top(Length::Px(y)))
}
