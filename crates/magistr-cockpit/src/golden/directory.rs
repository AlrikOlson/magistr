//! Golden images: the review flow that compares a frame against its
//! checked-in PNG.

use std::fs;
use std::path::{Path, PathBuf};

use tiny_skia::Pixmap;

use super::error::GoldenError;

/// A directory of golden PNGs, one per named frame. A check against a
/// name whose golden is missing writes it and fails (bless deliberately);
/// a mismatch writes `<name>.new.png` beside the golden and fails with
/// both paths; a match removes any stale candidate. With
/// [`Golden::UPDATE_ENV`] set (and not `0`), every check rewrites its
/// golden and passes — the way to re-bless after an intended change.
/// Comparison is on decoded pixels, never on PNG bytes, so an encoder
/// change cannot fail a frame.
///
/// The comparison allows for anti-aliasing: a rasterizer's edge coverage
/// is the last bit of a `sin` or a `sqrt`, and that bit is the platform's
/// (Apple's libm and MSVC's differ), so a curve's edge pixel can land a
/// shade off on another machine. A frame matches when no pixel differs
/// by more than [`Golden::AA_CHANNEL_TOLERANCE`] on any channel and at
/// most [`Golden::AA_PIXEL_BUDGET`] of its pixels differ at all. Anything
/// larger — a glyph moved, a colour changed, a line drawn or gone — is a
/// mismatch, as before.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Golden {
    dir: PathBuf,
}

impl Golden {
    /// Set to anything but `0` to rewrite every golden that is checked.
    pub const UPDATE_ENV: &'static str = "MAGISTR_GOLDEN_UPDATE";

    /// Goldens under `dir`. Every suite roots this in its own crate's
    /// `tests/golden` — the terminal stills and filmstrips in magistr-tui,
    /// the native stills in magistr-gui2 — so a still of one renderer can
    /// never re-bless the other's.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Where `name`'s golden lives.
    #[must_use]
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.png"))
    }

    /// Where a mismatching frame for `name` is written.
    #[must_use]
    pub fn candidate_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.new.png"))
    }

    /// The most a channel may differ by and still be anti-aliasing: an
    /// edge pixel a shade off, never a pixel that changed colour.
    pub const AA_CHANNEL_TOLERANCE: u8 = 64;

    /// The most of a frame that may differ at all, as pixels per
    /// thousand: a few edge pixels, never a shifted shape.
    pub const AA_PIXEL_BUDGET: usize = 1;

    /// Whether the environment asks for re-blessing.
    #[must_use]
    pub fn updating() -> bool {
        std::env::var_os(Self::UPDATE_ENV).is_some_and(|v| !v.is_empty() && v != "0")
    }

    /// Compare `pixmap` with `name`'s golden; see the type's docs for the
    /// three outcomes.
    pub fn check(&self, name: &str, pixmap: &Pixmap) -> Result<(), GoldenError> {
        self.check_with_update(name, pixmap, Self::updating())
    }

    /// Compare or update explicitly, without consulting the process environment.
    fn check_with_update(
        &self,
        name: &str,
        pixmap: &Pixmap,
        updating: bool,
    ) -> Result<(), GoldenError> {
        let golden = self.path(name);
        let candidate = self.candidate_path(name);
        if updating || !golden.exists() {
            let blessed = golden.exists();
            write_png(&golden, pixmap)?;
            forget(&candidate);
            return if updating || blessed {
                Ok(())
            } else {
                Err(GoldenError::Blessed { golden })
            };
        }
        let expected = Pixmap::load_png(&golden).map_err(|e| GoldenError::Io {
            path: golden.clone(),
            error: e.to_string(),
        })?;
        let size_changed =
            expected.width() != pixmap.width() || expected.height() != pixmap.height();
        let (differing, beyond_aa) = if size_changed {
            (pixmap.pixels().len(), pixmap.pixels().len())
        } else {
            expected
                .pixels()
                .iter()
                .zip(pixmap.pixels())
                .filter(|(a, b)| a != b)
                .fold((0, 0), |(all, beyond), (a, b)| {
                    (all + 1, beyond + usize::from(!within_aa(*a, *b)))
                })
        };
        let budget = pixmap.pixels().len() * Self::AA_PIXEL_BUDGET / 1000;
        if !size_changed && beyond_aa == 0 && differing <= budget {
            forget(&candidate);
            return Ok(());
        }
        write_png(&candidate, pixmap)?;
        Err(GoldenError::Mismatch {
            golden,
            candidate,
            differing,
            size_changed,
        })
    }

    /// [`Golden::check`] as a test assertion: panics with the outcome.
    pub fn assert_matches(&self, name: &str, pixmap: &Pixmap) {
        if let Err(error) = self.check(name, pixmap) {
            panic!("{error}");
        }
    }
}

/// Whether two pixels differ by no more than anti-aliasing would.
fn within_aa(a: tiny_skia::PremultipliedColorU8, b: tiny_skia::PremultipliedColorU8) -> bool {
    let delta = [
        a.red().abs_diff(b.red()),
        a.green().abs_diff(b.green()),
        a.blue().abs_diff(b.blue()),
        a.alpha().abs_diff(b.alpha()),
    ];
    delta.into_iter().all(|d| d <= Golden::AA_CHANNEL_TOLERANCE)
}

/// Write `pixmap` as PNG at `path`, creating the directory.
fn write_png(path: &Path, pixmap: &Pixmap) -> Result<(), GoldenError> {
    let io = |e: &dyn std::fmt::Display| GoldenError::Io {
        path: path.to_path_buf(),
        error: e.to_string(),
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io(&e))?;
    }
    pixmap.save_png(path).map_err(|e| io(&e))
}

/// Remove a stale candidate; a candidate that is not there is fine.
fn forget(candidate: &Path) {
    let _ = fs::remove_file(candidate);
}

#[cfg(test)]
#[path = "directory.tests.rs"]
mod tests;
