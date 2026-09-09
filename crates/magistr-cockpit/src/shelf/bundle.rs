use std::path::{Path, PathBuf};

use crate::cockpit::{Chapter, Shelf};

/// Write `shelf` as files under `dir` — export bundle:
/// [`Shelf::RECEIPTS_FILE`] and, when a `chapter` rides along,
/// [`Chapter::FILE`] beside it, then a `stills/` and a `filmstrips/`
/// directory holding each picture copied from its golden. The paths
/// written, receipts first.
///
/// # Errors
///
/// The directory could not be made, a picture could not be copied, or
/// a document could not be written — a picture that has gone missing
/// since the shelf was gathered is an error, never a silent gap in the
/// bundle.
pub fn write_bundle(
    shelf: &Shelf,
    chapter: Option<&Chapter>,
    dir: &Path,
) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let receipts = dir.join(Shelf::RECEIPTS_FILE);
    std::fs::write(&receipts, shelf.receipts_markdown())?;
    let mut written = vec![receipts];
    if let Some(chapter) = chapter {
        let document = dir.join(Chapter::FILE);
        std::fs::write(&document, chapter.markdown(shelf))?;
        written.push(document);
    }
    for still in &shelf.stills {
        let into = dir.join(still.kind.dir_name());
        std::fs::create_dir_all(&into)?;
        let target = into.join(&still.name);
        std::fs::copy(&still.source, &target)?;
        written.push(target);
    }
    Ok(written)
}

#[cfg(test)]
#[path = "bundle.tests.rs"]
mod tests;
