use serde::{Deserialize, Serialize};

/// One write call: the whole content of one workspace file, as exact bytes.
/// Two callers take this verb. Magistr's own source writes arrive after the
/// session's permission decision — an ACP `fs/write_text_file`, a
/// host-performed edit — and that decision is the confirmation, so
/// [`WriteInput::new`] is authorized past the gate. A served call decodes
/// with nothing authorized and takes the same confirm gate as a single-file
/// edit. Either way the write is never formatted, is verified like any
/// other change, and carries a journal intent with a truthful receipt.
/// Serialization yields the sealed intent bytes; `confirm`, `dry_run`,
/// `auto_apply` and `budget` never enter the binding.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteInput {
    pub path: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
    #[serde(default, skip_serializing)]
    pub confirm: Option<String>,
    #[serde(default, skip_serializing)]
    pub dry_run: bool,
    #[serde(default, skip_serializing)]
    pub auto_apply: bool,
    #[serde(default = "default_budget", skip_serializing)]
    pub budget: u64,
}

impl WriteInput {
    /// A write the host already authorized: it is not confirm-gated.
    #[must_use]
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
            expected_hash: None,
            confirm: None,
            dry_run: false,
            auto_apply: true,
            budget: default_budget(),
        }
    }
}

fn default_budget() -> u64 {
    4000
}
