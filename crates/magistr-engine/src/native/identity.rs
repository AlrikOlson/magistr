use std::path::Path;

use magistr_workflow::infra::{IdSource, ProjectIdentity, resolve_project_id_with};

/// The think-and-ship identity declaration a first init plants at
/// `root/.think-and-ship/project.json`, or `None` when a repository file
/// already declares one at `root` or an ancestor.
///
/// The id is resolved exactly as the server resolves it — environment
/// override, then an existing `.think-and-ship/project.json` found walking
/// up from `root`, then the deterministic `<basename>-<hash>` fallback — and
/// the declaration carries that already-resolved id. The id is never minted
/// here: every store the project already holds is keyed by what resolution
/// returns today, and declaring anything else would orphan it. The bytes are
/// the workflow's own rendering, so the setup plan shows the file that
/// setup's administrative service writes.
#[must_use]
pub fn identity_declaration(root: &Path) -> Option<String> {
    let (id, source) = resolve_project_id_with(None, Some(root));
    (source != IdSource::RepoFile).then(|| ProjectIdentity { id, name: None }.declaration())
}

#[cfg(test)]
#[path = "identity.tests.rs"]
mod tests;
