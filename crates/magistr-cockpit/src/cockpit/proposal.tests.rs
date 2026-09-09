use super::*;

fn proposal() -> Proposal {
    Proposal {
        chunks: vec![
            ProposedChunk::new("harmonic-engine", "core", 100),
            ProposedChunk::new("station-data-import", "core", 110)
                .standing_on(&["harmonic-engine"]),
            ProposedChunk::new("forecast-endpoint", "api", 120)
                .standing_on(&["station-data-import"]),
            ProposedChunk::new("single-binary-install", "ops", 130),
        ],
        sources: vec!["Code scan \u{b7} interview answers".to_owned()],
    }
}

/// The workstreams are distinct in first-seen order and each wears its
/// index as a hue; the summary counts both.
#[test]
fn workstreams_are_distinct_and_hued_by_first_appearance() {
    let p = proposal();
    assert_eq!(p.workstreams(), vec!["core", "api", "ops"]);
    assert_eq!(p.hue_of("ops"), 2);
    assert_eq!(p.hue_of("nowhere"), 0);
    assert_eq!(p.summary(), "4 chunks \u{b7} 3 workstreams");
    assert_eq!(
        p.first_ready().map(|c| c.id.as_str()),
        Some("harmonic-engine")
    );
}

/// A reorder moves the row, pins it and renumbers the priorities by
/// position; a rename pins too; the pinned set is the redraw's
/// constraint and the wire shape follows the list.
#[test]
fn reorder_and_rename_pin_and_the_wire_follows_the_list() {
    let mut p = proposal();
    assert!(!p.reorder(0, 0));
    assert!(!p.reorder(0, 9));
    assert!(p.reorder(3, 0));
    assert_eq!(p.chunks[0].id, "single-binary-install");
    assert!(p.chunks[0].pinned);
    let priorities: Vec<u32> = p.chunks.iter().map(|c| c.priority).collect();
    assert_eq!(priorities, [100, 110, 120, 130]);
    assert!(!p.rename(1, "   "));
    assert!(p.rename(1, "tide engine"));
    assert_eq!(p.chunks[1].name, "tide engine");
    assert_eq!(p.pinned().len(), 2);
    let wire = p.wire();
    assert_eq!(wire[0]["id"], "single-binary-install");
    assert_eq!(wire[1]["name"], "tide engine");
    assert_eq!(p.workstreams(), vec!["ops", "core", "api"]);
}
