/// The state reported by the run owner, separate from a requested boundary action.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkPhase {
    Starting,
    Running,
    Paused,
    Ended { reason: String },
}
