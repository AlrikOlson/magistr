//! Complete selected roadmap records with expendable safe text presentation.
use crate::{commands::DesktopState, document::Document};
use magistr_domain::identity::SessionAddress;
use magistr_workflow_contracts::{ReadState, RoadmapData, roadmap::Chunk};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct RoadmapItem {
    record: Chunk,
    description: Document,
    notes: Document,
    requirements: Vec<Document>,
    sections: Vec<Document>,
}
impl RoadmapItem {
    fn new(record: Chunk) -> Self {
        Self {
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
            .map(RoadmapItem::new)
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
