use std::collections::HashMap;

use super::*;

/// Channel values that survive a round trip through a colour register's
/// 0..100 percentage exactly, so a decode can be compared pixel for pixel.
const EXACT: [u8; 6] = [0, 51, 102, 153, 204, 255];

/// An RGB buffer of `width × height` painted by `color`.
fn rgb(width: u16, height: u16, color: impl Fn(u16, u16) -> [u8; 3]) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(usize::from(width) * usize::from(height) * 3);
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&color(x, y));
        }
    }
    pixels
}

/// An RGBA buffer of `width × height` painted by `pixel`.
fn rgba(width: u16, height: u16, pixel: impl Fn(u16, u16) -> [u8; 4]) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(usize::from(width) * usize::from(height) * 4);
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&pixel(x, y));
        }
    }
    pixels
}

/// `encode` into a fresh buffer, or the error it refused with.
fn encoded(format: PixelFormat, width: u16, height: u16, pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    encode(&mut out, format, width, height, pixels).expect("a well-formed image encodes");
    out
}

/// A sixel stream decoded back to pixels: `None` where the image painted
/// nothing, so transparency survives the comparison.
struct Image {
    width: usize,
    pixels: Vec<Option<[u8; 3]>>,
}

impl Image {
    fn at(&self, x: u16, y: u16) -> Option<[u8; 3]> {
        self.pixels[usize::from(y) * self.width + usize::from(x)]
    }
}

/// Where the decode has got to: the sixel cursor is a column and a band
/// of six rows, and every character advances the column by one.
struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<Option<[u8; 3]>>,
    x: usize,
    band: usize,
}

impl Canvas {
    /// Paint `run` copies of one sixel character in `color`: bit `n` of
    /// `character - 0x3F` is row `n` of the current band.
    fn paint(&mut self, color: Option<[u8; 3]>, character: u8, run: u32) {
        let value = character.saturating_sub(EMPTY);
        for _ in 0..run {
            for row in 0..BAND {
                let y = self.band * BAND + row;
                if value & (1 << row) != 0 && y < self.height && self.x < self.width {
                    let at = y * self.width + self.x;
                    self.pixels[at] = color;
                }
            }
            self.x += 1;
        }
    }
}

/// The decimal number at `at`, leaving `at` on the byte after it.
fn number(bytes: &[u8], at: &mut usize) -> u32 {
    let mut value = 0;
    while let Some(digit) = bytes.get(*at).filter(|byte| byte.is_ascii_digit()) {
        value = value * 10 + u32::from(digit - b'0');
        *at += 1;
    }
    value
}

/// A `;`-separated list of `count` numbers starting at `at`.
fn numbers(bytes: &[u8], at: &mut usize, count: usize) -> Vec<u32> {
    let mut values = Vec::with_capacity(count);
    for n in 0..count {
        if n > 0 {
            assert_eq!(bytes.get(*at), Some(&b';'), "parameters are ;-separated");
            *at += 1;
        }
        values.push(number(bytes, at));
    }
    values
}

/// A colour register's 0..100 percentage back as a channel byte.
fn from_percent(value: u32) -> u8 {
    u8::try_from((value * 255 + 50) / 100).unwrap_or(u8::MAX)
}

/// Read a sixel DCS string back to pixels. This is the reference the
/// round-trip tests measure the encoder against, so it parses the format
/// rather than the encoder's habits: raster attributes, register
/// definitions and selections, the repeat introducer, `$` and `-`.
fn decode(bytes: &[u8]) -> Image {
    let body = bytes
        .strip_prefix(b"\x1bP")
        .expect("a sixel stream opens with DCS");
    let head = body
        .iter()
        .position(|byte| *byte == b'q')
        .expect("the DCS parameters end with q");
    let body = &body[head + 1..];

    let mut registers: HashMap<u32, [u8; 3]> = HashMap::new();
    let mut selected = 0;
    let mut canvas = Canvas {
        width: 0,
        height: 0,
        pixels: Vec::new(),
        x: 0,
        band: 0,
    };
    let mut at = 0;
    while at < body.len() {
        match body[at] {
            b'"' => {
                at += 1;
                let raster = numbers(body, &mut at, 4);
                canvas.width = raster[2] as usize;
                canvas.height = raster[3] as usize;
                canvas.pixels = vec![None; canvas.width * canvas.height];
            }
            b'#' => {
                at += 1;
                let register = number(body, &mut at);
                if body.get(at) == Some(&b';') {
                    at += 1;
                    let color = numbers(body, &mut at, 4);
                    assert_eq!(color[0], 2, "registers are defined in RGB");
                    registers.insert(
                        register,
                        [
                            from_percent(color[1]),
                            from_percent(color[2]),
                            from_percent(color[3]),
                        ],
                    );
                } else {
                    selected = register;
                }
            }
            b'!' => {
                at += 1;
                let run = number(body, &mut at);
                let character = *body.get(at).expect("a repeat introduces a character");
                at += 1;
                canvas.paint(registers.get(&selected).copied(), character, run);
            }
            b'$' => {
                at += 1;
                canvas.x = 0;
            }
            b'-' => {
                at += 1;
                canvas.x = 0;
                canvas.band += 1;
            }
            0x1b => break,
            character @ EMPTY..=0x7E => {
                at += 1;
                canvas.paint(registers.get(&selected).copied(), character, 1);
            }
            other => panic!("unexpected sixel byte {other:#04x}"),
        }
    }
    Image {
        width: canvas.width,
        pixels: canvas.pixels,
    }
}

#[test]
fn the_stream_is_a_well_formed_dcs_with_the_images_raster_size() {
    let bytes = encoded(PixelFormat::Rgb, 5, 9, &rgb(5, 9, |_, _| [255, 0, 0]));

    assert!(bytes.starts_with(b"\x1bP0;1;0q\"1;1;5;9"), "header");
    assert!(bytes.ends_with(b"\x1b\\"), "string terminator");
}

#[test]
fn a_flat_two_colour_image_round_trips_pixel_for_pixel() {
    let ink = [255, 0, 102];
    let paper = [0, 153, 204];
    let color = |x: u16, y: u16| {
        if (x + y).is_multiple_of(2) {
            ink
        } else {
            paper
        }
    };
    let pixels = rgb(8, 8, color);

    let image = decode(&encoded(PixelFormat::Rgb, 8, 8, &pixels));

    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(image.at(x, y), Some(color(x, y)), "({x},{y})");
        }
    }
}

#[test]
fn an_image_taller_than_one_band_round_trips_across_all_of_them() {
    // 13 rows is two full bands and one row of a third.
    let color = |x: u16, y: u16| {
        [
            EXACT[usize::from(x) % EXACT.len()],
            EXACT[usize::from(y) % EXACT.len()],
            EXACT[usize::from(x + y) % EXACT.len()],
        ]
    };
    let pixels = rgb(7, 13, color);

    let image = decode(&encoded(PixelFormat::Rgb, 7, 13, &pixels));

    for y in 0..13 {
        for x in 0..7 {
            assert_eq!(image.at(x, y), Some(color(x, y)), "({x},{y})");
        }
    }
}

#[test]
fn a_transparent_pixel_paints_nothing() {
    let opaque = [51, 102, 153];
    let pixel = |x: u16, _: u16| {
        if x < 4 {
            [opaque[0], opaque[1], opaque[2], 255]
        } else {
            [9, 9, 9, 0]
        }
    };
    let pixels = rgba(8, 6, pixel);

    let image = decode(&encoded(PixelFormat::Rgba, 8, 6, &pixels));

    for y in 0..6 {
        for x in 0..4 {
            assert_eq!(image.at(x, y), Some(opaque), "({x},{y})");
        }
        for x in 4..8 {
            assert_eq!(image.at(x, y), None, "({x},{y})");
        }
    }
}

#[test]
fn a_long_flat_run_uses_the_repeat_introducer() {
    let bytes = encoded(PixelFormat::Rgb, 600, 6, &rgb(600, 6, |_, _| [204, 0, 51]));

    assert!(bytes.contains(&b'!'), "a 600-column run is repeat-encoded");
    // One register, one pass, one run: nowhere near a character per column.
    assert!(bytes.len() < 100, "{} bytes for one run", bytes.len());
    let image = decode(&bytes);
    assert_eq!(image.at(0, 0), Some([204, 0, 51]));
    assert_eq!(image.at(599, 5), Some([204, 0, 51]));
}

#[test]
fn a_short_run_is_written_out_in_full() {
    // Columns 0..3 differ, so no run reaches the introducer's four.
    let color = |x: u16, _: u16| [EXACT[usize::from(x) % 3 + 1], 0, 0];
    let bytes = encoded(PixelFormat::Rgb, 3, 6, &rgb(3, 6, color));

    assert!(!bytes.contains(&b'!'), "runs of one are not introduced");
    let image = decode(&bytes);
    for x in 0..3 {
        assert_eq!(image.at(x, 0), Some(color(x, 0)));
    }
}

#[test]
fn a_gradient_past_the_register_ceiling_round_trips_within_the_cut() {
    let color = |x: u16, y: u16| {
        [
            u8::try_from(x * 8).unwrap_or(u8::MAX),
            u8::try_from(y * 8).unwrap_or(u8::MAX),
            128,
        ]
    };
    let pixels = rgb(32, 32, color);

    let image = decode(&encoded(PixelFormat::Rgb, 32, 32, &pixels));

    let mut worst = 0;
    for y in 0..32 {
        for x in 0..32 {
            let got = image.at(x, y).expect("every opaque pixel is painted");
            for (want, got) in color(x, y).into_iter().zip(got) {
                worst = worst.max(want.abs_diff(got));
            }
        }
    }
    assert!(worst <= 48, "worst channel error {worst} after the cut");
}

#[test]
fn an_image_with_no_pixels_is_refused() {
    let mut out = Vec::new();

    assert_eq!(
        encode(&mut out, PixelFormat::Rgb, 0, 4, &[]),
        Err(GraphicsError::EmptyImage)
    );
    assert!(out.is_empty(), "nothing is written for a refused image");
}

#[test]
fn a_buffer_that_does_not_match_the_dimensions_is_refused() {
    let mut out = Vec::new();

    assert_eq!(
        encode(&mut out, PixelFormat::Rgb, 4, 4, &[0; 12]),
        Err(GraphicsError::PixelCount {
            expected: 48,
            actual: 12,
        })
    );
    assert!(out.is_empty(), "nothing is written for a refused image");
}

#[test]
fn a_fully_transparent_image_is_a_header_and_nothing_else() {
    let bytes = encoded(PixelFormat::Rgba, 4, 6, &rgba(4, 6, |_, _| [1, 2, 3, 0]));

    assert_eq!(bytes, b"\x1bP0;1;0q\"1;1;4;6\x1b\\");
}
