use super::*;
use crate::roadmap::domain::ChunkStatus;
use crate::tracker::ownership::{Divergence, Field, Owner};

fn roadmap_with_link() -> RoadmapEngine {
    let mut e = RoadmapEngine::new("proj".into());
    e.add_chunk(
        "c1".into(),
        "Chunk c1".into(),
        ChunkStatus::Pending,
        10,
        "why".into(),
        vec!["works".into()],
        vec![],
        false,
    )
    .expect("add");
    e.record_tracker_link("c1", "linear", "THI-1", "hash", Some("v1"))
        .expect("link");
    e
}

fn report_with(owner: Owner) -> ProjectionReport {
    ProjectionReport {
        divergences: vec![(
            "c1".to_string(),
            Divergence {
                field: Field::Title,
                owner,
                ours: "our title".into(),
                theirs: "their title".into(),
            },
        )],
        ..ProjectionReport::default()
    }
}

#[test]
fn a_contested_divergence_becomes_a_concern_linked_both_ways() {
    let mut signal = SignalEngine::new("proj".into());
    let roadmap = roadmap_with_link();

    let ids = emit_divergence_concerns(
        &mut signal,
        &roadmap,
        "linear",
        &report_with(Owner::Contested),
    )
    .unwrap();

    assert_eq!(ids.len(), 1);
    let s = signal.get(&ids[0]).expect("signal");
    assert_eq!(s.kind, SignalKind::Concern);
    assert!(
        s.cross_refs.contains(&"chunk:c1".to_string()),
        "findable from the chunk: {:?}",
        s.cross_refs
    );
    assert!(
        s.cross_refs.contains(&"ext:linear/THI-1".to_string()),
        "findable from the ticket: {:?}",
        s.cross_refs
    );
    assert!(
        s.body.contains("neither side owns it"),
        "the signal body is the divergence's own sentence, not a second wording"
    );
}

/// A projection runs on every push. A duplicating emitter would turn one
/// disagreement into a stream of identical concerns, and the inbox would be
/// abandoned within a day.
#[test]
fn re_raising_an_unchanged_conflict_mints_nothing() {
    let mut signal = SignalEngine::new("proj".into());
    let roadmap = roadmap_with_link();
    let report = report_with(Owner::Contested);

    let first = emit_divergence_concerns(&mut signal, &roadmap, "linear", &report).unwrap();
    let second = emit_divergence_concerns(&mut signal, &roadmap, "linear", &report).unwrap();

    assert_eq!(first.len(), 1);
    assert!(second.is_empty(), "the same conflict was raised twice");
    assert_eq!(signal.signals().signals.len(), 1);
}

/// A conflict whose VALUES moved is a different disagreement and deserves
/// its own concern — dedup must not swallow a genuine change.
#[test]
fn a_changed_conflict_raises_again() {
    let mut signal = SignalEngine::new("proj".into());
    let roadmap = roadmap_with_link();

    emit_divergence_concerns(
        &mut signal,
        &roadmap,
        "linear",
        &report_with(Owner::Contested),
    )
    .unwrap();

    let mut moved = report_with(Owner::Contested);
    moved.divergences[0].1.theirs = "a different retitle".into();
    let again = emit_divergence_concerns(&mut signal, &roadmap, "linear", &moved).unwrap();

    assert_eq!(again.len(), 1, "a new disagreement must reach someone");
    assert_eq!(signal.signals().signals.len(), 2);
}

/// A field the tracker owns diverging is normal. Raising it is the noise
/// that gets the whole channel ignored.
#[test]
fn a_field_they_own_raises_nothing() {
    let mut signal = SignalEngine::new("proj".into());
    let roadmap = roadmap_with_link();

    let ids =
        emit_divergence_concerns(&mut signal, &roadmap, "linear", &report_with(Owner::Theirs))
            .unwrap();

    assert!(ids.is_empty());
    assert!(signal.signals().signals.is_empty());
}

/// Without a link there is no ticket to name, and a concern that cannot say
/// which ticket it is about is not worth raising.
#[test]
fn a_chunk_with_no_link_raises_nothing() {
    let mut signal = SignalEngine::new("proj".into());
    let roadmap = RoadmapEngine::new("proj".into());

    let ids = emit_divergence_concerns(
        &mut signal,
        &roadmap,
        "linear",
        &report_with(Owner::Contested),
    )
    .unwrap();
    assert!(ids.is_empty());
}

// ── propose_titles_from_sweep (tracker-sweep-title-proposal) ──────────

use crate::tracker::domain::{WorkItem, WorkItemState};
use crate::tracker::ownership::Ownership;
use crate::tracker::sweep::SweepReport;

fn item_titled(title: &str) -> WorkItem {
    WorkItem {
        external_id: Some("THI-1".to_string()),
        title: title.into(),
        body: String::new(),
        state: WorkItemState::Todo,
        labels: vec![],
        assignee: None,
        version: Some("v2".into()),
        group: None,
    }
}

fn sweep_finding(item: WorkItem) -> SweepReport {
    SweepReport {
        fetched: 1,
        remote: vec![item],
        ..SweepReport::default()
    }
}

#[test]
fn a_remote_retitle_becomes_a_title_proposal_and_the_title_does_not_move() {
    let mut roadmap = roadmap_with_link();

    let proposed = propose_titles_from_sweep(
        &mut roadmap,
        "linear",
        &sweep_finding(item_titled("A human's better name")),
        &Ownership::default(),
    )
    .unwrap();

    assert_eq!(proposed, vec!["c1".to_string()]);
    let chunk = roadmap
        .roadmap()
        .chunks
        .iter()
        .find(|c| c.id == "c1")
        .expect("chunk");
    let p = chunk
        .title_proposal
        .as_ref()
        .expect("the proposal must be RECORDED, not merely returned");
    assert_eq!(p.suggested_title, "A human's better name");
    assert_eq!(
        p.source, "ext:linear/THI-1",
        "the proposal must say which ticket caused it"
    );
    assert_eq!(
        chunk.title, "Chunk c1",
        "a proposal is never a rename — the plan's title must not move"
    );
}

/// The ownership gate. A project that sets Title to Ours has said the plan
/// wins: re-asserting it is push's business, and proposing it here would
/// launder a remote overwrite into a polite suggestion. Theirs means the
/// team decided title divergence is not a conversation at all.
#[test]
fn a_title_the_plan_owns_gets_no_proposal_from_the_sweep() {
    for owner in [Owner::Ours, Owner::Theirs] {
        let mut roadmap = roadmap_with_link();
        let proposed = propose_titles_from_sweep(
            &mut roadmap,
            "linear",
            &sweep_finding(item_titled("A human's better name")),
            &Ownership::default().with(Field::Title, owner),
        )
        .unwrap();
        assert!(proposed.is_empty(), "{owner:?} must not propose");
        assert!(
            roadmap.roadmap().chunks[0].title_proposal.is_none(),
            "{owner:?}: a proposal landed on the chunk anyway"
        );
    }
}

/// The echo fence, inherited: only `report.remote` may mint. A drifted item
/// is the trap this guards — it is OUR OWN write coming back with its
/// content mangled by a lossy adapter round trip, so its title genuinely
/// differs from the plan's, and treating it as a human's retitle would turn
/// adapter lossage into a stream of phantom proposals.
#[test]
fn an_echo_classified_item_cannot_mint_a_title_proposal() {
    let mut roadmap = roadmap_with_link();
    let report = SweepReport {
        fetched: 2,
        echoes: 2,
        drifted: vec![item_titled("Chunk c1 (mangled by the round trip)")],
        ..SweepReport::default()
    };

    let proposed =
        propose_titles_from_sweep(&mut roadmap, "linear", &report, &Ownership::default()).unwrap();

    assert!(proposed.is_empty());
    assert!(
        roadmap.roadmap().chunks[0].title_proposal.is_none(),
        "an echo minted a title proposal — the sweep is proposing our own \
         write back at us"
    );
}

#[test]
fn an_unchanged_remote_title_proposes_nothing() {
    let mut roadmap = roadmap_with_link();

    let proposed = propose_titles_from_sweep(
        &mut roadmap,
        "linear",
        &sweep_finding(item_titled("Chunk c1")),
        &Ownership::default(),
    )
    .unwrap();

    assert!(proposed.is_empty());
    assert!(roadmap.roadmap().chunks[0].title_proposal.is_none());
}

/// A sweep runs every few minutes forever, so the criterion is not "it
/// proposes" but "proposing twice changes nothing": `proposed_at` must not
/// advance for an unchanged suggestion, or every old disagreement looks
/// perpetually new.
#[test]
fn re_proposing_the_same_retitle_does_not_restamp() {
    let mut roadmap = roadmap_with_link();
    let report = sweep_finding(item_titled("A human's better name"));

    propose_titles_from_sweep(&mut roadmap, "linear", &report, &Ownership::default()).unwrap();
    let first = roadmap.roadmap().chunks[0]
        .title_proposal
        .as_ref()
        .expect("recorded")
        .proposed_at
        .clone();

    propose_titles_from_sweep(&mut roadmap, "linear", &report, &Ownership::default()).unwrap();
    let second = roadmap.roadmap().chunks[0]
        .title_proposal
        .as_ref()
        .expect("still recorded")
        .proposed_at
        .clone();

    assert_eq!(first, second, "an unchanged suggestion was restamped");
}
