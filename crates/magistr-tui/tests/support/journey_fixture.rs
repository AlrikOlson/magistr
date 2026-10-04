//! Full-roadmap-scale authored records for the combined terminal acceptance journey.
use magistr_cockpit::cockpit::{Model, Size, StageTab};
use magistr_cockpit::harness::{Scenes, fixtures};
use magistr_engine::{ChunkStatus, NativeSnapshot, RoadmapProgress};
use serde_json::json;

pub const RECORDS: usize = 1_000;
pub const REQUEST: &str = "Review the invitation delivery plan";
pub const DRAFT: &str = "Keep this unsent draft.";

pub fn snapshot() -> NativeSnapshot {
    let long = (0..80)
        .map(|n| {
            format!("Reading checkpoint {n:03}. Each customer keeps its own saved receipt.\n\n")
        })
        .collect::<String>();
    let mut chunks = fixtures::chunks_from(vec![
        json!({"id":"waiting","name":"Publish invitations","title":"Publish invitations","status":"pending","priority":10,
            "group":"Delivery","deps":["storage"],"description":format!("{}\n{}", Scenes::RICH_BODY, long),
            "acceptance":["Keep the original receipt and command.","Preserve reading position and unsent requests."],
            "cross_refs":["think:917"]}),
        json!({"id":"ready","name":"Review invitations","title":"Review invitations","status":"pending","priority":20,"group":"Delivery"}),
        json!({"id":"blocked","name":"Connect delivery","title":"Connect delivery","status":"pending","priority":30,"group":"Delivery",
            "blocked_by":{"kind":"external","reason":"The delivery account needs an access key."}}),
        json!({"id":"history","name":"Previous delivery","title":"Previous delivery","status":"done","priority":40,"group":"Delivery"}),
        json!({"id":"storage","name":"Receipt storage","title":"Receipt storage","status":"in_progress","priority":50,"group":"Storage",
            "description":"Keep each saved receipt independent."}),
    ]);
    chunks.extend((5..RECORDS).map(|n| fixtures::chunk_from(json!({
        "id":format!("archive-{n:04}"),"name":format!("Saved work {n:04}"),"title":format!("Saved work {n:04}"),
        "group":format!("Track {:02}",n%80),"priority":100+n,
        "status":if n%3==0 {"done"} else {"pending"},
        "deps":if n%7==0 {vec!["storage"]} else {vec![]},
    }))));
    NativeSnapshot {
        progress: RoadmapProgress {
            total_tasks: u32::try_from(chunks.len()).expect("bounded fixture"),
            checked_tasks: u32::try_from(
                chunks
                    .iter()
                    .filter(|chunk| chunk.status == ChunkStatus::Done)
                    .count(),
            )
            .expect("bounded fixture"),
            ..Default::default()
        },
        chunks,
        steps: fixtures::steps_from(vec![
            json!({"step_number":917,"purpose":"Review delivery findings",
            "thought":format!("{}\n{}",Scenes::RICH_BODY,long),
            "outcome":"The recovery check passed; restart still needs review.","execution_ref":"chunk:waiting"}),
        ]),
        ..Default::default()
    }
}

pub fn model(size: Size) -> Model {
    Model::new(size)
        .with_title("Delivery planning")
        .with_tab(StageTab::Roadmap)
        .with_snapshot(&snapshot())
}
