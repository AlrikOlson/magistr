use super::*;
use magistr_workflow::roadmap::RoadmapEngine;

fn engine_with(chunks: &[(&str, ChunkStatus, Option<&str>)]) -> RoadmapEngine {
    let mut e = RoadmapEngine::new("p".into());
    for (id, status, group) in chunks {
        e.add_chunk(
            (*id).into(),
            format!("Chunk {id}"),
            ChunkStatus::Pending,
            100,
            String::new(),
            vec![],
            vec![],
            false,
        )
        .expect("add");
        if let Some(g) = group {
            e.set_group(id, Some((*g).into())).expect("group");
        }
        if *status != ChunkStatus::Pending {
            // Done requires InProgress first; go through the legal path.
            if *status == ChunkStatus::Done {
                e.set_status(id, ChunkStatus::InProgress).expect("ip");
            }
            e.set_status(id, *status).expect("status");
        }
    }
    e
}

#[test]
fn empty_roadmap_has_no_progress() {
    let e = RoadmapEngine::new("p".into());
    assert_eq!(
        progress_from_chunks(&e.roadmap().chunks),
        RoadmapProgress::default()
    );
}

#[test]
fn counts_chunks_as_tasks_and_groups_as_phases() {
    let e = engine_with(&[
        ("a", ChunkStatus::Done, Some("Engine")),
        ("b", ChunkStatus::Done, Some("Engine")),
        ("c", ChunkStatus::Pending, Some("GUI")),
        ("d", ChunkStatus::InProgress, None),
    ]);
    let p = progress_from_chunks(&e.roadmap().chunks);
    assert_eq!(p.total_tasks, 4);
    assert_eq!(p.checked_tasks, 2);
    assert_eq!(p.total_phases, 2);
    assert_eq!(p.completed_phases, 1, "Engine is fully done, GUI is not");
    assert!(!p.is_complete());
}

#[test]
fn obsoleted_chunks_count_toward_nothing() {
    let e = engine_with(&[
        ("a", ChunkStatus::Done, Some("Engine")),
        ("z", ChunkStatus::Obsoleted, Some("Engine")),
        ("y", ChunkStatus::Obsoleted, Some("Dead")),
    ]);
    let p = progress_from_chunks(&e.roadmap().chunks);
    assert_eq!(p.total_tasks, 1);
    assert_eq!(p.checked_tasks, 1);
    assert_eq!(
        p.total_phases, 1,
        "a group of only obsoleted chunks is not a phase"
    );
    assert_eq!(p.completed_phases, 1);
    assert!(p.is_complete());
}
