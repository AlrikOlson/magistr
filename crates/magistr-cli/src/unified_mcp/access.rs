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
                | "ship_status"
                | "roadmap_status"
                | "roadmap_next"
                | "roadmap_get"
                | "think_engine_status"
                | "think_search_trace"
                | "think_get_step"
                | "think_sessions"
                | "think_session"
                | "signal_pending"
        ) {
            return true;
        }
        if self == Self::ReadOnly {
            return false;
        }
        match name {
            "roadmap_add_chunk" | "roadmap_update_chunk" => args
                .get("status")
                .is_none_or(|v| matches!(v.as_str(), Some("pending" | "backlog"))),
            "roadmap_set_status" => matches!(args["status"].as_str(), Some("pending" | "backlog")),
            "roadmap_obsolete_chunk"
            | "roadmap_reprioritize"
            | "roadmap_link"
            | "think_record_step"
            | "think_pin_step"
            | "think_trace_checkpoint"
            | "think_set_branch_status"
            | "think_step_impact"
            | "think_revise_estimate"
            | "signal_capture" => true,
            _ => false,
        }
    }
}
#[cfg(test)]
#[path = "access.tests.rs"]
mod tests;
