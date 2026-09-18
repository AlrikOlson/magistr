use super::*;

#[test]
fn failed_delivery_restores_text_and_files_in_both_destinations() {
    use magistr_cockpit::cockpit::{RequestPhase, StageTab};
    for tab in [StageTab::Turn, StageTab::Roadmap] {
        let (tx, commands) = Commands::channel();
        drop(commands);
        let route = Route::new(tx);
        let mut m = model().with_tab(tab);
        route.fold(
            &mut m,
            Msg::Paste("Review @src/main.rs\nand explain".into()),
        );
        route.fold(&mut m, key(KeyCode::Enter));
        assert_eq!(m.prompt().text(), "Review @src/main.rs\nand explain");
        if tab == StageTab::Roadmap {
            assert_eq!(
                m.roadmap_requests().last().unwrap().phase,
                RequestPhase::Failed
            );
        }
        assert!(!m.agent_activity().active);
    }
}

use magistr_application::sessions::{AgentEvent, PermissionChoice, PermissionKind};
use magistr_cockpit::cockpit::Size;
use magistr_cockpit::input::{KeyCode, KeyEvent, Modifiers};
use magistr_cockpit_runtime::channel::Commands;
use magistr_domain::policy::RiskTier;

fn model() -> Model {
    Model::new(Size::new(80, 24)).with_tab(magistr_cockpit::cockpit::StageTab::Turn)
}

fn key(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code))
}

fn drain(commands: &mut Commands) -> Vec<Cmd> {
    let mut out = Vec::new();
    while let Some(cmd) = commands.try_next() {
        out.push(cmd);
    }
    out
}

#[test]
fn a_submitted_prompt_reaches_the_commands_the_session_takes() {
    let (tx, mut commands) = Commands::channel();
    let route = Route::new(tx);
    let mut m = model();
    for c in "go".chars() {
        assert!(!route.fold(&mut m, key(KeyCode::Char(c))));
    }
    assert!(!route.fold(&mut m, key(KeyCode::Enter)));
    assert_eq!(drain(&mut commands), vec![Cmd::Prompt("go".to_owned())]);
}

#[test]
fn a_producers_event_lands_in_the_model_and_its_animation_stays_here() {
    let (tx, mut commands) = Commands::channel();
    let route = Route::new(tx);
    let mut m = model();
    assert!(!route.fold(&mut m, Msg::Agent(AgentEvent::Text("hi".to_owned()))));
    assert_eq!(m.turn().rows()[0].text, "hi");
    assert!(m.is_animating(), "the streaming turn declares motion");
    assert_eq!(
        drain(&mut commands),
        vec![],
        "Cmd::Animate is the loop's own and never a producer's"
    );
}

#[test]
fn an_announcement_is_the_loops_own_and_the_answer_to_it_is_a_producers() {
    let (tx, mut commands) = Commands::channel();
    let route = Route::new(tx);
    let mut m = model();
    route.fold(
        &mut m,
        Msg::Agent(AgentEvent::PermissionRequest {
            id: 7,
            title: "Edit main.rs".to_owned(),
            choices: vec![
                PermissionChoice::new("allow", PermissionKind::AllowOnce),
                PermissionChoice::new("reject", PermissionKind::RejectOnce),
            ],
            edit: false,
            risk: RiskTier::Medium,
            plan: None,
        }),
    );
    assert_eq!(
        drain(&mut commands),
        vec![],
        "the announce is logged, not sent"
    );
    route.fold(&mut m, key(KeyCode::Enter));
    assert_eq!(
        drain(&mut commands),
        vec![Cmd::Permission {
            id: 7,
            option_id: "allow".to_owned()
        }]
    );
}

#[test]
fn ctrl_d_on_an_empty_prompt_is_the_quit_and_a_gone_session_is_not_an_error() {
    let (tx, commands) = Commands::channel();
    drop(commands);
    let route = Route::new(tx);
    let mut m = model();
    assert!(!route.fold(&mut m, key(KeyCode::Char('x'))));
    assert!(
        !route.fold(&mut m, key(KeyCode::Enter)),
        "a refused prompt keeps the window open"
    );
    assert_eq!(m.prompt().text(), "x");
    route.fold(&mut m, key(KeyCode::Backspace));
    assert!(route.fold(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Char('d')).with(Modifiers::CTRL))
    ));
}

/// The clipboard is the window's own: `y` over a run card puts the transcript on the
/// system clipboard — or logs that it could not — and nothing reaches a producer.
#[test]
fn a_transcript_copy_is_the_loops_own_and_never_a_producers() {
    let (tx, mut commands) = Commands::channel();
    let route = Route::new(tx);
    let mut m = crate::fixture::Fixture::pane_following(Size::new(120, 40));
    assert!(!route.fold(&mut m, key(KeyCode::Char('y'))));
    assert_eq!(
        drain(&mut commands),
        vec![],
        "the clipboard is not a command"
    );
    assert!(m.prompt().is_empty(), "and nothing was typed");
}
