//! Complete selected roadmap records with expendable safe text presentation.
use crate::{commands::DesktopState, document::Document};
use magistr_domain::identity::SessionAddress;
use magistr_workflow_contracts::roadmap::{
    Chunk, ChunkStatus, WorkstreamReadiness, workstream_frontier,
};
use magistr_workflow_contracts::{ReadState, RoadmapData};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct RoadmapItem {
    record: Chunk,
    description: Document,
    notes: Document,
    requirements: Vec<Document>,
    sections: Vec<Document>,
    readiness: WorkstreamReadiness,
    dependencies: Vec<Relation>,
    dependents: Vec<Relation>,
}
#[derive(Serialize)]
struct Relation {
    id: String,
    title: Option<String>,
    status: Option<ChunkStatus>,
    group: Option<String>,
}
impl RoadmapItem {
    fn new(record: Chunk, records: &[Chunk]) -> Self {
        let by_id: std::collections::BTreeMap<_, _> = records
            .iter()
            .map(|item| (item.id.as_str(), item))
            .collect();
        let resolve = |id: &str| {
            let item = by_id.get(id);
            Relation {
                id: id.into(),
                title: item.map(|item| item.title.clone()),
                status: item.map(|item| item.status),
                group: item.and_then(|item| item.group.clone()),
            }
        };
        let readiness = workstream_frontier(records, record.group.as_deref())
            .items
            .into_iter()
            .find(|item| item.id == record.id)
            .map_or(WorkstreamReadiness::Inactive, |item| item.readiness);
        Self {
            readiness,
            dependencies: record.deps.iter().map(|id| resolve(id)).collect(),
            dependents: records
                .iter()
                .filter(|item| item.deps.contains(&record.id))
                .map(|item| resolve(&item.id))
                .collect(),
            description: Document::parse(&record.description),
            notes: Document::parse(&record.notes),
            requirements: record
                .acceptance
                .iter()
                .map(|text| Document::parse(text))
                .collect(),
            sections: record
                .content
                .as_ref()
                .and_then(|content| content.sections.as_ref())
                .into_iter()
                .flatten()
                .map(|section| Document::parse(section.prose.as_deref().unwrap_or_default()))
                .collect(),
            record,
        }
    }
}
#[derive(Serialize)]
pub(crate) struct ItemRead {
    address: SessionAddress,
    revision: u64,
    id: String,
    item: ReadState<Option<RoadmapItem>>,
}
fn selected(read: &ReadState<RoadmapData>, id: &str) -> ReadState<Option<RoadmapItem>> {
    read.map(|data| {
        data.chunks
            .iter()
            .find(|record| record.id == id)
            .cloned()
            .map(|record| RoadmapItem::new(record, &data.chunks))
    })
}
#[tauri::command]
pub(crate) async fn read_roadmap_item(
    address: SessionAddress,
    id: String,
    state: tauri::State<'_, DesktopState>,
) -> Result<ItemRead, String> {
    let snapshot = state.client.subscribe(&address).await?.snapshot();
    tokio::task::spawn_blocking(move || ItemRead {
        address: snapshot.address,
        revision: snapshot.workflow_revision,
        item: selected(&snapshot.seed.snapshot.roadmap, &id),
        id,
    })
    .await
    .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "roadmap_item.tests.rs"]
mod tests;
