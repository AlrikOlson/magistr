//! The Tasks extension surface: the one `index/build` task, derived on the
//! spot from the index status — no task store. Served two ways: SEP-2663
//! `tasks/get` through rmcp's typed hook for clients that declare the
//! extension, and the retained `tasks/list` answer for the polling flow the
//! reference clients used.

use magistr_workspace::{
    diagnostics::utc_timestamp,
    service::{IndexBuildState, IndexBuildTask},
};
use rmcp::model::{DetailedTask, JsonObject, Task, TaskPayload, TaskStatus};
use serde_json::{Value, json};

fn status(state: IndexBuildState) -> TaskStatus {
    match state {
        IndexBuildState::Working => TaskStatus::Working,
        IndexBuildState::Completed => TaskStatus::Completed,
        IndexBuildState::Failed => TaskStatus::Failed,
    }
}

/// The SEP-2663 view: task metadata plus the payload its status implies.
pub(crate) fn detailed(task: &IndexBuildTask) -> DetailedTask {
    let mut wire = Task::new(
        IndexBuildTask::ID,
        status(task.state),
        utc_timestamp(task.created_at_ms),
        utc_timestamp(task.last_updated_at_ms),
    );
    wire.status_message = Some(task.message.clone());
    wire.poll_interval_ms = task.poll_interval_ms;
    let payload = match task.state {
        IndexBuildState::Working => TaskPayload::Working,
        IndexBuildState::Completed => TaskPayload::Completed {
            result: JsonObject::new(),
        },
        IndexBuildState::Failed => TaskPayload::Failed {
            error: json!({"message": task.message})
                .as_object()
                .cloned()
                .unwrap_or_default(),
        },
    };
    DetailedTask::new(wire, payload)
}

/// The retained `tasks/list` row: `ttl` null by design, the record lives
/// exactly as long as the index does.
pub(crate) fn listing(task: &IndexBuildTask) -> Value {
    let mut row = json!({
        "taskId": IndexBuildTask::ID,
        "status": task.state.as_str(),
        "statusMessage": task.message,
        "createdAt": utc_timestamp(task.created_at_ms),
        "lastUpdatedAt": utc_timestamp(task.last_updated_at_ms),
        "ttl": Value::Null,
    });
    if let Some(interval) = task.poll_interval_ms {
        row["pollInterval"] = json!(interval);
    }
    row
}

#[cfg(test)]
#[path = "tasks.tests.rs"]
mod tests;
