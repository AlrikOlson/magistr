//! The decision feature's own contract: every route to an answer is
//! the same answer, a rejected answer leaves the pending state as it
//! was, the reducer speaks only for the provider's ask and the policy
//! owner's rule, and the effects it produces keep their order.

use super::*;
use crate::agent::{AgentEvent, AskPlan, PermissionChoice, PermissionKind};
use crate::cockpit::update::update;
use crate::cockpit::{
    CheckpointCmd, DockItem, EngineCmd, MotionName, Msg, Selected, Size, Surface, TurnState,
    WorkbenchEvent,
};
use crate::input::{Button, KeyCode, KeyEvent, Modifiers, MouseEvent, MouseKind};
use crate::workbench::JournalEntry;
use magistr_engine::policy::RiskTier;

fn model() -> Model {
    Model::new(Size::new(80, 24)).with_tab(crate::cockpit::StageTab::Turn)
}

fn key(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code))
}

fn shifted(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code).with(Modifiers::SHIFT))
}

fn click(widget: Widget) -> Msg {
    Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 20),
        target: Some(widget.id()),
    }
}

fn intent(intent: DecisionIntent) -> Msg {
    Msg::Decision(intent)
}

fn choices() -> Vec<PermissionChoice> {
    vec![
        PermissionChoice::new("allow", PermissionKind::AllowOnce),
        PermissionChoice::new("allow_always", PermissionKind::AllowAlways),
        PermissionChoice::new("reject", PermissionKind::RejectOnce),
    ]
}

/// A plain tool ask — the dock's one-line strip.
fn plain(id: u64) -> AgentEvent {
    AgentEvent::PermissionRequest {
        id,
        title: "Bash".to_owned(),
        choices: choices(),
        edit: false,
        risk: RiskTier::Medium,
        plan: None,
    }
}

/// A plan ask — the Moment.
fn plan(id: u64) -> AgentEvent {
    AgentEvent::PermissionRequest {
        id,
        title: "Re-bless goldens".to_owned(),
        choices: choices(),
        edit: false,
        risk: RiskTier::Medium,
        plan: Some(AskPlan::new(["Render", "Compare"]).scoped("goldens")),
    }
}

/// An edit ask — a review on a window.
fn edit(id: u64, path: &str) -> AgentEvent {
    AgentEvent::PermissionRequest {
        id,
        title: format!("Edit {path}"),
        choices: choices(),
        edit: true,
        risk: RiskTier::Medium,
        plan: None,
    }
}

fn answered(id: u64, option_id: &str) -> Cmd {
    Cmd::Permission {
        id,
        option_id: option_id.to_owned(),
    }
}

/// Only the commands the decision is about — the fold's own animation
/// bookkeeping rides beside every message and says nothing here.
fn decided(cmds: Vec<Cmd>) -> Vec<Cmd> {
    cmds.into_iter()
        .filter(|cmd| !matches!(cmd, Cmd::Animate(_) | Cmd::Still))
        .collect()
}

/// A native cockpit with a verified checkpoint, a journal and `ask`
/// waiting — the review surface's model.
fn reviewing(ask: AgentEvent) -> Model {
    let mut m = Model::new(Size::new(125, 32)).with_surface(Surface::Native);
    {
        let spine = m.rail_mut().spine_mut();
        let id = spine.open(magistr_engine::CheckpointReason::Iteration, "iter 1");
        spine.saved(&id, Some("abc123".to_owned()), true);
    }
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![JournalEntry {
                at_ms: None,
                id: "e1".to_owned(),
                path: "theme/ink.rs".to_owned(),
                added: 3,
                removed: 1,
                parse_ok: true,
                diff: "+new".to_owned(),
            }],
        }),
    );
    update(&mut m, Msg::Agent(ask));
    m
}

// ── Every route is the same answer ───────────────────────────────

#[test]
fn the_strip_answers_the_same_by_key_by_button_and_by_intent() {
    let routes: [(&str, Msg); 3] = [
        ("key", key(KeyCode::Char('a'))),
        ("button", click(Widget::Allow)),
        (
            "intent",
            intent(DecisionIntent::Answer(PermissionKind::AllowOnce)),
        ),
    ];
    for (route, msg) in routes {
        let mut m = model();
        update(&mut m, Msg::Agent(plain(1)));
        assert_eq!(
            decided(update(&mut m, msg)),
            vec![answered(1, "allow")],
            "{route}"
        );
        assert_eq!(m.permissions_pending(), 0, "{route}: the card left");
    }
}

#[test]
fn the_moment_answers_the_same_by_key_by_button_and_by_intent() {
    let routes: [(&str, Msg); 3] = [
        ("key", shifted(KeyCode::Char('A'))),
        ("button", click(Widget::Always)),
        (
            "intent",
            intent(DecisionIntent::Answer(PermissionKind::AllowAlways)),
        ),
    ];
    for (route, msg) in routes {
        let mut m = model();
        update(&mut m, Msg::Agent(plan(2)));
        assert!(m.moment().is_some(), "{route}: a plan ask is a Moment");
        assert_eq!(
            decided(update(&mut m, msg)),
            vec![answered(2, "allow_always")],
            "{route}"
        );
        assert!(m.moment().is_none(), "{route}: the card left");
        assert!(
            m.rail()
                .charter()
                .iter()
                .any(|rule| rule.subject() == "always allow · goldens"),
            "{route}: the standing answer is on the charter: {:?}",
            m.rail().charter()
        );
    }
}

#[test]
fn the_review_answers_the_same_by_key_by_button_and_by_intent() {
    let routes: [(&str, Msg); 3] = [
        ("key", key(KeyCode::Enter)),
        ("button", click(Widget::Allow)),
        (
            "intent",
            intent(DecisionIntent::Answer(PermissionKind::AllowOnce)),
        ),
    ];
    for (route, msg) in routes {
        let mut m = reviewing(edit(3, "theme/ink.rs"));
        assert!(m.review().is_some(), "{route}: an edit ask is a review");
        assert_eq!(
            decided(update(&mut m, msg)),
            vec![
                answered(3, "allow"),
                Cmd::Checkpoint(CheckpointCmd::Merge {
                    id: "cp-1".to_owned()
                }),
            ],
            "{route}: the answer, then the merge beside it"
        );
    }
}

#[test]
fn a_written_condition_travels_the_same_by_keys_and_by_intents() {
    let by_keys = {
        let mut m = model();
        update(&mut m, Msg::Agent(plan(4)));
        update(&mut m, key(KeyCode::Tab));
        for c in "allow, stop if rows change".chars() {
            update(&mut m, key(KeyCode::Char(c)));
        }
        (decided(update(&mut m, key(KeyCode::Enter))), m)
    };
    let by_intents = {
        let mut m = model();
        update(&mut m, Msg::Agent(plan(4)));
        update(&mut m, intent(DecisionIntent::OpenCondition));
        for c in "allow, stop if rows change".chars() {
            update(&mut m, intent(DecisionIntent::Type(c)));
        }
        (
            decided(update(
                &mut m,
                intent(DecisionIntent::Answer(PermissionKind::AllowOnce)),
            )),
            m,
        )
    };
    assert_eq!(
        by_keys.0,
        vec![
            Cmd::Constraint {
                id: 4,
                condition: "allow, stop if rows change".to_owned(),
            },
            answered(4, "allow"),
        ],
        "the constraint on the ask, then the answer that closes it"
    );
    assert_eq!(by_keys.0, by_intents.0);
    assert_eq!(by_keys.1.rail().charter(), by_intents.1.rail().charter());
    assert!(
        by_intents.1.condition().is_empty(),
        "the register was spent"
    );
    assert!(!by_intents.1.condition_open());
}

#[test]
fn the_registers_editing_intents_are_the_keys() {
    let mut keys = model();
    update(&mut keys, Msg::Agent(plan(5)));
    update(&mut keys, key(KeyCode::Tab));
    for c in "abd".chars() {
        update(&mut keys, key(KeyCode::Char(c)));
    }
    update(&mut keys, key(KeyCode::Left));
    update(&mut keys, key(KeyCode::Backspace));
    update(&mut keys, key(KeyCode::Right));
    update(&mut keys, key(KeyCode::Char('c')));

    let mut intents = model();
    update(&mut intents, Msg::Agent(plan(5)));
    for i in [
        DecisionIntent::OpenCondition,
        DecisionIntent::Type('a'),
        DecisionIntent::Type('b'),
        DecisionIntent::Type('d'),
        DecisionIntent::Left,
        DecisionIntent::Backspace,
        DecisionIntent::Right,
        DecisionIntent::Type('c'),
    ] {
        update(&mut intents, intent(i));
    }
    assert_eq!(keys.condition().text(), "adc");
    assert_eq!(keys.condition(), intents.condition());
    assert_eq!(
        decided(update(&mut keys, key(KeyCode::Esc))),
        vec![],
        "Esc clears the register first"
    );
    assert_eq!(
        decided(update(&mut intents, intent(DecisionIntent::ClearCondition))),
        vec![]
    );
    assert_eq!(keys.condition(), intents.condition());
    assert!(keys.condition().is_empty());
    assert!(keys.moment().is_some() && intents.moment().is_some());
}

#[test]
fn show_images_is_one_intent_reached_by_the_key_and_the_button() {
    use crate::agent::ImageBlock;
    let picture = || AgentEvent::PermissionRequest {
        id: 6,
        title: "Look".to_owned(),
        choices: choices(),
        edit: false,
        risk: RiskTier::Medium,
        plan: Some(AskPlan::new(["Render"]).with_images(vec![ImageBlock {
            mime_type: "image/png".to_owned(),
            data: "not-a-png".to_owned(),
            uri: None,
        }])),
    };
    for msg in [
        key(KeyCode::Char(' ')),
        click(Widget::ShowImages),
        intent(DecisionIntent::ShowImages),
    ] {
        let mut m = model();
        update(&mut m, Msg::Agent(picture()));
        assert_eq!(decided(update(&mut m, msg.clone())), vec![], "{msg:?}");
        let moment = m.moment().expect("the card keeps its place");
        assert!(moment.hidden_images().is_empty(), "{msg:?}: handed over");
        assert!(
            m.turn()
                .rows()
                .iter()
                .any(|row| row.text.contains("could not be shown")),
            "{msg:?}: the picture went to the feed through the one route: {:?}",
            m.turn()
                .rows()
                .iter()
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>()
        );
    }
}

// ── A rejected answer restores the pending state ─────────────────

#[test]
fn an_answer_the_ask_does_not_offer_leaves_the_moment_as_it_was() {
    for msg in [
        shifted(KeyCode::Char('D')),
        click(Widget::Never),
        intent(DecisionIntent::Answer(PermissionKind::RejectAlways)),
    ] {
        let mut m = model();
        update(&mut m, Msg::Agent(plan(7)));
        assert_eq!(decided(update(&mut m, msg.clone())), vec![], "{msg:?}");
        assert!(m.moment().is_some(), "{msg:?}: the card is still up");
        assert!(m.condition().is_empty(), "{msg:?}: a dead key is not text");
        assert!(!m.condition_open(), "{msg:?}: and opens nothing");
        assert!(
            m.rail().charter().is_empty(),
            "{msg:?}: no rule was written"
        );
        assert_eq!(m.permissions_pending(), 1);
    }
    // With words in the register a letter is text (the keys' own
    // régime); the button and the intent still name an answer, and a
    // refused one leaves the words where they were.
    for msg in [
        click(Widget::Never),
        intent(DecisionIntent::Answer(PermissionKind::RejectAlways)),
    ] {
        let mut m = model();
        update(&mut m, Msg::Agent(plan(7)));
        update(&mut m, key(KeyCode::Tab));
        update(&mut m, key(KeyCode::Char('x')));
        assert_eq!(decided(update(&mut m, msg.clone())), vec![], "{msg:?}");
        assert!(m.moment().is_some(), "{msg:?}: the card is still up");
        assert_eq!(m.condition().text(), "x", "{msg:?}: the words survive");
        assert!(m.condition_open(), "{msg:?}: and the register stays open");
        assert!(
            m.rail().charter().is_empty(),
            "{msg:?}: no rule was written"
        );
    }
}

#[test]
fn an_answer_the_ask_does_not_offer_leaves_the_strip_as_it_was() {
    for msg in [
        shifted(KeyCode::Char('D')),
        click(Widget::Never),
        intent(DecisionIntent::Answer(PermissionKind::RejectAlways)),
    ] {
        let mut m = model();
        update(&mut m, Msg::Agent(plain(8)));
        update(&mut m, Msg::Agent(plain(9)));
        assert_eq!(decided(update(&mut m, msg.clone())), vec![], "{msg:?}");
        assert_eq!(m.permissions_pending(), 2, "{msg:?}");
        assert!(
            matches!(m.dock().front(), Some(DockItem::Permission(p)) if p.id() == 8),
            "{msg:?}: the front is the ask it was"
        );
    }
}

#[test]
fn an_intent_with_nothing_pending_does_nothing() {
    let mut m = model();
    for i in [
        DecisionIntent::Answer(PermissionKind::AllowOnce),
        DecisionIntent::ShowImages,
        DecisionIntent::OpenCondition,
        DecisionIntent::ClearCondition,
        DecisionIntent::Type('a'),
        DecisionIntent::Backspace,
        DecisionIntent::Left,
        DecisionIntent::Right,
        DecisionIntent::Scroll(3),
    ] {
        assert_eq!(decided(update(&mut m, intent(i))), vec![], "{i:?}");
    }
    assert!(m.condition().is_empty(), "a semantic key is not text");
    assert!(!m.condition_open());
    assert!(m.prompt().is_empty(), "and never the prompt's");
    assert_eq!(m.decision().scroll(), 0);
}

// ── Authorities stay distinct ────────────────────────────────────

#[test]
fn the_decision_speaks_only_for_the_provider_and_the_policy() {
    let intents = [
        DecisionIntent::Answer(PermissionKind::AllowOnce),
        DecisionIntent::Answer(PermissionKind::AllowAlways),
        DecisionIntent::Answer(PermissionKind::RejectOnce),
        DecisionIntent::Answer(PermissionKind::RejectAlways),
        DecisionIntent::ShowImages,
        DecisionIntent::OpenCondition,
        DecisionIntent::Type('y'),
        DecisionIntent::ClearCondition,
        DecisionIntent::Backspace,
        DecisionIntent::Left,
        DecisionIntent::Right,
        DecisionIntent::Scroll(1),
    ];
    let asks = [plain(10), plan(11), edit(12, "theme/ink.rs")];
    for ask in asks {
        let mut m = reviewing(ask);
        let mut seen = Vec::new();
        for i in intents {
            for cmd in decided(update(&mut m, intent(i))) {
                assert!(
                    matches!(
                        cmd,
                        Cmd::Permission { .. }
                            | Cmd::Constraint { .. }
                            | Cmd::Checkpoint(CheckpointCmd::Merge { .. })
                    ),
                    "{i:?} produced {cmd:?}: a decision never speaks for the \
                     workspace, the engine or the loop"
                );
                seen.push(cmd);
            }
        }
        assert!(
            seen.iter().any(|cmd| matches!(cmd, Cmd::Permission { .. })),
            "the ask was answered along the way: {seen:?}"
        );
    }
}

#[test]
fn a_proposal_key_never_answers_a_pending_permission() {
    let mut m = model();
    update(&mut m, Msg::Agent(plain(13)));
    let cmds = decided(update(&mut m, key(KeyCode::Char('y'))));
    assert_eq!(
        cmds,
        vec![],
        "`y` is neither an answer nor a proposal verb here"
    );
    assert_eq!(m.permissions_pending(), 1);
    assert!(
        !cmds
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Engine(EngineCmd::ResolveProposal { .. }))),
    );
}

#[test]
fn a_workspace_approval_answers_only_the_edit_ask_that_names_the_file() {
    // A plain ask whose title happens to name the file is the
    // provider's own question, not a diff to approve.
    let mut m = reviewing(AgentEvent::PermissionRequest {
        id: 14,
        title: "Bash cat theme/ink.rs".to_owned(),
        choices: choices(),
        edit: false,
        risk: RiskTier::Medium,
        plan: None,
    });
    assert_eq!(decided(approve_edit(&mut m, "theme/ink.rs")), vec![]);
    assert_eq!(m.permissions_pending(), 1, "the plain ask still waits");

    let mut m = reviewing(edit(15, "theme/ink.rs"));
    assert_eq!(
        decided(approve_edit(&mut m, "theme/ink.rs")),
        vec![answered(15, "allow")],
        "the edit ask is answered with its plain allow, and nothing merges: \
         the Changes tab's Approve is a reviewed-mark, not the review's button"
    );
    assert_eq!(m.permissions_pending(), 0);
}

// ── Effects keep their order ─────────────────────────────────────

#[test]
fn a_risky_ask_announces_before_it_asks_for_the_checkpoint() {
    let mut m = model();
    let cmds = decided(update(
        &mut m,
        Msg::Agent(AgentEvent::PermissionRequest {
            id: 16,
            title: "rm -rf build".to_owned(),
            choices: choices(),
            edit: false,
            risk: RiskTier::High,
            plan: None,
        }),
    ));
    assert_eq!(
        cmds,
        vec![
            Cmd::Announce("needs you: rm -rf build".to_owned()),
            Cmd::Checkpoint(CheckpointCmd::Save {
                id: "cp-1".to_owned(),
                reason: magistr_engine::CheckpointReason::Risk,
                label: "rm -rf build".to_owned(),
            }),
        ]
    );
    assert_eq!(m.permissions_seen(), 1);
}

#[test]
fn a_cancelled_ask_stands_back_up_and_a_lapsed_one_says_so() {
    let mut m = Model::new(Size::new(125, 32)).with_surface(Surface::Native);
    update(&mut m, Msg::Agent(plan(17)));
    assert!(m.moment().is_some());
    // The window's Esc stands the card down to the strip.
    update(&mut m, key(KeyCode::Esc));
    assert!(m.moment().is_none(), "stood down");
    assert!(m.decision().is_stood_down(17));
    assert_eq!(m.permissions_pending(), 1, "but still pending");
    assert_eq!(
        decided(update(
            &mut m,
            Msg::Agent(AgentEvent::PermissionCancelled { id: 17 })
        )),
        vec![],
        "a cancellation is silent"
    );
    assert!(!m.decision().is_stood_down(17));
    assert_eq!(m.permissions_pending(), 0);

    update(&mut m, Msg::Agent(plan(18)));
    update(&mut m, key(KeyCode::Esc));
    let cmds = decided(update(
        &mut m,
        Msg::Agent(AgentEvent::PermissionLapsed {
            id: 18,
            title: "Re-bless goldens".to_owned(),
            after_secs: 90,
        }),
    ));
    assert_eq!(
        cmds,
        vec![Cmd::Announce(
            "denied after 1 min: nobody answered Re-bless goldens".to_owned()
        )]
    );
    assert!(!m.decision().is_stood_down(18));
    assert_eq!(m.permissions_pending(), 0);
}

#[test]
fn an_answer_moves_neither_the_camera_nor_the_selection() {
    let mut m = Model::new(Size::new(125, 32)).with_surface(Surface::Native);
    m.inspect(Selected::Step(3));
    let place = m.place();
    update(&mut m, Msg::Agent(plan(19)));
    update(
        &mut m,
        intent(DecisionIntent::Answer(PermissionKind::AllowOnce)),
    );
    assert_eq!(m.place(), place);
    assert_eq!(m.selected(), Some(&Selected::Step(3)));
}

#[test]
fn a_moment_holds_still_and_answering_it_hands_the_motion_back() {
    let mut m = model();
    update(&mut m, Msg::Agent(AgentEvent::Text("working".to_owned())));
    assert_eq!(m.turn_state(), TurnState::Streaming);
    assert!(
        m.motions_in_flight().contains(&MotionName::Sweep),
        "the streaming turn sweeps"
    );
    update(&mut m, Msg::Agent(plan(20)));
    assert!(
        m.motions_in_flight().is_empty(),
        "a decision screen moves nothing"
    );
    assert!(
        m.is_animating(),
        "but the tick the turn asked for is not taken away by the ask"
    );
    update(
        &mut m,
        intent(DecisionIntent::Answer(PermissionKind::AllowOnce)),
    );
    assert!(m.motions_in_flight().contains(&MotionName::Sweep));
}

#[test]
fn the_strips_scroll_is_unclamped_and_the_moments_is_clamped_by_its_card() {
    let mut m = model();
    update(&mut m, Msg::Agent(plain(21)));
    m.decision.scroll_by(3, usize::MAX);
    assert_eq!(m.decision().scroll(), 3);
    m.decision.scroll_by(-5, usize::MAX);
    assert_eq!(m.decision().scroll(), 0, "never below the top");
    m.decision.scroll_by(9, 4);
    assert_eq!(m.decision().scroll(), 4, "never past the card's end");
    update(&mut m, Msg::Agent(plan(22)));
    update(
        &mut m,
        intent(DecisionIntent::Answer(PermissionKind::AllowOnce)),
    );
    update(
        &mut m,
        intent(DecisionIntent::Answer(PermissionKind::AllowOnce)),
    );
    assert_eq!(
        m.decision().scroll(),
        0,
        "an answered Moment scrolls back to the top"
    );
}
