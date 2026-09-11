use serde::Serialize;

use super::{ScriptResult, WriteReport};

/// The script envelope's rows: the script row first, then one write row per
/// queued change exactly as the edit and files verbs report them.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ScriptRow {
    Script(ScriptResult),
    Write(WriteReport),
}
