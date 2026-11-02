//! Per-connection authority. Composition cannot widen a planning session.
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Access {
    #[default]
    Full,
    Planning,
    ReadOnly,
}
impl Access {
    pub(crate) fn permits(self, name: &str, args: &Value) -> bool {
        if self == Self::Full {
            return true;
        }
        if matches!(
            name,
            "context"
                | "search"
                | "read"
                | "map"
                | "workspace_status"
                | "run_status"
                | "plan_status"
                | "plan_next"
                | "plan_get"
                | "decision_status"
                | "decision_search"
                | "decision_get"
                | "decision_logs"
                | "decision_log"
                | "request_ready"
        ) {
            return true;
        }
        if self == Self::ReadOnly {
            return false;
        }
        match name {
            "plan_add_task" | "plan_update_task" => args
                .get("status")
                .is_none_or(|v| matches!(v.as_str(), Some("to_do" | "idea"))),
            "plan_set_status" => matches!(args["status"].as_str(), Some("to_do" | "idea")),
            "plan_retire_task"
            | "plan_reprioritize"
            | "plan_link"
            | "decision_record"
            | "decision_pin"
            | "decision_review"
            | "decision_set_thread_status"
            | "decision_impact"
            | "decision_revise_estimate"
            | "request_capture" => true,
            _ => false,
        }
    }
}
#[cfg(test)]
#[path = "access.tests.rs"]
mod tests;
