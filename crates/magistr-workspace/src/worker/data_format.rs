use serde::{Deserialize, Serialize};

/// Data syntaxes validated inside the isolated native worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerDataFormat {
    Json,
    Toml,
    Yaml,
}
