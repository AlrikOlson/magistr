use magistr_engine::NativeFrame;
use serde_json::json;

use super::*;
use crate::cockpit::{
    Budget, LauncherData, LauncherSession, Msg, Size, StageTab, Surface, WorkbenchEvent, update,
};
use crate::engine::{EngineEvent, StoreSeed};
use crate::workbench::JournalEntry;

fn chunk(id: &str, status: &str) -> serde_json::Value {
    json!({ "id": id, "name": id, "status": status, "priority": 200, "deps": [] })
}

/// A window on the Sky with `chunks` seeded — the surface every Brief
/// reading in this file is taken on, since the overlay is native only.
fn sky(chunks: Vec<serde_json::Value>) -> Model {
    let mut model = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_tab(StageTab::Roadmap);
    update(
        &mut model,
        Msg::Engine(EngineEvent::Seed(StoreSeed {
            ship_status: serde_json::Value::Null,
            chunk: Some("gui2-2a".to_owned()),
            done: 211,
            total: 306,
            signals_pending: 0,
            chunks,
            signals: Vec::new(),
            trace: Vec::new(),
        })),
    );
    model
}

fn completed(model: &mut Model, id: &str) {
    update(
        model,
        Msg::Engine(EngineEvent::Frame(NativeFrame {
            family: "roadmap".to_owned(),
            kind: "chunk_completed".to_owned(),
            payload: json!({ "chunk": chunk(id, "done") }),
        })),
    );
}

fn journal(model: &mut Model, ids: &[&str]) {
    update(
        model,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: ids
                .iter()
                .map(|id| JournalEntry {
                    id: (*id).to_owned(),
                    path: format!("src/{id}.rs"),
                    added: 3,
                    removed: 1,
                    parse_ok: true,
                    diff: String::new(),
                })
                .collect(),
        }),
    );
}

fn away() -> Duration {
    Duration::from_secs(6 * 3600 + 12 * 60)
}

/// A quiet project raises nothing: the Brief exists to report news, and
/// an overlay that says "no news" is one Esc a person should never have
/// had to press.
#[test]
fn a_quiet_project_raises_no_brief() {
    let model = sky(vec![chunk("gui2-2a", "in_progress")]);
    assert!(Brief::of(&model, away()).is_none());
}

/// The spend is carried for the record and never on its own — it is the
/// one card that cannot make a Brief happen.
#[test]
fn the_spend_alone_is_not_news() {
    let mut model = sky(vec![chunk("gui2-2a", "in_progress")]);
    model
        .rail_mut()
        .set_budget(Budget::capped(1200).map(|b| b.spent(361)));
    assert!(Brief::of(&model, away()).is_none());
}

/// Worst first: the alert card, then the attention card, then the good
/// one, then the quiet one — `Severity::ALL`'s own order.
#[test]
fn the_cards_are_dealt_worst_first() {
    let mut model = sky(vec![chunk("e6b", "stuck"), chunk("gui2-2a", "in_progress")]);
    journal(&mut model, &["e1"]);
    completed(&mut model, "e10-fleet");
    model
        .rail_mut()
        .set_budget(Budget::capped(1200).map(|b| b.spent(361)));
    let brief = Brief::of(&model, away()).expect("there is news");
    let bands: Vec<Severity> = brief.cards().iter().map(BriefCard::severity).collect();
    assert_eq!(bands, Severity::ALL.to_vec());
}

/// The headline combines the elapsed gap and item count.
#[test]
fn the_headline_is_the_gap_and_the_count() {
    let mut model = sky(vec![chunk("e6b", "stuck"), chunk("gui2-2a", "in_progress")]);
    journal(&mut model, &["e1"]);
    completed(&mut model, "e10-fleet");
    model
        .rail_mut()
        .set_budget(Budget::capped(1200).map(|b| b.spent(361)));
    let brief = Brief::of(&model, away()).expect("there is news");
    assert_eq!(brief.away(), "6 h 12 m");
    assert_eq!(brief.headline(), "6 h 12 m · 4 items");
}

/// The alert card is dealt from the same predicate the constellation
/// draws its red ring with, so the overlay and the sky underneath it
/// cannot disagree about what is stopped.
#[test]
fn the_alert_card_is_the_stars_that_alert() {
    let model = sky(vec![
        chunk("e6b", "stuck"),
        chunk("later", "blocked"),
        chunk("gui2-2a", "in_progress"),
    ]);
    let brief = Brief::of(&model, away()).expect("there is news");
    let card = &brief.cards()[0];
    assert_eq!(card.severity(), Severity::Alert);
    assert_eq!(card.cap(), "Stuck · 2");
    assert_eq!(
        card.body().verb(),
        Some(("Answer", crate::cockpit::Widget::BriefAnswer))
    );
}

/// The good card counts the completions this cockpit *watched*, not the
/// done chunks a seed handed it: a store full of shipped work is
/// history, and only a completion the cockpit was there for is news.
#[test]
fn the_good_card_counts_watched_completions_not_seeded_ones() {
    let mut model = sky(vec![
        chunk("old-a", "done"),
        chunk("old-b", "done"),
        chunk("gui2-2a", "in_progress"),
    ]);
    assert!(Brief::of(&model, away()).is_none(), "a seed is not news");
    completed(&mut model, "e10-fleet");
    completed(&mut model, "tui2-sheet-overflow");
    let brief = Brief::of(&model, away()).expect("two completions are news");
    let card = &brief.cards()[0];
    assert_eq!(card.severity(), Severity::Good);
    assert_eq!(card.cap(), "Done · 2");
    let BriefBody::Act { text, .. } = card.body() else {
        panic!("the good card acts");
    };
    assert_eq!(text, "e10-fleet, tui2-sheet-overflow. Merged.");
}

/// A completion re-broadcast does not double-count: the log is a set of
/// what shipped, not a tally of the frames that said so.
#[test]
fn a_repeated_completion_frame_is_counted_once() {
    let mut model = sky(vec![chunk("gui2-2a", "in_progress")]);
    completed(&mut model, "e10-fleet");
    completed(&mut model, "e10-fleet");
    assert_eq!(model.roadmap().shipped(), ["e10-fleet"]);
}

/// The spend card is a meter and has no verb — a second primary action is
/// exactly what a button here would be.
#[test]
fn the_spend_card_is_a_meter_and_carries_no_verb() {
    let mut model = sky(vec![chunk("e6b", "stuck"), chunk("gui2-2a", "in_progress")]);
    model
        .rail_mut()
        .set_budget(Budget::capped(1200).map(|b| b.spent(361).stop_at(1000)));
    model = model.with_launcher_sessions(3);
    let brief = Brief::of(&model, away()).expect("there is news");
    let card = brief.cards().last().expect("the quiet card is last");
    assert_eq!(card.severity(), Severity::Quiet);
    assert_eq!(card.cap(), "Spend · 3 sessions");
    assert_eq!(card.body().verb(), None);
    let BriefBody::Measure {
        readout,
        fill,
        tick,
    } = card.body()
    else {
        panic!("the spend card measures");
    };
    assert_eq!(readout, "3.61 / 12.00 USD");
    assert_eq!(*fill, 300);
    assert_eq!(*tick, Some(833));
}

/// A gap under an hour is minutes; a gap over a day is days and hours.
/// Two units, never three — the headline is a glance.
#[test]
fn the_gap_is_the_coarsest_two_units_that_carry_it() {
    assert_eq!(elapsed(Duration::from_secs(12 * 60)), "12 m");
    assert_eq!(elapsed(Duration::from_secs(6 * 3600 + 12 * 60)), "6 h 12 m");
    assert_eq!(
        elapsed(Duration::from_secs(2 * 86400 + 3 * 3600)),
        "2 d 3 h"
    );
}

/// A window with a change waiting says what is waiting, how much of it,
/// and — only when a run left a verdict — how the gates went.
#[test]
fn the_attention_card_only_claims_a_gate_verdict_it_has() {
    let mut model = sky(vec![chunk("gui2-2a", "in_progress")]);
    journal(&mut model, &["e1", "e2", "e3"]);
    let brief = Brief::of(&model, away()).expect("a change is news");
    let BriefBody::Act { text, .. } = brief.cards()[0].body() else {
        panic!("the attention card acts");
    };
    assert_eq!(text, "gui2-2a is ready for review. 3 files.");
}

/// Helper: a launcher with `n` session rows, which is what the spend
/// card counts.
impl Model {
    fn with_launcher_sessions(mut self, n: usize) -> Self {
        self.launcher = Some(LauncherData {
            sessions: (0..n).map(|_| LauncherSession::default()).collect(),
            ..LauncherData::default()
        });
        self
    }
}
