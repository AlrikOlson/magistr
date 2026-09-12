use magistr_engine::NativeEvent;
use serde_json::json;

use super::*;
use crate::harness::fixtures;

fn accept_priority(id: &str) -> EngineCmd {
    EngineCmd::ResolveProposal {
        chunk: id.to_owned(),
        kind: ProposalKind::Reprioritize,
        accept: true,
    }
}

#[test]
fn refused_answers_keep_arrival_order_even_when_refusals_arrive_in_reverse() {
    for reverse in [false, true] {
        let mut dock = Dock::default();
        for id in ["first", "second", "queued"] {
            dock.apply(&reprioritize(id));
        }
        let before: Vec<_> = dock.cards().cloned().collect();
        let first = accept_priority("first");
        let second = accept_priority("second");
        dock.answer_front(&first);
        dock.answer_front(&second);
        assert_eq!(dock.cards().cloned().collect::<Vec<_>>(), before[2..]);
        let order = if reverse {
            [&second, &first]
        } else {
            [&first, &second]
        };
        for cmd in order {
            dock.restore_answer(cmd);
        }
        assert_eq!(dock.cards().cloned().collect::<Vec<_>>(), before);
        dock.restore_answer(&first);
        assert_eq!(dock.len(), 3, "a duplicate refusal never duplicates a card");
    }
}

#[test]
fn only_the_matching_answer_restores_a_pending_card() {
    let mut dock = Dock::default();
    dock.apply(&reprioritize("first"));
    let cmd = accept_priority("first");
    dock.answer_front(&cmd);
    assert!(dock.is_empty());
    assert_eq!(dock.rows(), 1);
    assert!(dock.front_mut().is_none());
    assert!(dock.pop_front().is_none());
    for unrelated in [
        EngineCmd::Checkpoint,
        EngineCmd::ClearBlocker {
            chunk: "first".to_owned(),
        },
        EngineCmd::ResolveProposal {
            chunk: "first".to_owned(),
            kind: ProposalKind::Title,
            accept: true,
        },
        EngineCmd::ResolveProposal {
            chunk: "first".to_owned(),
            kind: ProposalKind::Reprioritize,
            accept: false,
        },
        accept_priority("other"),
    ] {
        dock.restore_answer(&unrelated);
        assert!(dock.is_empty(), "{unrelated:?}");
    }
    dock.restore_answer(&cmd);
    assert_eq!(dock.len(), 1);
    assert_eq!(dock.rows(), 3);
}

#[test]
fn a_permission_arriving_during_an_answer_stays_ahead_of_the_restored_card() {
    let mut dock = Dock::default();
    dock.apply(&reprioritize("first"));
    dock.apply(&reprioritize("queued"));
    let cmd = accept_priority("first");
    dock.answer_front(&cmd);
    dock.push_permission(Permission::new(1, "Bash", Vec::new()));
    dock.push_permission(Permission::new(2, "Edit", Vec::new()));
    dock.restore_answer(&cmd);
    assert_eq!(dock.permissions_pending(), 2);
    assert_eq!(dock.pop_permission().expect("first permission").id(), 1);
    assert_eq!(dock.pop_permission().expect("second permission").id(), 2);
    assert!(matches!(dock.front(), Some(DockItem::Proposal { chunk, .. }) if chunk == "first"));
    assert_eq!(dock.len(), 2);
}

#[test]
fn visible_mutation_and_pop_never_touch_a_hidden_answer() {
    let mut dock = Dock::default();
    dock.apply(&reprioritize("first"));
    dock.apply(&reprioritize("queued"));
    let cmd = accept_priority("first");
    dock.answer_front(&cmd);
    let Some(DockItem::Proposal { reason, .. }) = dock.front_mut() else {
        panic!("the queued card is visible");
    };
    *reason = "edited".to_owned();
    assert!(
        matches!(dock.pop_front(), Some(DockItem::Proposal { chunk, reason, .. })
        if chunk == "queued" && reason == "edited")
    );
    dock.restore_answer(&cmd);
    assert!(
        matches!(dock.front(), Some(DockItem::Proposal { chunk, reason, .. })
        if chunk == "first" && reason == "later")
    );
}

#[test]
fn authoritative_frames_reconcile_pending_cards_without_stale_restoration() {
    for kind in ["chunk_changed", "chunk_completed", "chunk_obsoleted"] {
        let mut dock = Dock::default();
        dock.apply(&reprioritize("first"));
        dock.apply(&reprioritize("queued"));
        let cmd = accept_priority("first");
        dock.answer_front(&cmd);
        dock.apply(&frame(kind, json!({ "id": "first" })));
        dock.restore_answer(&cmd);
        assert_eq!(dock.len(), 1, "{kind}");
        assert!(
            matches!(dock.front(), Some(DockItem::Proposal { chunk, .. }) if chunk == "queued")
        );
        assert!(
            dock.pending.is_empty(),
            "the authoritative answer releases pending state"
        );
    }
    let mut dock = Dock::default();
    dock.apply(&reprioritize("first"));
    dock.apply(&reprioritize("queued"));
    dock.answer_front(&accept_priority("first"));
    dock.apply(&frame(
        "chunk_changed",
        json!({
            "id": "first",
            "reprioritize": { "suggested_priority": 400, "reason": "reconsidered" }
        }),
    ));
    assert!(
        matches!(dock.front(), Some(DockItem::Proposal { chunk, suggestion, reason, .. })
        if chunk == "first" && suggestion == "400" && reason == "reconsidered")
    );
    assert_eq!(dock.len(), 2, "an upsert stays in the original place");
}

fn frame(kind: &str, chunk: serde_json::Value) -> NativeEvent {
    fixtures::frame("roadmap", kind, json!({ "chunk": chunk }))
}

fn reprioritize(id: &str) -> NativeEvent {
    frame(
        "chunk_changed",
        json!({
            "id": id,
            "status": "pending",
            "reprioritize": { "suggested_priority": 300, "reason": "later" },
        }),
    )
}

#[test]
fn an_empty_dock_still_takes_one_row() {
    let dock = Dock::default();
    assert!(dock.is_empty());
    assert_eq!(dock.rows(), 1);
}

#[test]
fn a_proposal_on_a_chunk_changed_frame_becomes_a_card_once() {
    let mut dock = Dock::default();
    dock.apply(&reprioritize("base"));
    dock.apply(&reprioritize("base"));
    assert_eq!(dock.len(), 1, "a repeat replaces, never duplicates");
    assert_eq!(
        dock.front()
            .map(|card| card.text(&crate::theme::Icons::UNICODE)),
        Some("◆ base: priority → 300 (later)".to_owned())
    );
}

#[test]
fn each_proposal_kind_gets_its_own_card_and_clears_when_the_payload_drops_it() {
    let mut dock = Dock::default();
    dock.apply(&frame(
        "chunk_changed",
        json!({
            "id": "base",
            "reprioritize": { "suggested_priority": 300, "reason": "later" },
            "status_proposal": { "suggested_status": "done", "reason": "tracker closed it" },
            "title_proposal": { "suggested_title": "Auth", "reason": "renamed upstream" },
        }),
    ));
    assert_eq!(dock.len(), 3);
    // The next frame carries only the title proposal: the others clear.
    dock.apply(&frame(
        "chunk_changed",
        json!({
            "id": "base",
            "title_proposal": { "suggested_title": "Auth", "reason": "renamed upstream" },
        }),
    ));
    assert_eq!(dock.len(), 1);
    assert_eq!(
        dock.front()
            .map(|card| card.text(&crate::theme::Icons::UNICODE)),
        Some("◆ base: title → Auth (renamed upstream)".to_owned())
    );
}

#[test]
fn only_an_awaiting_human_blocker_becomes_a_card() {
    let mut dock = Dock::default();
    dock.apply(&frame(
        "chunk_changed",
        json!({ "id": "ext", "blocked_by": { "kind": "external", "reason": "upstream" } }),
    ));
    assert!(dock.is_empty(), "an external blocker is not the human's");
    dock.apply(&frame(
        "chunk_changed",
        json!({ "id": "base", "blocked_by": { "kind": "awaiting_human", "reason": "needs a decision" } }),
    ));
    assert_eq!(
        dock.front()
            .map(|card| card.text(&crate::theme::Icons::UNICODE)),
        Some("⊗ base blocked: needs a decision".to_owned())
    );
    // The blocker cleared: the card goes.
    dock.apply(&frame("chunk_changed", json!({ "id": "base" })));
    assert!(dock.is_empty());
}

#[test]
fn a_completed_chunk_takes_its_cards_away() {
    let mut dock = Dock::default();
    dock.apply(&reprioritize("base"));
    dock.apply(&reprioritize("other"));
    dock.apply(&frame("chunk_completed", json!({ "id": "base" })));
    assert_eq!(dock.len(), 1);
    assert_eq!(
        dock.front()
            .map(|card| card.text(&crate::theme::Icons::UNICODE)),
        Some("◆ other: priority → 300 (later)".to_owned())
    );
}

#[test]
fn permissions_queue_ahead_of_proposals_and_count_separately() {
    let mut dock = Dock::default();
    dock.apply(&reprioritize("base"));
    dock.push_permission(Permission::new(1, "Bash", Vec::new()));
    dock.push_permission(Permission::new(2, "Edit", Vec::new()));
    assert_eq!(dock.len(), 3);
    assert_eq!(dock.permissions_pending(), 2);
    let order: Vec<String> = dock
        .cards()
        .map(|card| card.text(&crate::theme::Icons::UNICODE))
        .collect();
    assert_eq!(
        order,
        [
            "⚠ Bash needs permission",
            "⚠ Edit needs permission",
            "◆ base: priority → 300 (later)",
        ],
        "requests the agent is blocked on come first, in arrival order"
    );
    assert!(matches!(
        dock.pop_front(),
        Some(DockItem::Permission(p)) if p.id() == 1
    ));
    assert_eq!(dock.permissions_pending(), 1);
}

#[test]
fn events_from_other_families_and_kinds_are_ignored() {
    let mut dock = Dock::default();
    dock.apply(&fixtures::frame(
        "ship",
        "objective_set",
        json!({"objective": {}}),
    ));
    dock.apply(&frame("chunk_added", json!({ "id": "base" })));
    assert!(dock.is_empty());
}

#[test]
fn an_edit_card_is_taken_by_the_path_it_names() {
    let mut dock = Dock::default();
    dock.push_permission(Permission::new(1, "Bash cargo test", Vec::new()));
    dock.push_permission(Permission::new(2, "Edit src/main.rs", Vec::new()).edit());
    let taken = dock
        .take_edit("src/main.rs")
        .expect("the diff-approval card names that file");
    assert_eq!(taken.id(), 2);
    assert_eq!(dock.len(), 1, "the plain tool permission stays");
    assert!(dock.take_edit("src/main.rs").is_none(), "it is gone");
}

#[test]
fn a_plain_tool_permission_is_never_taken_as_a_diff_approval() {
    let mut dock = Dock::default();
    // Not marked as an edit, however much its title looks like one.
    dock.push_permission(Permission::new(1, "Edit src/main.rs", Vec::new()));
    assert!(dock.take_edit("src/main.rs").is_none());
    assert_eq!(dock.len(), 1);
}

#[test]
fn an_edit_card_for_another_file_stays_put() {
    let mut dock = Dock::default();
    dock.push_permission(Permission::new(1, "Edit src/other.rs", Vec::new()).edit());
    assert!(dock.take_edit("src/main.rs").is_none());
    assert_eq!(dock.len(), 1);
}
