use std::path::Path;

use crate::cockpit::{ShelfStill, StillKind};
use crate::pixel::CellSize;
use crate::turn::StreamImage;

/// The still at `source`, decoded; `None` when the file is missing, is
/// not a picture the cockpit reads, or has no file name.
///
/// The picture is a [`StreamImage`]: the cockpit already owns a decoder
/// and a straight-RGBA buffer for the pictures an agent sends, and a
/// golden is a picture like any other. The cell it is measured against
/// is the default one — the shelf never places a still on a cell grid;
/// the window draws it at a thumbnail height, and the bundle copies the
/// bytes on disk rather than re-encoding the decoded pixels.
#[must_use]
pub fn load_still(source: &Path) -> Option<ShelfStill> {
    let name = source.file_name()?.to_string_lossy().into_owned();
    let bytes = std::fs::read(source).ok()?;
    let image = StreamImage::decode(&bytes, name.clone(), CellSize::DEFAULT)?;
    Some(ShelfStill {
        name,
        source: source.to_path_buf(),
        kind: StillKind::of(source),
        image,
    })
}

#[cfg(test)]
#[path = "load_still.tests.rs"]
mod tests;
