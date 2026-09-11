use serde::Serialize;

/// The existence transition an applied (or previewed) write performs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WriteAction {
    Created,
    Modified,
    Deleted,
    Unchanged,
}
