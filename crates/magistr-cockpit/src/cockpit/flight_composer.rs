//! The live message editor in the Flight Recorder interface.

use super::completion::Completion;
use super::{FlightStyle, Model, Pane, Selected, StageTab, TurnState, Widget};
use crate::layout::{Align, Inset, Length, Node, TextMetrics, TextRole};
use crate::pixel::Typography;

/// Pixel layout and file completion for the real prompt editor.
pub struct FlightComposer;

impl FlightComposer {
    /// Reserve exactly the same measured rows that the editor paints.
    #[must_use]
    pub fn height(model: &Model, width: f32, screen_height: f32) -> f32 {
        let mut face = Typography::default();
        face.set_cell(model.cell_size());
        let line = f32::from(model.cell_size().height);
        // Include the insertion point, including a final empty line.
        let text = format!("{} ", model.prompt().text());
        let rows = TextMetrics::new(text)
            .wrap_by(Self::editor_width(model, width), &mut |s| {
                face.advance(s, TextRole::Steer)
            })
            .len()
            .max(1);
        let rows = u16::try_from(rows).unwrap_or(u16::MAX);
        let minimum = 52.0 + line * 3.0;
        let maximum = (screen_height * 0.3).floor().max(minimum);
        (52.0 + line * 2.0 + f32::from(rows) * line)
            .min(maximum)
            .max(minimum)
    }

    fn button_width(model: &Model) -> f32 {
        let mut face = Typography::default();
        face.set_cell(model.cell_size());
        (face.advance("Send ↑", TextRole::Label) + 20.0).max(74.0)
    }

    fn editor_width(model: &Model, width: f32) -> f32 {
        (width - 34.0 - 26.0 - 16.0 - Self::button_width(model)).max(24.0)
    }

    /// The approved message area, wired to the existing prompt and action IDs.
    #[must_use]
    pub fn view(model: &Model, style: &FlightStyle, width: f32, height: f32) -> Node {
        let roadmap = model.tab() == StageTab::Roadmap;
        let heading = if roadmap {
            "Request roadmap changes"
        } else {
            "Message Magistr"
        };
        let running = if roadmap {
            model.roadmap_request_pending()
        } else {
            model.turn_state() == TurnState::Streaming
        };
        let active = running || !model.prompt().text().trim().is_empty();
        let line = f32::from(model.cell_size().height);
        let editor_height = (height - 52.0 - line * 2.0).max(line);
        let editor = if model.prompt().is_empty() {
            style
                .label(
                    if roadmap {
                        "Describe what you want to add or change…"
                    } else if running {
                        "Write your next message…"
                    } else {
                        "Ask a question or describe what you want to do…"
                    },
                    style.muted,
                )
                .text_role(TextRole::Steer)
        } else {
            Node::editor(model.prompt().text(), model.prompt().cursor())
                .text_role(TextRole::Steer)
                .style(style.style(style.ink).bg(style.raised))
        }
        .id(Widget::Prompt.id())
        .height(Length::Px(editor_height))
        .grow(1.0)
        .min_width(Length::Px(0.0))
        .viewport();
        let button_bg = if active { style.selected } else { style.raised };
        let mut button = style
            .ground(
                style.chip(
                    if roadmap && running {
                        "Cancel"
                    } else if running {
                        "Stop"
                    } else {
                        "Send ↑"
                    },
                    if active { style.ink } else { style.muted },
                    button_bg,
                ),
                button_bg,
            )
            .width(Length::Px(Self::button_width(model)))
            .height(Length::Px(line + 4.0))
            .shrink(0.0);
        if active {
            button = button.id(Widget::PromptAction.id());
        }
        let mut entry = style.ground(
            style.inset(
                Node::row()
                    .align_items(Align::Center)
                    .child(editor)
                    .child(FlightStyle::hspace(16.0))
                    .child(button),
                12.0,
                5.0,
            ),
            style.raised,
        );
        entry.restyle_at(Widget::PromptAction.id(), &|s| s.clone().bg(button_bg));
        let entry = style
            .outlined(entry, style.raised, style.line)
            .height(Length::Px(editor_height + 16.0))
            .shrink(0.0);
        let activity = model.agent_activity();
        let hint = if roadmap && running {
            if model.roadmap_request_running() {
                format!("Updating roadmap · {}", activity.status)
            } else {
                model
                    .roadmap_requests()
                    .last()
                    .map_or("Waiting for response", |r| r.phase.label())
                    .to_owned()
            }
        } else if !roadmap && activity.active {
            format!("{} · {}", activity.task, activity.status)
        } else if width >= 600.0 {
            "Enter to send  ·  Shift + Enter for a new line  ·".to_owned()
        } else {
            "Enter to send  ·".to_owned()
        };
        let mut hint_face = Typography::default();
        hint_face.set_cell(model.cell_size());
        let hint = FlightStyle::fit_label(
            &hint,
            (width - 58.0 - hint_face.advance("Add files", TextRole::Label)).max(1.0),
            &hint_face,
        );
        let files = style
            .label("Add files", style.accent)
            .id(Widget::PromptFiles.id())
            .shrink(0.0);
        let footer = Node::row()
            .align_items(Align::Center)
            .child(style.label(hint, style.muted).min_width(Length::Px(0.0)))
            .child(FlightStyle::hspace(8.0))
            .child(files);
        let context = if roadmap {
            Self::selected_work(model)
                .or(model.title())
                .unwrap_or("Roadmap")
        } else {
            model
                .rail()
                .ship()
                .scope()
                .and_then(|scope| model.roadmap().chunk_for_ref(scope))
                .and_then(Self::work_title)
                .or(model.title())
                .unwrap_or("Current conversation")
        };
        let aside = if width >= 640.0 {
            let mut face = Typography::default();
            face.set_cell(model.cell_size());
            let available = (width
                - 58.0
                - face.advance(heading, TextRole::Heading)
                - face.advance("  ·  Draft", TextRole::Label))
            .max(0.0);
            let context = if roadmap && Self::selected_work(model).is_some() {
                format!("Selected: {context}")
            } else {
                context.to_owned()
            };
            let context = FlightStyle::fit_label(&context, available, &face);
            style.label(format!("{context}  ·  Draft"), style.muted)
        } else {
            style.label("Draft", style.muted)
        };
        let mut card = style
            .outlined(
                Node::column()
                    .child(style.section_heading(heading, aside))
                    .child(
                        style
                            .inset(
                                Node::column()
                                    .child(entry)
                                    .child(FlightStyle::vspace(6.0))
                                    .child(footer),
                                16.0,
                                6.0,
                            )
                            .grow(1.0),
                    ),
                style.panel,
                style.line,
            )
            .height(Length::Px(height))
            .shrink(0.0);
        for id in [Widget::PromptFiles.id(), Widget::PromptAction.id()] {
            if model.hovered() == Some(id) {
                card.restyle_at(id, &|s| s.clone().bg(style.selected));
            }
        }
        card
    }

    fn work_title(chunk: &super::RoadmapChunk) -> Option<&str> {
        let title = chunk.title.trim();
        if !title.is_empty() {
            return Some(title);
        }
        let name = chunk.name.trim();
        (!name.is_empty() && name != chunk.id).then_some(name)
    }

    fn selected_work(model: &Model) -> Option<&str> {
        let Selected::Chunk(id) = model.selected()? else {
            return None;
        };
        model.roadmap().chunk(id).and_then(Self::work_title)
    }

    /// Put the selected work in an editable request without replacing a draft.
    /// The visible text goes through the normal send, history and retry paths.
    pub fn request_change(model: &mut Model) {
        if model.prompt.is_empty()
            && let Some(title) = Self::selected_work(model)
        {
            let prefix = format!("For “{title}”, ");
            model.prompt.insert(&prefix);
        }
        model.pane = Pane::Stage;
    }

    /// Open the workspace file chooser without replacing the current draft.
    pub fn add_files(model: &mut Model) {
        if model.prompt.text()[..model.prompt.cursor()]
            .chars()
            .next_back()
            .is_some_and(|c| !c.is_whitespace())
        {
            model.prompt.insert(" ");
        }
        let anchor = model.prompt.cursor();
        model.prompt.insert("@");
        let mut completion = Completion::file(anchor);
        completion.refresh(
            model.prompt.text(),
            model.prompt.cursor(),
            &model.workspace_files,
        );
        // Keep an empty chooser open so the user receives a useful empty state.
        model.completion = Some(completion);
    }

    /// A floating chooser above the composer; it cannot squeeze the message area.
    #[must_use]
    pub fn completion(model: &Model, style: &FlightStyle, width: f32) -> Option<Node> {
        let completion = model.completion()?;
        let mut rows = Node::column();
        if completion.items().is_empty() {
            rows = rows.child(style.label(
                if completion.is_file() {
                    "No matching files. Type a path or press Esc to close."
                } else {
                    "No matching commands. Press Esc to close."
                },
                style.muted,
            ));
        } else {
            for (index, item) in completion.items().iter().enumerate() {
                let selected = index == completion.selected();
                let ground = if selected {
                    style.selected
                } else {
                    style.panel
                };
                let row = style
                    .ground(style.inset(style.label(item, style.ink), 12.0, 5.0), ground)
                    .id(Widget::completion(index))
                    .height(Length::Px(style.line_height + 10.0))
                    .shrink(0.0);
                rows = rows.child(row);
            }
        }
        Some(
            style
                .outlined(
                    Node::column()
                        .child(style.section_heading(
                            if completion.is_file() {
                                "Add files"
                            } else {
                                "Commands"
                            },
                            style.label("Enter to choose · Esc to close", style.muted),
                        ))
                        .child(rows),
                    style.panel,
                    style.line,
                )
                .width(Length::Px((width - 32.0).clamp(120.0, 640.0)))
                .overlay(
                    Inset::none()
                        .left(Length::Px(16.0))
                        .bottom(Length::Px(model.flight_layout().composer_height + 8.0)),
                ),
        )
    }
}

#[cfg(test)]
#[path = "flight_composer.tests.rs"]
mod tests;
