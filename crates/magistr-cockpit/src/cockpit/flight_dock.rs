//! Pending answers and pinned command output above the message area.
use super::{DockItem, FlightStyle as S, Model, Widget};
use crate::layout::{Align, Length, Node, TextMetrics, TextRole};
use crate::pixel::Typography;
use crate::theme::Pen;

/// Pixel surfaces for requests that must remain actionable while reading.
pub struct FlightDock;
impl FlightDock {
    #[must_use]
    pub fn height(model: &Model, width: f32) -> f32 {
        if model.dock().is_empty() {
            return 0.0;
        }
        let line = f32::from(model.cell_size().height);
        if model.moment().is_some() {
            return line + 20.0;
        }
        let count = lines(model, width).len().clamp(1, 4);
        let rows = if width < 700.0
            && matches!(model.dock().front(), Some(DockItem::Permission(p)) if p.buttons().count() > 2)
        {
            2.0
        } else {
            1.0
        };
        line * count as f32 + (line + 14.0).max(34.0) + (line + 4.0) * rows + 28.0
    }
    #[must_use]
    pub fn view(model: &Model, width: f32) -> Option<Node> {
        let item = model.dock().front()?;
        let s = S::for_model(model);
        if model.moment().is_some() {
            return Some(
                s.ground(
                    s.inset(s.label("Waiting for your answer", s.accent), 16.0, 10.0),
                    s.panel,
                )
                .height(Length::Px(Self::height(model, width)))
                .shrink(0.0),
            );
        }
        let text = lines(model, width);
        let top = model.decision().scroll().min(text.len().saturating_sub(4));
        let mut body = Node::column().children(
            text.into_iter()
                .skip(top)
                .take(4)
                .map(|line| s.label(line, s.ink)),
        );
        body = body.child(S::vspace(8.0));
        let mut actions = Node::row().align_items(Align::Center);
        let buttons: Vec<_> = match item {
            DockItem::Permission(p) => p
                .buttons()
                .map(|(kind, _)| {
                    use magistr_application::sessions::PermissionKind;
                    let label = match kind {
                        PermissionKind::AllowOnce => "Allow once",
                        PermissionKind::AllowAlways => "Allow for session",
                        PermissionKind::RejectOnce => "Deny once",
                        PermissionKind::RejectAlways => "Deny for session",
                    };
                    (label, Widget::button(kind))
                })
                .collect(),
            DockItem::Proposal { .. } => {
                vec![("Accept", Widget::Accept), ("Reject", Widget::Reject)]
            }
            DockItem::Blocker { .. } => vec![("Mark resolved", Widget::Clear)],
        };
        for (index, (label, widget)) in buttons.into_iter().enumerate() {
            if width < 700.0 && index == 2 {
                body = body.child(actions);
                actions = Node::row().align_items(Align::Center);
            }
            actions = actions
                .child(s.chip(label, s.accent, s.raised).id(widget.id()))
                .child(S::hspace(8.0));
        }
        body = body.child(actions);
        let count = model.dock().len();
        let aside = if count > 1 {
            format!("{} more waiting", count - 1)
        } else {
            String::new()
        };
        Some(
            s.outlined(
                Node::column()
                    .child(s.section_heading("Needs your answer", s.label(aside, s.muted)))
                    .child(s.inset(body, 16.0, 10.0)),
                s.panel,
                s.line,
            )
            .id(Widget::Dock.id())
            .height(Length::Px(Self::height(model, width)))
            .shrink(0.0),
        )
    }
    #[must_use]
    pub fn terminal_height(model: &Model) -> f32 {
        let count = model.term_dock().taken(model.now());
        if count == 0 {
            return 0.0;
        }
        (count - 1) as f32 * f32::from(model.cell_size().height)
            + (f32::from(model.cell_size().height) + 14.0).max(34.0)
            + 2.0
    }
    #[must_use]
    pub fn terminal(model: &Model, pen: &Pen) -> Option<Node> {
        let old = super::view::term_dock(model, pen)?;
        let s = S::for_model(model);
        let title = model
            .term_dock()
            .id()
            .and_then(|id| model.turn().terminal_title(id))
            .unwrap_or("Command output");
        let body = Node::column()
            .id(Widget::TermDock.id())
            .viewport()
            .children(old.child_nodes().iter().skip(1).cloned());
        Some(
            s.outlined(
                Node::column()
                    .child(
                        s.section_heading(title, s.label("Esc to close", s.muted))
                            .id(Widget::TermDockGrip.id()),
                    )
                    .child(body),
                s.panel,
                s.line,
            )
            .height(Length::Px(Self::terminal_height(model)))
            .shrink(0.0),
        )
    }
}

fn lines(model: &Model, width: f32) -> Vec<String> {
    let Some(item) = model.dock().front() else {
        return Vec::new();
    };
    let title = |id: &str| {
        model
            .roadmap()
            .chunk(id)
            .map_or("Work item", |c| c.name.as_str())
    };
    let text = match item {
        DockItem::Permission(p) => p.title().to_owned(),
        DockItem::Proposal {
            chunk,
            kind,
            suggestion,
            reason,
        } => format!(
            "{}: {} → {suggestion}\n{reason}",
            title(chunk),
            kind.label()
        ),
        DockItem::Blocker { chunk, reason } => format!("{} is blocked. {reason}", title(chunk)),
    };
    let mut face = Typography::default();
    face.set_cell(model.cell_size());
    text.lines()
        .flat_map(|line| {
            TextMetrics::new(line)
                .wrap_by((width - 34.0).max(40.0), &mut |part| {
                    face.advance(part, TextRole::Label)
                })
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}
