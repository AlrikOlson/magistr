use std::path::{Path, PathBuf};

use crate::cockpit::{Chapter, Shelf};

/// Write `chapter` as one self-contained page under `dir` —
/// [`Chapter::PAGE`], share route: the same document
/// the Report view composes and the bundle carries, as a file a
/// stakeholder opens. The path written.
///
/// # Errors
///
/// The directory could not be made or the page could not be written.
pub fn write_page(shelf: &Shelf, chapter: &Chapter, dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let page = dir.join(Chapter::PAGE);
    std::fs::write(&page, chapter.html(shelf))?;
    Ok(page)
}

#[cfg(test)]
#[path = "page.tests.rs"]
mod tests;
