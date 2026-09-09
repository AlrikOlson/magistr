use std::path::Path;

use think_and_ship::infra::{IdSource, resolve_project_id_with, write_project_file};

/// Make sure `root` declares its think-and-ship identity, returning the id.
///
/// Resolves the id exactly as the external server would — environment
/// override, then an existing `.think-and-ship/project.json` found walking
/// up from `root`, then the deterministic `<basename>-<hash>` fallback — and
/// only when nothing is declared writes that already-resolved id to
/// `root/.think-and-ship/project.json`. The id is never minted here: every
/// store the project already holds is keyed by what resolution returns
/// today, and writing anything else would orphan it.
///
/// Called by `magistr init`; idempotent.
pub fn ensure_project_file(root: &Path) -> std::io::Result<String> {
    let (id, source) = resolve_project_id_with(None, Some(root));
    if source != IdSource::RepoFile {
        write_project_file(root, &id, None)?;
    }
    Ok(id)
}

#[cfg(test)]
#[path = "identity.tests.rs"]
mod tests;
