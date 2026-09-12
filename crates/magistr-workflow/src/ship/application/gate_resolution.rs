use serde::Serialize;

/// An approval gate's current resolution, independent of its transport.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum GateResolution {
    Answered {
        choice: String,
        decided_by: String,
        decided_at: String,
        note: Option<String>,
    },
    Expired {
        choice: String,
        note: &'static str,
    },
    Pending {
        seconds_left: i64,
        hint: &'static str,
    },
}
