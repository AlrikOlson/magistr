//! Native composer journeys use the same input route as the real window.
use magistr_application::sessions::{AgentEvent, TurnEnd};
use magistr_cockpit::cockpit::{
    Cmd, FrameClock, Model, Msg, Size, StageTab, Surface, Widget, update,
};
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, Modifiers, MouseEvent, MouseKind};
use magistr_cockpit::theme::{MotionMode, Theme, ThemeMode};
use magistr_gui2::{
    Frame, comp_composer::ComposerComposition as C, comp_workspace::WorkspaceComposition as W,
};

fn model(tab: StageTab) -> Model {
    Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_tab(tab)
        .with_workspace_files(vec!["src/main.rs".into(), "résumé draft.md".into()])
}
fn paint(frame: &mut Frame, model: &mut Model, w: u32, h: u32) -> tiny_skia::Pixmap {
    frame
        .still(model, &FrameClock::new(0, std::time::Duration::ZERO), w, h)
        .unwrap()
}
fn input(frame: &mut Frame, model: &mut Model, msg: Msg) -> Vec<Cmd> {
    frame
        .input(model, msg)
        .into_iter()
        .flat_map(|msg| update(model, msg))
        .collect()
}
fn key(frame: &mut Frame, model: &mut Model, code: KeyCode) -> Vec<Cmd> {
    input(frame, model, Msg::Key(KeyEvent::new(code)))
}
fn click(
    frame: &mut Frame,
    model: &mut Model,
    widget: magistr_cockpit::widget_id::WidgetId,
) -> Vec<Cmd> {
    let id = if widget == Widget::Composer.id() {
        C::PILL
    } else {
        W::id(widget)
    };
    let [x, y, w, h] = frame.rect_of(id).expect("visible control");
    let (x, y) = ((x + w / 2.0) as u32, (y + h / 2.0) as u32);
    let target = frame.hits().hit_at(x as f64, y as f64);
    assert_eq!(target, Some(widget));
    input(
        frame,
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0).at_px(x, y),
            target,
        },
    )
}
#[test]
fn files_navigation_send_stop_and_refusal_use_the_shared_draft() {
    for tab in [StageTab::Turn, StageTab::Roadmap] {
        let mut m = model(tab);
        let mut f = Frame::new().unwrap();
        paint(&mut f, &mut m, 1100, 760);
        input(&mut f, &mut m, Msg::Paste("Review ".into()));
        click(&mut f, &mut m, Widget::PromptFiles.id());
        paint(&mut f, &mut m, 1100, 760);
        assert!(m.completion().is_some());
        click(&mut f, &mut m, Widget::completion(1));
        assert_eq!(m.prompt().text(), "Review @\"résumé draft.md\" ");
        let original = m.prompt().text().to_owned();
        click(
            &mut f,
            &mut m,
            if tab == StageTab::Turn {
                Widget::TabRoadmap
            } else {
                Widget::TabTurn
            }
            .id(),
        );
        paint(&mut f, &mut m, 1100, 760);
        click(
            &mut f,
            &mut m,
            if tab == StageTab::Turn {
                Widget::TabTurn
            } else {
                Widget::TabRoadmap
            }
            .id(),
        );
        paint(&mut f, &mut m, 1100, 760);
        assert_eq!(m.prompt().text(), original);
        let cmds = click(&mut f, &mut m, Widget::PromptAction.id());
        assert!(cmds.contains(&if tab == StageTab::Turn {
            Cmd::Prompt(original.clone())
        } else {
            Cmd::RoadmapPrompt(original.clone())
        }));
        input(&mut f, &mut m, Msg::Paste("Keep @src/main.rs".into()));
        paint(&mut f, &mut m, 1100, 760);
        let draft = m.prompt().clone();
        let pointer = click(&mut f, &mut m, Widget::PromptAction.id());
        let keyboard = input(
            &mut f,
            &mut m,
            Msg::Key(KeyEvent::new(KeyCode::Char('.')).with(Modifiers::SUPER)),
        );
        assert_eq!(pointer, keyboard);
        assert_eq!(m.prompt(), &draft);
        assert_eq!(m.tab(), tab);
        update(
            &mut m,
            Msg::Agent(AgentEvent::PromptRejected {
                text: original.clone(),
                error: "Images are unavailable in this session.".into(),
            }),
        );
        assert_eq!(m.prompt(), &draft, "a newer draft wins");
        assert!(m.prompt().history().contains(&original));
    }
}
#[test]
fn completion_enter_does_not_send_and_tab_can_activate_send() {
    let mut m = model(StageTab::Turn);
    let mut f = Frame::new().unwrap();
    paint(&mut f, &mut m, 1100, 760);
    click(&mut f, &mut m, Widget::PromptFiles.id());
    paint(&mut f, &mut m, 1100, 760);
    assert!(key(&mut f, &mut m, KeyCode::Enter).is_empty());
    assert_eq!(m.prompt().text(), "@src/main.rs ");
    paint(&mut f, &mut m, 1100, 760);
    assert_eq!(f.focus_target(), Some(Widget::Composer.id()));
    input(&mut f, &mut m, Msg::Paste("explain".into()));
    paint(&mut f, &mut m, 1100, 760);
    assert_eq!(f.focus_target(), Some(Widget::Composer.id()));
    key(&mut f, &mut m, KeyCode::Tab);
    assert_eq!(f.focus_target(), Some(Widget::PromptFiles.id()));
    key(&mut f, &mut m, KeyCode::Tab);
    assert_eq!(f.focus_target(), Some(Widget::PromptAction.id()));
    assert!(
        key(&mut f, &mut m, KeyCode::Enter).contains(&Cmd::Prompt("@src/main.rs explain".into()))
    );
}
#[test]
fn bounded_editor_and_chooser_fit_both_themes_and_display_scales() {
    let target = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    for (theme, mode) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light)] {
        for (layout, w, h, scale) in [
            ("wide", 1100, 760, 1.0),
            ("compact", 620, 640, 1.0),
            ("scaled", 1240, 1280, 2.0),
        ] {
            let mut f = Frame::new().unwrap();
            f.rescale(scale).unwrap();
            let mut m = model(StageTab::Turn)
                .with_theme(Theme::forced(mode).with_motion(MotionMode::Reduced));
            paint(&mut f, &mut m, w, h);
            let empty_height = f.rect_of(C::PILL).unwrap()[3];
            input(
                &mut f,
                &mut m,
                Msg::Paste("First line with @src/main.rs\n".repeat(30)),
            );
            let image = paint(&mut f, &mut m, w, h);
            let [x, y, ew, eh] = f.rect_of(C::PILL).unwrap();
            assert!(eh > empty_height && eh <= 140.0 * scale as f32);
            assert!(x >= 0.0 && y >= 0.0 && x + ew <= w as f32 && y + eh <= h as f32);
            assert_eq!(m.prompt().rows(), 31);
            image
                .save_png(target.join(format!("composer-{layout}-{theme}.png")))
                .unwrap();
            let before = m.clone();
            assert_eq!(image.data(), paint(&mut f, &mut m, w, h).data());
            assert_eq!(m, before);
            click(&mut f, &mut m, Widget::PromptFiles.id());
            paint(&mut f, &mut m, w, h)
                .save_png(target.join(format!("composer-files-{layout}-{theme}.png")))
                .unwrap();
            assert!(
                f.access()
                    .nodes()
                    .iter()
                    .any(|node| node.label == "résumé draft.md")
            );
        }
    }
}
#[test]
fn pointer_caret_and_preedit_preserve_the_committed_draft() {
    let mut m = model(StageTab::Turn);
    let mut f = Frame::new().unwrap();
    input(&mut f, &mut m, Msg::Paste("Wi e\u{301}🙂\nsecond".into()));
    paint(&mut f, &mut m, 1100, 760);
    let [x, y, _, _] = f.rect_of(C::PILL).unwrap();
    input(
        &mut f,
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0)
                .at_px(x as u32, (y + 3.0) as u32),
            target: Some(Widget::Composer.id()),
        },
    );
    assert_eq!(m.prompt().cursor(), 0);
    let draft = m.prompt().clone();
    assert!(f.set_preedit("你好"));
    paint(&mut f, &mut m, 1100, 760);
    assert_eq!(m.prompt(), &draft);
    assert!(f.says().iter().any(|(text, _)| text.starts_with("你好Wi")));
    assert_eq!(
        f.access().find(Widget::Composer).unwrap().value.as_deref(),
        Some(draft.text()),
        "composition preview is not committed input"
    );
    assert!(f.set_preedit(""));
    input(&mut f, &mut m, Msg::Paste("你好".into()));
    assert_eq!(m.prompt().text(), "你好Wi e\u{301}🙂\nsecond");
}

#[test]
fn historical_completion_does_not_hide_new_activity() {
    let mut m = model(StageTab::Turn);
    let mut f = Frame::new().unwrap();
    update(&mut m, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    update(&mut m, Msg::Agent(AgentEvent::Text("New answer".into())));
    paint(&mut f, &mut m, 1100, 760);
    assert!(
        f.says()
            .iter()
            .any(|(s, _)| s.contains("Writing a response"))
    );
    assert!(f.says().iter().any(|(s, _)| s == "Stop"));
}
