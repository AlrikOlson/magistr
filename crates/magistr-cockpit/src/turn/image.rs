//! A decoded image block of the stream.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::pixel::CellSize;

/// The tallest an image is allowed to stand in the stream, in rows, on a
/// terminal: past this it is scaled down, so one screenshot can never
/// bury the turn. A surface with the room for more — a window — raises
/// it through [`StreamImage::set_row_cap`].
pub const MAX_IMAGE_ROWS: u16 = 20;

/// The widest image the cockpit decodes, per side. A block past this is
/// refused and shows its alt text — the pixels would cost more than the
/// stream is worth.
const MAX_IMAGE_PIXELS: u32 = 8192;

/// One cell of a halfblock row: the pixel sampled at the top half of the
/// cell and the one at its bottom half, each as RGB.
pub type Halfblock = ((u8, u8, u8), (u8, u8, u8));

/// An image the agent sent, decoded once: its alt text, its pixels as
/// straight (non-premultiplied) RGBA, and the terminal cell it is
/// measured against.
///
/// PNG, JPEG and WebP are decoded — the three formats agents and vision
/// tools actually send. A block in any other format never becomes a
/// `StreamImage`; the stream shows its alt text instead.
///
/// The cell size travels with the image so [`footprint`](Self::footprint)
/// — how many cells the image takes at a stage width — is answerable
/// without threading the terminal's geometry through the wrap.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StreamImage {
    alt: String,
    width: u16,
    height: u16,
    rgba: Arc<Vec<u8>>,
    cell: CellSize,
    /// The tallest the image may stand, in rows of `cell`.
    row_cap: u16,
    /// A hash of the pixels — the picture's identity for a cache that
    /// admits it once, computed here so nothing has to hash the buffer
    /// again per frame.
    digest: u64,
}

impl StreamImage {
    /// The image `bytes` encode, labelled `alt`, measured against `cell`.
    ///
    /// The format is decided by [`Format::sniff`] — from the bytes, not
    /// from whatever MIME type the block claimed.
    ///
    /// `None` when the bytes are not one of the three formats this
    /// build reads, when the decoder refuses them, or when the image is
    /// larger than [`MAX_IMAGE_PIXELS`] on a side.
    #[must_use]
    pub fn decode(bytes: &[u8], alt: impl Into<String>, cell: CellSize) -> Option<Self> {
        let (width, height, rgba) = match Format::sniff(bytes)? {
            Format::Png => png(bytes),
            Format::Jpeg => jpeg(bytes),
            Format::WebP => webp(bytes),
        }?;
        let width = u16::try_from(width).ok()?;
        let height = u16::try_from(height).ok()?;
        if width == 0 || height == 0 {
            return None;
        }
        let mut hasher = DefaultHasher::new();
        rgba.hash(&mut hasher);
        Some(Self {
            alt: alt.into(),
            width,
            height,
            digest: hasher.finish(),
            rgba: Arc::new(rgba),
            cell,
            row_cap: MAX_IMAGE_ROWS,
        })
    }

    /// What the block said the image shows.
    #[must_use]
    pub fn alt(&self) -> &str {
        &self.alt
    }

    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// The pixels, straight RGBA, row-major.
    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// The cell size the footprint is measured against.
    #[must_use]
    pub const fn cell_size(&self) -> CellSize {
        self.cell
    }

    /// Measure against `cell` from here on.
    pub fn set_cell_size(&mut self, cell: CellSize) {
        self.cell = cell;
    }

    /// The tallest the image may stand, in rows — [`MAX_IMAGE_ROWS`]
    /// until a surface with more room says otherwise.
    #[must_use]
    pub const fn row_cap(&self) -> u16 {
        self.row_cap
    }

    /// Let the image stand up to `rows` tall from here on; a cap under
    /// one row is one row, since an image always takes a row.
    pub fn set_row_cap(&mut self, rows: u16) {
        self.row_cap = rows.max(1);
    }

    /// The picture's identity: a hash of its pixels and nothing else —
    /// not its label, not the cell it is measured against — so a
    /// renderer that caches a decoded picture can admit it once and
    /// find it again from any line that carries the same bytes.
    #[must_use]
    pub const fn digest(&self) -> u64 {
        self.digest
    }

    /// How many `(columns, rows)` of cells the image takes when it is
    /// given `columns` to sit in: its natural size in cells, shrunk to
    /// fit the width and its row cap with its aspect kept. Never
    /// zero on either side while `columns` is non-zero, so an image
    /// always occupies at least one row and can always be found again.
    #[must_use]
    pub fn footprint(&self, columns: u16) -> (u16, u16) {
        if columns == 0 {
            return (0, 0);
        }
        let natural_cols = u32::from(self.width).div_ceil(u32::from(self.cell.width).max(1));
        let natural_rows = u32::from(self.height).div_ceil(u32::from(self.cell.height).max(1));
        let cols = natural_cols.clamp(1, u32::from(columns));
        let rows = natural_rows.max(1);
        // Shrink by whichever bound bites hardest, keeping the aspect:
        // the scale is a ratio of the natural size to the allowance.
        let width_scale = f64::from(cols) / f64::from(natural_cols.max(1));
        let height_scale = f64::from(u32::from(self.row_cap)) / f64::from(rows);
        let scale = width_scale.min(height_scale).min(1.0);
        let fitted_cols = ((f64::from(natural_cols) * scale).round() as u32).clamp(1, cols.max(1));
        let fitted_rows = ((f64::from(natural_rows) * scale).round() as u32)
            .clamp(1, u32::from(self.row_cap))
            .max(1);
        (
            u16::try_from(fitted_cols).unwrap_or(columns),
            u16::try_from(fitted_rows).unwrap_or(self.row_cap),
        )
    }

    /// The colours of one halfblock row of a `cols × rows` placement:
    /// one [`Halfblock`] per column, left to right.
    ///
    /// This is the fallback the cockpit draws when the terminal has no
    /// graphics protocol: `▀` in the upper colour on the lower one puts
    /// two image rows in every cell.
    #[must_use]
    pub fn halfblocks(&self, cols: u16, rows: u16, row: u16) -> Vec<Halfblock> {
        if cols == 0 || rows == 0 || row >= rows {
            return Vec::new();
        }
        (0..cols)
            .map(|col| {
                let x = (f64::from(col) + 0.5) / f64::from(cols);
                let upper = (f64::from(row) + 0.25) / f64::from(rows);
                let lower = (f64::from(row) + 0.75) / f64::from(rows);
                (self.sample(x, upper), self.sample(x, lower))
            })
            .collect()
    }

    /// The pixel at the fractional position `(x, y)` of the image,
    /// nearest-neighbour, composited over black where it is transparent.
    fn sample(&self, x: f64, y: f64) -> (u8, u8, u8) {
        let col = ((x * f64::from(self.width)) as u32).min(u32::from(self.width) - 1);
        let row = ((y * f64::from(self.height)) as u32).min(u32::from(self.height) - 1);
        let at = ((row * u32::from(self.width) + col) * 4) as usize;
        let Some(px) = self.rgba.get(at..at + 4) else {
            return (0, 0, 0);
        };
        let over = |channel: u8| ((u32::from(channel) * u32::from(px[3])) / 255) as u8;
        (over(px[0]), over(px[1]), over(px[2]))
    }
}

/// The image formats the cockpit decodes.
///
/// Told from the bytes rather than from the block's MIME type: the MIME
/// only builds the alt text, an agent's label can be absent or wrong,
/// and the format is one decision rather than two that can disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Png,
    Jpeg,
    WebP,
}

impl Format {
    /// Which format `bytes` open with, or `None` for anything else — a
    /// format this build has no decoder for, or not an image at all.
    fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            Some(Self::Png)
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
            Some(Self::WebP)
        } else {
            None
        }
    }
}

/// Whether an image of `width × height` is inside [`MAX_IMAGE_PIXELS`].
const fn within_ceiling(width: u32, height: u32) -> bool {
    width <= MAX_IMAGE_PIXELS && height <= MAX_IMAGE_PIXELS
}

/// A PNG as `(width, height, straight RGBA)`.
///
/// tiny-skia decodes before it reports a size, so unlike the other two
/// the ceiling can only be checked after the allocation.
fn png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let pixmap = tiny_skia::Pixmap::decode_png(bytes).ok()?;
    if !within_ceiling(pixmap.width(), pixmap.height()) {
        return None;
    }
    // tiny-skia keeps premultiplied pixels; the sampler and the kitty
    // payload both want straight alpha. The other two decoders hand
    // back straight pixels already, so this loop stays PNG-only.
    let mut rgba = Vec::with_capacity(pixmap.pixels().len() * 4);
    for px in pixmap.pixels() {
        let straight = px.demultiply();
        rgba.extend_from_slice(&[
            straight.red(),
            straight.green(),
            straight.blue(),
            straight.alpha(),
        ]);
    }
    Some((pixmap.width(), pixmap.height(), rgba))
}

/// A JPEG as `(width, height, straight RGBA)`.
///
/// The headers are read on their own so the ceiling is answered before
/// the pixels are allocated: a decode bomb is refused at its declared
/// size, not after it has been unpacked.
fn jpeg(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    use zune_jpeg::zune_core::{colorspace::ColorSpace, options::DecoderOptions};

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder =
        zune_jpeg::JpegDecoder::new_with_options(std::io::Cursor::new(bytes), options);
    decoder.decode_headers().ok()?;
    let info = decoder.info()?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    if !within_ceiling(width, height) {
        return None;
    }
    // JPEG carries no alpha; the RGBA colourspace fills it opaque.
    Some((width, height, decoder.decode().ok()?))
}

/// A WebP as `(width, height, straight RGBA)`, lossy or lossless, its
/// first frame when it is animated.
///
/// The container declares its size, so the ceiling is answered before
/// the pixels are allocated. A WebP with no alpha channel decodes to
/// RGB and is widened here rather than left three-channel.
fn webp(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes)).ok()?;
    let (width, height) = decoder.dimensions();
    if !within_ceiling(width, height) {
        return None;
    }
    let mut buffer = vec![0; decoder.output_buffer_size()?];
    decoder.read_image(&mut buffer).ok()?;
    if decoder.has_alpha() {
        return Some((width, height, buffer));
    }
    let mut rgba = Vec::with_capacity(buffer.len() / 3 * 4);
    for px in buffer.chunks_exact(3) {
        rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
    }
    Some((width, height, rgba))
}

#[cfg(test)]
#[path = "image.tests.rs"]
mod tests;
