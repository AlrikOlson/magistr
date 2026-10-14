//! Read-only agent choices and saved summaries translate the existing runtime and history owners.
use crate::commands::DesktopState;
use magistr::{agent::AgentRuntime, session::session_record::SessionRecord};
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub(crate) struct AgentChoice {
    id: String,
    label: String,
}
#[derive(Serialize)]
pub(crate) struct ModelChoice {
    id: String,
    name: String,
}
#[tauri::command]
pub(crate) async fn model_choices(
    directory: std::path::PathBuf,
    provider: String,
    state: State<'_, DesktopState>,
) -> Result<Vec<ModelChoice>, String> {
    magistr::agent::probe_models(&state.inputs.for_project(&directory), &provider)
        .await
        .map(|choices| {
            choices
                .into_iter()
                .map(|choice| ModelChoice {
                    id: choice.id,
                    name: choice.name,
                })
                .collect()
        })
}
#[derive(Serialize)]
pub(crate) struct SavedConversation {
    id: String,
    name: Option<String>,
    workspace: String,
    provider: String,
    purpose: &'static str,
    model: Option<String>,
    updated_at: String,
    ended_at: Option<String>,
    summary: Option<String>,
    cost: String,
    turns: String,
    resume_reason: Option<String>,
}
fn saved(record: SessionRecord, available: &[AgentRuntime]) -> SavedConversation {
    let resume_reason = if !record.can_resume() {
        Some("This agent cannot resume this conversation. You can read its saved summary.".into())
    } else if !std::path::Path::new(&record.workspace).is_dir() {
        Some("The project folder is unavailable. Restore it before resuming.".into())
    } else if !available
        .iter()
        .any(|runtime| runtime.provider_id() == record.provider)
    {
        Some("This agent is unavailable. Set it up before resuming.".into())
    } else {
        None
    };
    SavedConversation {
        cost: record.cost_display(),
        turns: record.turns_display(),
        provider: AgentRuntime::provider_label_for(&record.provider).to_owned(),
        purpose: crate::wire::purpose(record.purpose),
        id: record.session_id,
        name: record.name,
        workspace: record.workspace,
        model: record.model,
        updated_at: record.updated_at.to_rfc3339(),
        ended_at: record.ended_at.map(|date| date.to_rfc3339()),
        summary: record.summary,
        resume_reason,
    }
}
#[tauri::command]
pub(crate) async fn agent_choices(
    state: State<'_, DesktopState>,
) -> Result<Vec<AgentChoice>, String> {
    let inputs = state.inputs.clone();
    tokio::task::spawn_blocking(move || {
        AgentRuntime::detect_all_for(&inputs)
            .into_iter()
            .map(|runtime| AgentChoice {
                id: runtime.provider_id().to_owned(),
                label: runtime.provider_label().to_owned(),
            })
            .collect()
    })
    .await
    .map_err(|error| error.to_string())
}
#[tauri::command]
pub(crate) async fn saved_conversations(
    state: State<'_, DesktopState>,
) -> Result<Vec<SavedConversation>, String> {
    let inputs = state.inputs.clone();
    tokio::task::spawn_blocking(move || {
        let available = AgentRuntime::detect_all_for(&inputs);
        magistr::application::saved_conversations(&inputs).map(|records| {
            records
                .into_iter()
                .map(|record| saved(record, &available))
                .collect()
        })
    })
    .await
    .map_err(|error| error.to_string())?
}
#[cfg(test)]
#[path = "conversations.tests.rs"]
mod tests;
