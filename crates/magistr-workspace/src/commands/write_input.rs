use serde::{Deserialize, Serialize};

/// One write call: the whole content of one workspace file, as exact bytes.
/// This is the verb Magistr's own source writes take after the session's
/// permission decision — an ACP `fs/write_text_file`, a host-performed
/// edit — so they carry a journal intent and answer a truthful receipt like
/// every other mutation. That decision is the confirmation: the write is
/// never confirm-gated and never formatted, and it is verified like any
/// other change. Serialization yields the sealed intent bytes; `budget`
/// never enters the binding.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteInput {
    pub path: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
    #[serde(default = "default_budget", skip_serializing)]
    pub budget: u64,
}

impl WriteInput {
    #[must_use]
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
            expected_hash: None,
            budget: default_budget(),
        }
    }
}

fn default_budget() -> u64 {
    4000
}
