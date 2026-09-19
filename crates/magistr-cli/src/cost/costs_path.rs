//! Resolve the full path to the costs file.

use std::path::PathBuf;

use crate::config::inputs::ProjectInputs;

/// Default filename for cost history.
const COSTS_FILENAME: &str = "costs.json";

/// The cost ledger, `<data_dir>/costs.json`, over the captured data
/// directory; `None` when this machine has nowhere to keep one.
#[must_use]
pub fn costs_path(inputs: &ProjectInputs) -> Option<PathBuf> {
    inputs.data_dir().map(|dir| dir.join(COSTS_FILENAME))
}
