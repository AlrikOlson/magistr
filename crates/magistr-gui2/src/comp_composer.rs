//! Native message area over the shared session draft and command identities.
use crate::comp_workspace::control;
use crate::{Component, Element, ElementId, Lexicon, Medium, Voice};
use magistr_cockpit::cockpit::{Model, StageTab, TurnState, Widget};

pub struct ComposerComposition;

impl ComposerComposition {
    pub const PILL: ElementId = ElementId(150);
    const VOICE: Voice = Voice {
        px: 14.0,
        ..Voice::SLOT
    };
    pub const LINE: f32 = 22.0;
    const CHROME: f32 = 88.0;

    /// Reserve bounded draft space. The native text leaf measures and scrolls its
    /// actual proportional rows, including long words and the insertion point.
    pub fn height(model: &Model, width: f32, screen_height: f32) -> f32 {
        let columns = ((width - 32.0).max(32.0) / 8.0) as u16;
        let rows = model.prompt().wrapped_rows(columns).clamp(1, 6) as f32;
        let ceiling = (screen_height * 0.35).clamp(Self::CHROME + Self::LINE, 220.0);
        (Self::CHROME + rows * Self::LINE).min(ceiling)
    }

    pub fn el(model: &Model, medium: Medium) -> Element {
        Self::sized(model, medium, 600.0, Self::height(model, 600.0, 760.0))
    }

    pub fn sized(model: &Model, medium: Medium, width: f32, height: f32) -> Element {
        let lex = Lexicon::of(medium);
        let planning = model.tab() == StageTab::Roadmap;
        let running = if planning {
            model.roadmap_request_pending()
        } else {
            model.turn_state() == TurnState::Streaming
        };
        let heading = if planning {
            "Request roadmap changes"
        } else {
            "Message Magistr"
        };
        let placeholder = if planning {
            "Describe what you want to add or change…"
        } else if running {
            "Write your next message…"
        } else {
            "Ask a question or describe what you want to do…"
        };
        let prompt = model.prompt();
        let editor = if prompt.is_empty() {
            Element::text(Self::VOICE, lex.sb0, placeholder)
        } else {
            Element::text(Self::VOICE, lex.tx, prompt.text())
        }
        .editor_cursor(prompt.cursor())
        .height((height - Self::CHROME).max(Self::LINE))
        .min_width(0.0)
        .no_shrink()
        .id(Self::PILL)
        .focusable()
        .focus_ring(lex.mauve)
        .role(accesskit::Role::MultilineTextInput)
        .label(heading)
        .claim(Component::Field);
        let action = control(
            if running { "Stop" } else { "Send" },
            Widget::PromptAction.id(),
            &lex,
        )
        .role(accesskit::Role::Button);
        let action = if !running && prompt.text().trim().is_empty() {
            action.disabled()
        } else {
            action
        };
        let mut footer = Element::row()
            .height(28.0)
            .no_shrink()
            .center()
            .gap(12.0)
            .child(
                control("Add files", Widget::PromptFiles.id(), &lex).role(accesskit::Role::Button),
            );
        if width >= 620.0 {
            footer = footer.child(
                Element::text(
                    Voice::HINT,
                    lex.sb0,
                    "Enter to send · Shift/Alt + Enter for a new line",
                )
                .nowrap(),
            );
        }
        footer = footer.child(Element::row().grow(1.0)).child(action);
        let activity = model.agent_activity();
        let status =
            if planning && model.roadmap_request_pending() && !model.roadmap_request_running() {
                model
                    .roadmap_requests()
                    .last()
                    .map_or("Waiting", |r| r.phase.label())
                    .to_owned()
            } else {
                format!("{} · {}", activity.task, activity.status)
            };
        Element::surface()
            .height(height)
            .no_shrink()
            .pad_x(12.0)
            .pad_y(8.0)
            .radius(9.0)
            .fill(lex.mantle)
            .stroke(lex.grounded(lex.hairc), 1.0)
            .child(
                Element::row()
                    .height(24.0)
                    .no_shrink()
                    .center()
                    .gap(12.0)
                    .child(Element::text(Voice::LEDGER_NAME, lex.tx, heading).nowrap())
                    .child(Element::row().grow(1.0))
                    .child(
                        Element::text(Voice::HINT, lex.sb0, status)
                            .nowrap()
                            .min_width(0.0),
                    ),
            )
            .child(editor)
            .child(
                Element::text(Voice::HINT, lex.sb0, Self::files(model))
                    .height(20.0)
                    .no_shrink()
                    .nowrap(),
            )
            .child(footer)
            .claim(Component::Field)
    }

    fn files(model: &Model) -> String {
        match references(model.prompt().text()) {
            Ok(paths) if paths.is_empty() => "Files: choose Add files or type @path".to_owned(),
            Ok(paths) => format!("Files: {}", paths.join(" · ")),
            Err(message) => message,
        }
    }

    /// Completion floats above the message area and shares the core's selection.
    pub fn completion(model: &Model, medium: Medium, viewport: (f32, f32)) -> Option<Element> {
        let completion = model.completion()?;
        let lex = Lexicon::of(medium);
        let mut card = Element::surface()
            .width((viewport.0 - 40.0).clamp(120.0, 640.0))
            .pad(12.0)
            .radius(9.0)
            .fill(lex.mantle)
            .stroke(lex.grounded(lex.hairc), 1.0)
            .child(
                Element::text(
                    Voice::LEDGER_NAME,
                    lex.tx,
                    if completion.is_file() {
                        "Add files"
                    } else {
                        "Commands"
                    },
                )
                .height(28.0)
                .nowrap(),
            );
        if completion.items().is_empty() {
            card = card.child(
                Element::text(
                    Voice::HINT,
                    lex.sb0,
                    "No matching files. Type a path or press Esc to close.",
                )
                .height(28.0)
                .nowrap(),
            );
        } else {
            for (index, item) in completion.items().iter().enumerate() {
                let row = control(item, Widget::completion(index), &lex)
                    .height(30.0)
                    .no_shrink()
                    .stretch();
                card = card.child(if index == completion.selected() {
                    Element::surface().fill(lex.s0).radius(4.0).child(row)
                } else {
                    row
                });
            }
        }
        Some(
            card.pin_left(20.0)
                .pin_bottom(Self::height(model, viewport.0 - 40.0, viewport.1 - 92.0) + 46.0)
                .claim(Component::Field),
        )
    }
}

fn references(text: &str) -> Result<Vec<String>, String> {
    magistr_application::sessions::prompt_file_references(text, |tail| {
        let mut stream = serde_json::Deserializer::from_str(tail).into_iter::<String>();
        let name = stream
            .next()
            .transpose()
            .map_err(|_| "A file name has an unfinished quote. Choose the file again.".to_owned())?
            .ok_or_else(|| "Choose a file after @.".to_owned())?;
        Ok((name, stream.byte_offset()))
    })
}

#[cfg(test)]
#[path = "comp_composer.tests.rs"]
mod tests;
