use magistr_cockpit::cockpit::{Msg, Size, Surface, update};
use magistr_cockpit::input::{KeyCode, KeyEvent};

use super::*;
use crate::element::Kind;
use crate::fixture::Fixture;

fn texts(root: &Element) -> Vec<String> {
    let mut texts = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            texts.push(content.clone());
        }
    });
    texts
}

/// The ask Moment is the plan card off the model's own ask: the title,
/// the numbered steps, the minted checkpoint — and answering through
/// the one `update` stands the composition down.
#[test]
fn the_ask_moment_is_the_plan_card_and_an_answer_stands_it_down() {
    let mut model = Fixture::mock("06", Size::new(120, 40)).expect("view 06 has a fixture");
    let ask = model.moment().expect("the ask is up");
    let title = ask.title().to_owned();
    let step = ask.steps().next().expect("the plan has steps").to_owned();
    let checkpoint = ask.checkpoint().expect("the spine minted one").to_owned();
    let moment =
        MomentComposition::el(&model, Medium::Night).expect("the composition follows the ask");
    let texts = texts(&moment);
    assert!(texts.contains(&title), "the ask's own title");
    assert!(
        texts.iter().any(|t| *t == format!("1. {step}")),
        "the steps are numbered"
    );
    assert!(texts.contains(&checkpoint), "the checkpoint is named");
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert!(model.moment().is_none(), "enter answered the front card");
    assert!(
        MomentComposition::el(&model, Medium::Night).is_none(),
        "and the Moment stands down with it"
    );
}

/// A plain ask — the one a live session raises, no plan on it — is the ask card on a
/// window at the Session: the *needs you* cap, the tool's title and the answer verbs;
/// the terminal keeps its strip and composes nothing; an answer stands it down.
#[test]
fn a_plain_ask_is_the_ask_card_on_a_window_and_nothing_on_a_terminal() {
    use magistr_cockpit::agent::{AgentEvent, PermissionChoice, PermissionKind};
    use magistr_engine::policy::RiskTier;
    let ask = || {
        Msg::Agent(AgentEvent::PermissionRequest {
            id: 4,
            title: "mcp__workbench-mcp__read".to_owned(),
            choices: vec![
                PermissionChoice::new("allow", PermissionKind::AllowOnce),
                PermissionChoice::new("reject", PermissionKind::RejectOnce),
            ],
            edit: false,
            risk: RiskTier::Medium,
            plan: None,
        })
    };
    let mut terminal = Model::new(Size::new(120, 40));
    update(&mut terminal, ask());
    assert!(
        MomentComposition::el(&terminal, Medium::Night).is_none(),
        "the terminal has its strip"
    );
    let mut window = Model::new(Size::new(120, 40)).with_surface(Surface::Native);
    update(&mut window, ask());
    for medium in Medium::ALL {
        let card = MomentComposition::el(&window, medium).expect("the ask is the card");
        let texts = texts(&card);
        assert!(texts.contains(&"needs you".to_owned()), "{texts:?}");
        assert!(
            texts.contains(&"mcp__workbench-mcp__read".to_owned()),
            "{texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == "allow") && texts.iter().any(|t| t == "reject"),
            "the answer verbs: {texts:?}"
        );
    }
    update(&mut window, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert!(
        MomentComposition::el(&window, Medium::Night).is_none(),
        "answered, the card stands down"
    );
}

/// The review Moment shows the change: the scope line, every journal
/// file's row, the selected diff in the diff vocabulary, and the
/// answer verbs.
#[test]
fn the_review_moment_shows_the_scope_the_files_and_the_diff() {
    let mut model = Fixture::mock("07", Size::new(120, 40)).expect("view 07 has a fixture");
    model.set_surface(Surface::Native);
    let permission = model.review().expect("the review is up");
    let verb = permission
        .buttons()
        .next()
        .expect("the review answers")
        .1
        .to_owned();
    let scope = model.review_scope();
    let file = model.review_file().expect("a file is selected");
    let path = file.path.clone();
    let moment = MomentComposition::el(&model, Medium::Night).expect("the review composes");
    let texts = texts(&moment);
    assert!(texts.contains(&scope), "the scope line");
    assert!(texts.contains(&path), "the file's row");
    assert!(
        texts.iter().any(|t| t.starts_with('+')),
        "an added diff row rides in the diff vocabulary"
    );
    assert!(texts.contains(&verb), "the answer verbs ride");
}
