use magistr_application::sessions::{AgentEvent, PermissionChoice, PermissionKind};
use magistr_cockpit::cockpit::{LoopEvent, LoopStage, Msg, Size, Surface, update};
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit::harness::fixtures;
use magistr_domain::policy::RiskTier;
use magistr_workflow_contracts::NativeSnapshot;

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

fn has(root: &Element, needle: &str) -> bool {
    texts(root).iter().any(|t| t == needle)
}

fn has_id(root: &Element, id: ElementId) -> bool {
    let mut found = false;
    root.each(&mut |_, element| found |= element.id == Some(id));
    found
}

/// A window on chunk `id` with `title` on the roadmap.
fn on_chunk(id: &str, title: &str) -> Model {
    let mut model = Model::new(Size::new(120, 40));
    model.set_surface(Surface::Native);
    update(
        &mut model,
        Msg::Engine(EngineEvent::Seed(NativeSnapshot {
            progress: fixtures::progress(0, 0),
            chunks: fixtures::chunks_from(vec![serde_json::json!({
                "id": id, "title": title, "status": "in_progress", "priority": 100
            })]),
            signals: fixtures::signals_from(Vec::new()),
            ..NativeSnapshot::default()
        })),
    );
    model
}

/// View 05's band is the eighth edition's row: the chunk chip with the
/// roadmap's title, the agent's `model · mode`, the iteration, the
/// spend through Copy, and the stage word — every slot a component.
#[test]
fn view_05s_band_is_the_one_row_of_essentials() {
    let model = Fixture::mock("05", Size::new(120, 40)).expect("view 05 has a fixture");
    let band = BandComposition::el(&model, Medium::Night);
    assert!(
        has(&band, "gui2-2a \u{b7} The design language"),
        "{:?}",
        texts(&band)
    );
    assert!(
        has_id(&band, BandComposition::CHUNK),
        "the chunk chip is the control"
    );
    assert!(
        has(&band, "sonnet \u{b7} Draft mode"),
        "the agent block's identity"
    );
    assert!(has(&band, "4 / 12"), "the iteration");
    assert!(has(&band, "1.84 USD"), "the spend, through Copy");
    assert!(has(&band, "agent working"), "the stage word");
    assert!(
        !texts(&band).iter().any(|t| t.contains("Iteration")),
        "no template word: the counter is the pair"
    );
}

/// Every slot arrives with its news: a fresh window's band is empty —
/// no chunk chip, no agent, no counter, no spend, no stage word.
#[test]
fn a_fresh_band_is_quiet() {
    let mut model = Model::new(Size::new(120, 40));
    model.set_surface(Surface::Native);
    let band = BandComposition::el(&model, Medium::Night);
    assert!(texts(&band).is_empty(), "{:?}", texts(&band));
    assert!(!has_id(&band, BandComposition::CHUNK));
}

/// The chunk chip names the id and the title; a title the roadmap does
/// not hold leaves the id alone; a long title is cut at a word with the
/// ellipsis so the chip stays a name.
#[test]
fn the_chunk_chip_names_the_id_and_a_title_that_fits() {
    let model = on_chunk("gui2-2a", "The design language");
    let band = BandComposition::el(&model, Medium::Night);
    assert!(has(&band, "gui2-2a \u{b7} The design language"));

    let mut bare = Model::new(Size::new(120, 40));
    bare.set_surface(Surface::Native);
    update(
        &mut bare,
        Msg::Engine(EngineEvent::Seed(NativeSnapshot {
            progress: fixtures::progress(0, 0),
            chunks: fixtures::chunks_from(vec![
                serde_json::json!({"id": "orphan", "status": "in_progress"}),
            ]),
            signals: fixtures::signals_from(Vec::new()),
            ..NativeSnapshot::default()
        })),
    );
    let band = BandComposition::el(&bare, Medium::Night);
    assert!(has(&band, "orphan"), "the id alone: {:?}", texts(&band));

    let long = on_chunk(
        "band",
        "The session says what it is: the cap becomes the identity band with every essential",
    );
    let band = BandComposition::el(&long, Medium::Night);
    let label = texts(&band)
        .into_iter()
        .find(|t| t.starts_with("band"))
        .expect("the chip");
    assert!(label.ends_with('\u{2026}'), "{label}");
    assert!(
        label.chars().count() <= "band \u{b7} ".len() + BandComposition::TITLE_CHARS + 1,
        "{label}"
    );
    assert!(
        !label.ends_with(" \u{2026}"),
        "cut at a word, no trailing space: {label}"
    );
}

/// The stage word follows the loop: the driver's word between agents,
/// `paused` while it waits, `loop ended` once it stopped — and with no
/// loop at all, `agent working` only while the agent works.
#[test]
fn the_stage_word_follows_the_loop() {
    let mut model = Model::new(Size::new(120, 40));
    model.set_surface(Surface::Native);
    update(
        &mut model,
        Msg::Loop(LoopEvent::Started {
            iteration: 4,
            max: 12,
        }),
    );
    let word = |model: &Model| {
        texts(&BandComposition::el(model, Medium::Night))
            .into_iter()
            .find(|t| t != "4 / 12")
    };
    assert_eq!(word(&model).as_deref(), Some("agent working"));
    update(&mut model, Msg::Loop(LoopEvent::Stage(LoopStage::Gates)));
    assert_eq!(word(&model).as_deref(), Some("gates running"));
    update(
        &mut model,
        Msg::Loop(LoopEvent::Paused("gates failed".to_owned())),
    );
    assert_eq!(word(&model).as_deref(), Some("paused"));
    update(
        &mut model,
        Msg::Loop(LoopEvent::Ended("max iterations".to_owned())),
    );
    assert_eq!(word(&model).as_deref(), Some("loop ended"));
}

/// A parked loop is never silent: while a plain ask fronts the dock the band names the
/// wait in the strip's own words, in place of the stage word.
#[test]
fn a_plain_ask_names_the_wait_on_the_band() {
    let mut model = Fixture::mock("05", Size::new(120, 40)).expect("view 05 has a fixture");
    model.set_surface(Surface::Native);
    update(
        &mut model,
        Msg::Agent(AgentEvent::PermissionRequest {
            id: 2,
            title: "roadmap_start_chunk".to_owned(),
            choices: vec![
                PermissionChoice::new("allow", PermissionKind::AllowOnce),
                PermissionChoice::new("reject", PermissionKind::RejectOnce),
            ],
            edit: false,
            risk: RiskTier::Medium,
            plan: None,
        }),
    );
    let band = BandComposition::el(&model, Medium::Night);
    assert!(
        has(&band, "needs you: Start a roadmap item"),
        "{:?}",
        texts(&band)
    );
    assert!(
        !has(&band, "agent working"),
        "the wait replaces the stage word"
    );
}
