//! On-demand complete records from the existing session observation owner.
use crate::commands::DesktopState;
use magistr_domain::identity::SessionAddress;
use magistr_workflow_contracts::NativeSnapshot;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct WorkflowView {
    address: SessionAddress,
    revision: u64,
    snapshot: NativeSnapshot,
    roadmap: magistr_workflow_contracts::ReadState<crate::roadmap::RoadmapOverview>,
}

#[tauri::command]
pub(crate) async fn read_workflow(
    address: SessionAddress,
    state: tauri::State<'_, DesktopState>,
) -> Result<WorkflowView, String> {
    let snapshot = state.client.subscribe(&address).await?.snapshot();
    let roadmap = crate::roadmap::overview(&snapshot.seed.snapshot.roadmap);
    Ok(WorkflowView {
        roadmap,
        address: snapshot.address,
        revision: snapshot.workflow_revision,
        snapshot: snapshot.seed.snapshot,
    })
}
