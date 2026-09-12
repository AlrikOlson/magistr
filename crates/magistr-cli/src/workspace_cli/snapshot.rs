//! `magistr workspace snapshot`: the source certification observation of a
//! workspace — canonical identity, durable generation, selection policy and
//! the content manifest — read beside a live server without any lock, and
//! printed as the serialized `SourceSnapshot` for whoever certifies a check.

use std::path::Path;

use color_eyre::eyre;
use magistr_workspace::{
    service::WorkspaceOpenError,
    snapshot::{SelectionPolicy, SnapshotError, SourceSnapshot},
};

use super::{admitted_root, exit::EXIT_USAGE, fail};

/// Print the snapshot of the workspace at `dir` with the retained exit
/// codes: 66 for a missing root, 64 for an unusable explicit input, 1 for
/// a refused observation.
pub fn run_snapshot(dir: &Path, artifacts: &[String]) -> eyre::Result<()> {
    let root = admitted_root(dir);
    let policy = artifacts
        .iter()
        .fold(SelectionPolicy::default(), |policy, artifact| {
            policy.with_artifact(artifact.clone())
        });
    let snapshot = match SourceSnapshot::capture(&root, policy) {
        Ok(snapshot) => snapshot,
        Err(error @ SnapshotError::RootMissing(_)) => {
            fail(WorkspaceOpenError::EXIT_NO_INPUT, &error.to_string())
        }
        Err(error @ SnapshotError::InvalidInput { .. }) => fail(EXIT_USAGE, &error.to_string()),
        Err(error) => fail(1, &error.to_string()),
    };
    println!("{}", serde_json::to_string_pretty(&snapshot)?);
    Ok(())
}
