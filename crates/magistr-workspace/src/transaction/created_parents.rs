use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Create the missing ancestors of `parent` below `root` one component at a
/// time and return exactly the directories this call created, outermost
/// first. A directory that already exists, or that another writer creates
/// between the check and the attempt, is never claimed, so compensation can
/// only ever remove what this batch made.
pub(super) fn create_missing_parents(root: &Path, parent: &Path) -> io::Result<Vec<PathBuf>> {
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| io::Error::other("outside root"))?;
    let mut created = Vec::new();
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::create_dir(&current) {
            Ok(()) => created.push(current.clone()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Ok(created)
}
