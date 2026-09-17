use magistr_application::sessions::{AgentEvent, PermissionChoice, PermissionKind};
use magistr_cockpit::cockpit::{
    LauncherData, LauncherSession, Model, Msg, SessionOutcome, Size, Surface, Widget, update,
};
use magistr_cockpit::input::{KeyCode, KeyEvent, Modifiers};
use magistr_cockpit::job::{JobId, JobOutput};
use magistr_domain::policy::RiskTier;

use super::*;
use crate::comp_root::RootComposition;
use crate::element::Kind;

/// The capture's own state, folded through the real `update`: the
/// project named think-and-ship, one plan-less ask pending, ⌘K, `thi`.
fn palette_open() -> Model {
    let mut model = Model::new(Size::new(120, 40)).with_surface(Surface::Native);
    update(
        &mut model,
        Msg::Job {
            id: JobId::new(0),
            output: JobOutput::Launcher(Box::new(LauncherData {
                project: "think-and-ship".to_owned(),
                has_config: true,
                stack: "Rust / Cargo".to_owned(),
                git: "master, clean".to_owned(),
                backend: "ACP (available)".to_owned(),
                sessions: vec![LauncherSession {
                    id: Some("sess-1".to_owned()),
                    when: "Aug 30 14:02".to_owned(),
                    duration: "12m".to_owned(),
                    outcome: SessionOutcome::Success,
                    preview: "shipped the palette".to_owned(),
                }],
            })),
        },
    );
    update(
        &mut model,
        Msg::Agent(AgentEvent::PermissionRequest {
            id: 1,
            title: "Write a file".to_owned(),
            choices: vec![PermissionChoice::new("allow", PermissionKind::AllowOnce)],
            edit: false,
            risk: RiskTier::Low,
            plan: None,
        }),
    );
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Char('k')).with(Modifiers::SUPER)),
    );
    for c in "thi".chars() {
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
    }
    model
}

fn texts(root: &Element) -> Vec<String> {
    let mut texts = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            texts.push(content.clone());
        }
    });
    texts
}

#[test]
fn the_palette_rises_with_the_model_and_shows_the_captures_rows() {
    let closed = Model::new(Size::new(120, 40));
    assert!(PaletteComposition::el(&closed, Medium::Night).is_none());

    let model = palette_open();
    let float = PaletteComposition::el(&model, Medium::Night).expect("⌘K raised it");
    let texts = texts(&float);
    assert!(texts.iter().any(|t| t == "\u{2318}K"), "the chord");
    assert!(texts.iter().any(|t| t == "thi"), "the query");
    assert!(texts.iter().any(|t| t == "Open\u{a0}"), "the place verb");
    assert!(texts.iter().any(|t| t == "think-and-ship"), "the identity");
    assert!(
        texts.iter().any(|t| t == "1 action required"),
        "the live badge"
    );
    assert!(
        texts.iter().any(|t| t == "Answer permission request"),
        "the pending act under the chosen row"
    );
    assert!(texts.iter().any(|t| t == "\u{21b5}"), "the chosen hint");
    assert!(
        texts.iter().any(|t| t == "\u{2318}\u{21b5}"),
        "the act hint"
    );
}

#[test]
fn the_chosen_row_wears_the_wash_and_the_sigil_is_the_volumes() {
    let model = palette_open();
    let float = PaletteComposition::el(&model, Medium::Night).expect("open");
    let mut washed = None;
    let mut sigils = 0;
    float.each(&mut |_, element| {
        if element.id == Some(ElementId(PaletteComposition::ROWS)) {
            washed = Some(element.fill.is_some());
        }
        if let Kind::Mark {
            mark: Mark::Sigil(name),
            ..
        } = element.kind
        {
            assert_eq!(name, "think-and-ship");
            sigils += 1;
        }
    });
    assert_eq!(washed, Some(true), "the chosen row is washed");
    assert_eq!(sigils, 1, "the place carries the project sigil");
}

#[test]
fn the_root_floats_the_palette_with_no_scrim_and_routes_its_rows() {
    let model = palette_open();
    let clock = magistr_cockpit::cockpit::FrameClock::new(0, std::time::Duration::ZERO);
    let root = RootComposition::el(&model, Medium::Night, (8.0, 17.0), (1100.0, 640.0), &clock);
    let mut found = false;
    root.each(&mut |_, element| {
        if element.id == Some(ElementId(PaletteComposition::ROWS)) {
            found = true;
        }
    });
    assert!(found, "the root composes the palette's rows");
    assert_eq!(
        RootComposition::target_of(&model, ElementId(PaletteComposition::ROWS)),
        Some(Widget::palette_row(0)),
        "a row click reaches the shared update as its widget"
    );
    assert_eq!(
        RootComposition::target_of(&model, ElementId(PaletteComposition::ROWS + 3)),
        Some(Widget::palette_row(3)),
    );
}
