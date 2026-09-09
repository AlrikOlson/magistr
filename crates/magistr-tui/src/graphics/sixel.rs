//! An RGBA image as a sixel DCS string.
//!
//! Sixel is the older of the two protocols the cockpit speaks and it is
//! poorer in every way that matters here: there is no image id, no
//! placement and no alpha, so a surface is put on screen by moving the
//! cursor and emitting the whole image again. What it does have is a
//! compact run-length encoding, and this module leans on it.
//!
//! The wire shape is `ESC P 0;1;0 q` — no aspect scaling, background left
//! alone (`P2 = 1`, so a pixel the image does not paint keeps whatever was
//! in the cell) — then the raster attributes `"1;1;<width>;<height>`, then
//! one `#n;2;r;g;b` definition per colour register (each channel a 0..100
//! percentage, not a byte), then the pixels as bands of six rows: within a
//! band each colour writes one pass of sixel characters, `$` returns to
//! the left margin for the next colour and `-` starts the next band. A
//! sixel character carries six vertical pixels of one colour as bits of
//! `char - 0x3F`, and `!<count><char>` repeats one.

use std::collections::BTreeMap;

use super::error::GraphicsError;
use super::palette::Palette;
use crate::render::encode::push_number;
use magistr_cockpit::pixel::PixelFormat;

/// Rows of pixels one sixel character carries.
const BAND: usize = 6;
/// The sixel character with no bit set; a value is added to it.
const EMPTY: u8 = 0x3F;
/// Shortest run worth the `!<count>` introducer: three characters of
/// introducer plus the repeated one only pays from four.
const RUN: usize = 4;
/// Device control string opener: no aspect scaling, background untouched.
const INTRODUCER: &[u8] = b"\x1bP0;1;0q";
/// String terminator.
const TERMINATOR: &[u8] = b"\x1b\\";

/// Append `pixels` — row-major in `format`'s layout, `width × height` —
/// to `out` as one sixel image, quantised to at most [`Palette::MAX`]
/// colours.
///
/// Nothing is written when the buffer does not match the dimensions.
pub fn encode(
    out: &mut Vec<u8>,
    format: PixelFormat,
    width: u16,
    height: u16,
    pixels: &[u8],
) -> Result<(), GraphicsError> {
    if width == 0 || height == 0 {
        return Err(GraphicsError::EmptyImage);
    }
    let expected = usize::from(width) * usize::from(height) * format.bytes_per_pixel();
    if pixels.len() != expected {
        return Err(GraphicsError::PixelCount {
            expected,
            actual: pixels.len(),
        });
    }
    let palette = Palette::quantize(pixels, format);
    out.extend_from_slice(INTRODUCER);
    push_raster(out, width, height);
    push_registers(out, palette.colors());
    push_bands(out, palette.indices(), width, height);
    out.extend_from_slice(TERMINATOR);
    Ok(())
}

/// `"Pan;Pad;Ph;Pv` — square pixels, and the size the terminal should
/// reserve before any data arrives.
fn push_raster(out: &mut Vec<u8>, width: u16, height: u16) {
    out.extend_from_slice(b"\"1;1;");
    push_number(out, u32::from(width));
    out.push(b';');
    push_number(out, u32::from(height));
}

/// One `#n;2;r;g;b` per palette entry, each channel as a percentage.
fn push_registers(out: &mut Vec<u8>, colors: &[[u8; 3]]) {
    for (n, color) in colors.iter().enumerate() {
        out.push(b'#');
        push_number(out, u32::try_from(n).unwrap_or(u32::MAX));
        out.extend_from_slice(b";2");
        for channel in color {
            out.push(b';');
            push_number(out, percent(*channel));
        }
    }
}

/// A channel byte as the 0..100 percentage a colour register takes.
fn percent(channel: u8) -> u32 {
    (u32::from(channel) * 100 + 127) / 255
}

/// The image as bands of six rows, one pass per colour present in the
/// band, colours in register order so the output is stable.
fn push_bands(out: &mut Vec<u8>, indices: &[Option<u8>], width: u16, height: u16) {
    let width = usize::from(width);
    let height = usize::from(height);
    let bands = height.div_ceil(BAND);
    for band in 0..bands {
        let mut passes: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
        for row in 0..BAND {
            let y = band * BAND + row;
            if y >= height {
                break;
            }
            let line = &indices[y * width..(y + 1) * width];
            for (x, index) in line.iter().enumerate() {
                if let Some(index) = index {
                    let bits = passes.entry(*index).or_insert_with(|| vec![0; width]);
                    bits[x] |= 1 << row;
                }
            }
        }
        for (n, (index, bits)) in passes.iter().enumerate() {
            if n > 0 {
                out.push(b'$');
            }
            out.push(b'#');
            push_number(out, u32::from(*index));
            push_pass(out, bits);
        }
        if band + 1 < bands {
            out.push(b'-');
        }
    }
}

/// One colour's pass across a band: run-length encoded sixel characters,
/// with the trailing run of untouched columns dropped (nothing to paint
/// there, and `$`/`-` return to the margin anyway).
fn push_pass(out: &mut Vec<u8>, bits: &[u8]) {
    let end = bits
        .iter()
        .rposition(|value| *value != 0)
        .map_or(0, |n| n + 1);
    let mut at = 0;
    while at < end {
        let value = bits[at];
        let mut run = 1;
        while at + run < end && bits[at + run] == value {
            run += 1;
        }
        push_run(out, value, run);
        at += run;
    }
}

/// `run` copies of the character for `value`, through the repeat
/// introducer once that is the shorter spelling.
fn push_run(out: &mut Vec<u8>, value: u8, run: usize) {
    let character = EMPTY + value;
    if run >= RUN {
        out.push(b'!');
        push_number(out, u32::try_from(run).unwrap_or(u32::MAX));
        out.push(character);
    } else {
        out.extend(std::iter::repeat_n(character, run));
    }
}

#[cfg(test)]
#[path = "sixel.tests.rs"]
mod tests;
