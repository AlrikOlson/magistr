//! Complete selected plan records with expendable safe text presentation.
use crate::{commands::DesktopState, document::Document};
use magistr_domain::identity::SessionAddress;
use magistr_workflow_contracts::plan::{
    Task, TaskStatus, WorkstreamReadiness, workstream_frontier,
};
use magistr_workflow_contracts::{ReadState, PlanData};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct PlanItem {
    record: Task,
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
    status: Option<TaskStatus>,
    feature: Option<String>,
}
impl PlanItem {
    fn new(record: Task, records: &[Task]) -> Self {
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
                feature: item.and_then(|item| item.feature.clone()),
            }
        };
        let readiness = workstream_frontier(records, record.feature.as_deref())
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
    item: ReadState<Option<PlanItem>>,
}
fn selected(read: &ReadState<PlanData>, id: &str) -> ReadState<Option<PlanItem>> {
    read.map(|data| {
        data.tasks
            .iter()
            .find(|record| record.id == id)
            .cloned()
            .map(|record| PlanItem::new(record, &data.tasks))
    })
}
#[tauri::command]
pub(crate) async fn read_plan_item(
    address: SessionAddress,
    id: String,
    state: tauri::State<'_, DesktopState>,
) -> Result<ItemRead, String> {
    let snapshot = state.client.subscribe(&address).await?.snapshot();
    tokio::task::spawn_blocking(move || ItemRead {
        address: snapshot.address,
        revision: snapshot.workflow_revision,
        item: selected(&snapshot.seed.snapshot.plan, &id),
        id,
    })
    .await
    .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "plan_item.tests.rs"]
mod tests;
