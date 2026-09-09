//! An image reduced to the at-most-256 colour registers sixel allows.
//!
//! Two regimes, because flat UI widgets and photographic gradients want
//! different things. An image with 256 distinct colours or fewer keeps
//! every one of them, so a rail gauge or a brand mark round-trips
//! losslessly. Anything richer goes through a median cut over a 5-bit
//! histogram: the colour cube is split along its widest axis at the
//! count-weighted median until there are 256 boxes, and each box's
//! weighted average becomes one register.
//!
//! Sixel has no alpha, so a pixel below [`ALPHA_FLOOR`] carries no index
//! at all — the encoder emits no bit for it and the cell keeps whatever
//! was under the image.

use std::collections::{HashMap, HashSet};

use magistr_cockpit::pixel::PixelFormat;

/// Below this alpha a pixel is treated as absent rather than blended:
/// there is no background to blend against at encode time.
const ALPHA_FLOOR: u8 = 128;

/// Bits dropped from each channel to form a histogram cell — 5 bits per
/// channel is 32 768 cells, faithful enough for the cut and small enough
/// to sort every frame.
const DROPPED_BITS: u8 = 3;

/// An image's colours as sixel can carry them: a palette of at most
/// [`Palette::MAX`] RGB triples, and one optional index per pixel in the
/// order the pixels were given (`None` = transparent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    colors: Vec<[u8; 3]>,
    indices: Vec<Option<u8>>,
}

/// One histogram cell: every pixel whose colour reduces to `key`.
#[derive(Clone, Copy, Debug)]
struct Cell {
    key: u16,
    /// The cell's mean colour, what the cut measures distances on.
    mean: [u8; 3],
    /// Channel sums over the cell's pixels, for the box's weighted mean.
    sum: [u64; 3],
    count: u64,
}

/// A box of the cut: the half-open range of cells it owns and their
/// total pixel count (kept beside the range so choosing the next box to
/// split does not rescan).
#[derive(Clone, Copy, Debug)]
struct Bin {
    start: usize,
    end: usize,
    count: u64,
}

impl Palette {
    /// The most colour registers a sixel image can define.
    pub const MAX: usize = 256;

    /// Quantise `pixels` — row-major in `format`'s layout — to at most
    /// [`Palette::MAX`] colours. A trailing partial pixel is ignored.
    #[must_use]
    pub fn quantize(pixels: &[u8], format: PixelFormat) -> Self {
        match exact_colors(pixels, format) {
            Some(colors) => Self::exact(pixels, format, colors),
            None => Self::cut(pixels, format),
        }
    }

    /// The palette, in the order the indices address it.
    #[must_use]
    pub fn colors(&self) -> &[[u8; 3]] {
        &self.colors
    }

    /// One entry per pixel: its palette index, or `None` when the pixel
    /// is too transparent to paint.
    #[must_use]
    pub fn indices(&self) -> &[Option<u8>] {
        &self.indices
    }

    /// Every colour kept as it is, sorted so the output is stable.
    fn exact(pixels: &[u8], format: PixelFormat, colors: HashSet<[u8; 3]>) -> Self {
        let mut colors: Vec<[u8; 3]> = colors.into_iter().collect();
        colors.sort_unstable();
        let lookup: HashMap<[u8; 3], u8> = colors
            .iter()
            .enumerate()
            .filter_map(|(n, color)| Some((*color, u8::try_from(n).ok()?)))
            .collect();
        let indices = opaque_colors(pixels, format)
            .map(|color| color.and_then(|color| lookup.get(&color).copied()))
            .collect();
        Self { colors, indices }
    }

    /// Median cut over the 5-bit histogram.
    fn cut(pixels: &[u8], format: PixelFormat) -> Self {
        let mut cells = histogram(pixels, format);
        let bins = split_until(&mut cells, Self::MAX);
        let mut colors = Vec::with_capacity(bins.len());
        let mut lookup: HashMap<u16, u8> = HashMap::with_capacity(cells.len());
        for (n, bin) in bins.iter().enumerate() {
            let Ok(index) = u8::try_from(n) else { break };
            colors.push(mean(&cells[bin.start..bin.end]));
            for cell in &cells[bin.start..bin.end] {
                lookup.insert(cell.key, index);
            }
        }
        let indices = opaque_colors(pixels, format)
            .map(|color| color.and_then(|color| lookup.get(&key(color)).copied()))
            .collect();
        Self { colors, indices }
    }
}

/// Each pixel's colour, or `None` when its alpha is below the floor.
fn opaque_colors(pixels: &[u8], format: PixelFormat) -> impl Iterator<Item = Option<[u8; 3]>> + '_ {
    let bytes = format.bytes_per_pixel();
    pixels.chunks_exact(bytes).map(move |px| {
        let transparent = matches!(px.get(3), Some(alpha) if *alpha < ALPHA_FLOOR);
        (!transparent).then(|| [px[0], px[1], px[2]])
    })
}

/// The image's distinct colours when there are few enough to keep them
/// all, `None` once the count passes [`Palette::MAX`].
fn exact_colors(pixels: &[u8], format: PixelFormat) -> Option<HashSet<[u8; 3]>> {
    let mut colors = HashSet::new();
    for color in opaque_colors(pixels, format).flatten() {
        colors.insert(color);
        if colors.len() > Palette::MAX {
            return None;
        }
    }
    Some(colors)
}

/// The colour's histogram cell: five bits per channel, red highest.
fn key(color: [u8; 3]) -> u16 {
    let reduced = |channel: u8| u16::from(channel >> DROPPED_BITS);
    (reduced(color[0]) << 10) | (reduced(color[1]) << 5) | reduced(color[2])
}

/// The image's non-transparent pixels bucketed by [`key`], cells ordered
/// by key so the cut starts from the same place every time.
fn histogram(pixels: &[u8], format: PixelFormat) -> Vec<Cell> {
    let mut cells: HashMap<u16, Cell> = HashMap::new();
    for color in opaque_colors(pixels, format).flatten() {
        let cell = cells.entry(key(color)).or_insert(Cell {
            key: key(color),
            mean: color,
            sum: [0; 3],
            count: 0,
        });
        for (channel, value) in cell.sum.iter_mut().zip(color) {
            *channel += u64::from(value);
        }
        cell.count += 1;
    }
    let mut cells: Vec<Cell> = cells.into_values().collect();
    for cell in &mut cells {
        let center = mean(std::slice::from_ref(&*cell));
        cell.mean = center;
    }
    cells.sort_unstable_by_key(|cell| cell.key);
    cells
}

/// The pixel-count-weighted mean colour of `cells`.
fn mean(cells: &[Cell]) -> [u8; 3] {
    let count: u64 = cells.iter().map(|cell| cell.count).sum();
    if count == 0 {
        return [0; 3];
    }
    let mut mean = [0u8; 3];
    for (channel, value) in mean.iter_mut().enumerate() {
        let sum: u64 = cells.iter().map(|cell| cell.sum[channel]).sum();
        *value = u8::try_from((sum + count / 2) / count).unwrap_or(u8::MAX);
    }
    mean
}

/// Split the widest, heaviest box in two until there are `max` of them or
/// none can be split further. `cells` is permuted so every box is one
/// contiguous range.
fn split_until(cells: &mut [Cell], max: usize) -> Vec<Bin> {
    if cells.is_empty() {
        return Vec::new();
    }
    let mut bins = vec![Bin {
        start: 0,
        end: cells.len(),
        count: cells.iter().map(|cell| cell.count).sum(),
    }];
    while bins.len() < max {
        let Some(pick) = heaviest(&bins) else { break };
        let bin = bins[pick];
        let at = bin.start + split(&mut cells[bin.start..bin.end]);
        let left = cells[bin.start..at].iter().map(|cell| cell.count).sum();
        bins[pick] = Bin {
            start: bin.start,
            end: at,
            count: left,
        };
        bins.push(Bin {
            start: at,
            end: bin.end,
            count: bin.count - left,
        });
    }
    bins
}

/// The splittable box holding the most pixels; ties go to the earliest so
/// the cut is deterministic.
fn heaviest(bins: &[Bin]) -> Option<usize> {
    bins.iter()
        .enumerate()
        .filter(|(_, bin)| bin.end - bin.start > 1)
        .max_by_key(|(n, bin)| (bin.count, std::cmp::Reverse(*n)))
        .map(|(n, _)| n)
}

/// Sort `cells` along their widest channel and return how many belong to
/// the lower half by pixel count — at least one, at most all but one.
fn split(cells: &mut [Cell]) -> usize {
    let axis = widest_axis(cells);
    cells.sort_unstable_by_key(|cell| (cell.mean[axis], cell.key));
    let total: u64 = cells.iter().map(|cell| cell.count).sum();
    let mut running = 0;
    let mut at = cells.len();
    for (n, cell) in cells.iter().enumerate() {
        running += cell.count;
        if running * 2 >= total {
            at = n + 1;
            break;
        }
    }
    at.clamp(1, cells.len().saturating_sub(1).max(1))
}

/// The channel over which the cells' mean colours spread furthest.
fn widest_axis(cells: &[Cell]) -> usize {
    (0..3)
        .max_by_key(|channel| {
            let values = cells.iter().map(|cell| cell.mean[*channel]);
            let (low, high) = values.fold((u8::MAX, u8::MIN), |(low, high), value| {
                (low.min(value), high.max(value))
            });
            high.saturating_sub(low)
        })
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "palette.tests.rs"]
mod tests;
