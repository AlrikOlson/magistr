use serde::{Deserialize, Serialize};

/// Host-curated argv admission; guests select a language id, never a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerSubprocessSpec {
    pub language_id: String,
    pub bin: String,
    pub args: Vec<String>,
}

impl From<WorkerSubprocessSpec> for crate::sandbox::SubprocessSpec {
    fn from(spec: WorkerSubprocessSpec) -> Self {
        Self {
            language_id: spec.language_id,
            bin: spec.bin,
            args: spec.args,
        }
    }
}
