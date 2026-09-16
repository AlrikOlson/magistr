//! The production terminal workspace: conversation stays present during inspection.
use super::inspector::Inspector;
use super::{
    FlightComposer, FlightRecorder, FlightStyle as S, FrameClock, Model, Pane, StageTab, TurnState,
    Widget,
};
use crate::layout::{Align, Inset, Length, Node, TextMetrics, TextRole};
use crate::pixel::Typography;
use crate::theme::Pen;

/// Composes the approved Flight Recorder from live domain state.
pub struct FlightShell;

#[cfg(test)]
#[path = "flight_shell.tests.rs"]
mod tests;

impl FlightShell {
    #[must_use]
    pub fn view(model: &Model, _clock: &FrameClock) -> Node {
        let s = S::for_model(model);
        let l = model.flight_layout();
        let pen = Pen::for_theme(&model.theme());
        let left = Node::column()
            .width(Length::Px(l.left_width))
            .shrink(0.0)
            .min_height(Length::Px(0.0))
            .child(
                conversation(model, &s, &pen)
                    .height(Length::Px(l.conversation_height))
                    .shrink(0.0),
            )
            .child(S::vspace(16.0))
            .child(FlightRecorder::view_in(
                model,
                l.left_width,
                l.recorder_height,
            ));
        let mut body = Node::row()
            .height(Length::Px(l.body_height))
            .shrink(0.0)
            .min_height(Length::Px(0.0))
            .child(left);
        if !l.compact {
            body = body.child(S::hspace(20.0)).child(
                Node::column()
                    .width(Length::Px(l.right_width))
                    .shrink(0.0)
                    .min_height(Length::Px(0.0))
                    .child(
                        super::FlightRoadmap::view(model, l.right_width, l.roadmap_height)
                            .height(Length::Px(l.roadmap_height))
                            .shrink(0.0),
                    )
                    .child(S::vspace(16.0))
                    .child(
                        details(model, &s, &pen, l.right_width, l.detail_height)
                            .height(Length::Px(l.detail_height))
                            .shrink(0.0),
                    ),
            );
        }
        if model.tab() == StageTab::Roadmap {
            body = super::FlightRoadmapPage::view(model);
        }
        let mut content = Node::column()
            .min_height(Length::Px(0.0))
            .style(s.style(s.ink).bg(s.root))
            .child(header(model, &s))
            .child(S::vspace(12.0))
            .child(body);
        if let Some(term) = super::FlightDock::terminal(model, &pen) {
            content = content.child(term);
        }
        if let Some(dock) = super::FlightDock::view(model, l.width) {
            content = content.child(dock);
        }
        content = content.child(S::vspace(12.0)).child(FlightComposer::view(
            model,
            &s,
            l.width,
            l.composer_height,
        ));
        let mut root = s
            .inset(content, l.margin, 12.0)
            .style(s.style(s.ink).bg(s.root));
        if l.compact
            && model.tab() != StageTab::Roadmap
            && (model.pane() == Pane::Inspector || model.tab() != StageTab::Turn)
        {
            root = root.child(
                details(model, &s, &pen, l.width, l.body_height)
                    .width(Length::Px(l.width))
                    .height(Length::Px(l.body_height))
                    .overlay(Inset::none().left(Length::Px(l.margin)).top(Length::Px(
                        24.0 + f32::from(model.cell_size().height).max(28.0),
                    ))),
            );
        }
        if let Some(popup) = FlightComposer::completion(model, &s, l.width) {
            root = root.child(popup);
        }
        super::view::finish(root, model, &pen, false)
    }
}

fn header(model: &Model, s: &S) -> Node {
    let title = model.title().unwrap_or("Your project");
    let mut navigation = Node::row().align_items(Align::Center);
    if model.flight_layout().compact && model.tab() != StageTab::Roadmap {
        navigation = navigation
            .child(
                s.chip("Roadmap", s.muted, s.root)
                    .id(Widget::TabRoadmap.id()),
            )
            .child(S::hspace(8.0));
    }
    navigation = navigation
        .child(
            s.chip("Records", s.muted, s.root)
                .id(Widget::TabSession.id()),
        )
        .child(S::hspace(8.0))
        .child(
            s.chip("Commands", s.muted, s.root)
                .id(Widget::Commands.id()),
        );
    s.ground(
        Node::row()
            .align_items(Align::Center)
            .height(Length::Px(f32::from(model.cell_size().height).max(28.0)))
            .shrink(0.0)
            .child(
                s.heading(if model.tab() == StageTab::Roadmap {
                    "← Conversation"
                } else {
                    "MAGISTR"
                })
                .id(Widget::TabTurn.id()),
            )
            .child(S::hspace(24.0))
            .child(
                s.label(title, s.muted)
                    .viewport()
                    .min_width(Length::Px(0.0))
                    .shrink(1.0),
            )
            .child(S::flex())
            .child(status_label(model, s))
            .child(S::hspace(20.0))
            .child(navigation),
        s.root,
    )
}

fn status_label(model: &Model, s: &S) -> Node {
    let text = status(model);
    let pen = Pen::for_theme(&model.theme());
    let ink = match text {
        "Connection error" | "Agent unavailable" | "Disconnected" | "Work failed" => {
            pen.gate_fail.fg
        }
        "Needs your answer" | "Limit reached" | "Request declined" => s.accent,
        "Stopped" | "Paused" | "Waiting for updates" => s.muted,
        _ => s.good,
    };
    s.label(text, ink)
}

fn status(model: &Model) -> &'static str {
    if !model.dock().is_empty() {
        "Needs your answer"
    } else if model.turn_notice == Some("Work failed")
        && model.loop_status().is_none_or(|s| s.ended().is_some())
    {
        "Work failed"
    } else if model.loop_status().is_some_and(|s| s.ended().is_some()) {
        "Stopped"
    } else if model.feed() == Some(magistr_engine::FeedStatus::Disconnected)
        && !model
            .loop_status()
            .is_some_and(|s| s.stage().is_between_agents())
    {
        "Disconnected"
    } else if model.projection().freshness() == super::Freshness::Stale {
        "Waiting for updates"
    } else if model.agent_missing().is_some() {
        "Agent unavailable"
    } else if let Some(loop_status) = model.loop_status() {
        if loop_status.is_paused() {
            "Paused"
        } else {
            match loop_status.stage() {
                super::LoopStage::Connecting => "Starting agent",
                super::LoopStage::Agent => "Working",
                super::LoopStage::Gates => "Checking work",
                super::LoopStage::Judge => "Reviewing work",
            }
        }
    } else if model.turn_notice.is_some_and(|notice| notice != "Ready") {
        model.turn_notice.unwrap_or("Ready")
    } else if model.rail().ship().is_shipped() {
        "Completed"
    } else {
        match model.turn_state() {
            TurnState::Streaming => "Working",
            TurnState::AwaitingInput | TurnState::Idle => "Ready",
        }
    }
}

fn conversation(model: &Model, s: &S, pen: &Pen) -> Node {
    let l = model.flight_layout();
    let title = model
        .rail()
        .ship()
        .objective()
        .unwrap_or("What would you like to work on?");
    let mut content = Node::column().min_height(Length::Px(0.0)).child(
        s.heading(title)
            .viewport()
            .height(Length::Px(f32::from(model.cell_size().height)))
            .shrink(0.0),
    );
    if l.conversation_height >= 380.0 {
        content = content.child(S::vspace(4.0)).child(s.label(
            format!("{} reasoning steps recorded", model.trace().steps().len()),
            s.muted,
        ));
    }
    content = content.child(S::vspace(8.0));
    let rows = if model.turn().is_empty() {
        let hint = "Send a message to begin. Your conversation and recorded work will appear here.";
        let available = model.conversation_height() as f32;
        let button_height = s.line_height + 4.0;
        let buttons = model.launcher().is_some() && available >= button_height;
        let mut face = Typography::default();
        face.set_cell(model.cell_size());
        let hint_height = TextMetrics::new(hint)
            .wrap_by((l.left_width - 42.0).max(1.0), &mut |text| {
                face.advance(text, TextRole::Prose)
            })
            .len() as f32
            * s.line_height;
        let mut rows = Vec::new();
        if available >= hint_height + if buttons { button_height + 16.0 } else { 0.0 } {
            rows.push(s.prose(hint).height(Length::Px(hint_height)));
        } else if !buttons && available >= s.line_height {
            rows.push(
                s.prose("Send a message to begin.")
                    .height(Length::Px(s.line_height))
                    .viewport(),
            );
        }
        if buttons {
            if !rows.is_empty() {
                rows.push(S::vspace(16.0));
            }
            rows.push(
                Node::row()
                    .height(Length::Px(button_height))
                    .shrink(0.0)
                    .child(
                        s.chip("Start next item", s.accent, s.raised)
                            .id(Widget::LaunchNext.id()),
                    )
                    .child(S::hspace(8.0))
                    .child(
                        s.chip("Run checks", s.accent, s.raised)
                            .id(Widget::LaunchGates.id()),
                    ),
            );
        }
        rows
    } else {
        vec![super::FlightConversation::view(model, pen)]
    };
    content = content.child(
        Node::column()
            .id(Widget::Stage.id())
            .viewport()
            .grow(1.0)
            .min_height(Length::Px(0.0))
            .children(rows),
    );
    s.outlined(
        Node::column()
            .min_height(Length::Px(0.0))
            .child(s.section_heading("Conversation", status_label(model, s)))
            .child(s.inset(content, 20.0, 12.0).grow(1.0)),
        s.panel,
        s.line,
    )
}

fn details(model: &Model, _s: &S, _pen: &Pen, width: f32, height: f32) -> Node {
    if model.pane() != Pane::Stage || model.tab() == StageTab::Turn {
        Inspector::flight_view(model, width, height)
    } else {
        super::FlightRecords::view(model, width, height)
    }
}
