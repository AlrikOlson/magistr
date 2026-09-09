use super::*;

/// An RGB buffer from a list of colours, one pixel each.
fn rgb(colors: &[[u8; 3]]) -> Vec<u8> {
    colors.iter().flatten().copied().collect()
}

/// An RGBA buffer from `(colour, alpha)` pairs.
fn rgba(pixels: &[([u8; 3], u8)]) -> Vec<u8> {
    pixels
        .iter()
        .flat_map(|([r, g, b], a)| [*r, *g, *b, *a])
        .collect()
}

/// The colour a palette resolves pixel `n` to.
fn resolved(palette: &Palette, n: usize) -> Option<[u8; 3]> {
    palette.indices()[n].map(|i| palette.colors()[usize::from(i)])
}

#[test]
fn an_image_with_few_colours_keeps_them_exactly() {
    let red = [255, 0, 0];
    let blue = [0, 0, 255];
    let palette = Palette::quantize(&rgb(&[red, blue, red, red]), PixelFormat::Rgb);

    assert_eq!(palette.colors().len(), 2);
    assert_eq!(resolved(&palette, 0), Some(red));
    assert_eq!(resolved(&palette, 1), Some(blue));
    assert_eq!(resolved(&palette, 2), Some(red));
    assert_eq!(resolved(&palette, 3), Some(red));
}

#[test]
fn every_pixel_of_an_exact_image_round_trips() {
    let colors: Vec<[u8; 3]> = (0..200u16)
        .map(|n| {
            let n = u8::try_from(n).unwrap_or(u8::MAX);
            [n, 255 - n, n / 2]
        })
        .collect();
    let palette = Palette::quantize(&rgb(&colors), PixelFormat::Rgb);

    assert!(palette.colors().len() <= Palette::MAX);
    for (n, color) in colors.iter().enumerate() {
        assert_eq!(resolved(&palette, n), Some(*color), "pixel {n}");
    }
}

#[test]
fn a_transparent_pixel_has_no_index_and_no_colour() {
    let palette = Palette::quantize(
        &rgba(&[([9, 9, 9], 0), ([1, 2, 3], 255), ([7, 7, 7], 127)]),
        PixelFormat::Rgba,
    );

    assert_eq!(resolved(&palette, 0), None);
    assert_eq!(resolved(&palette, 1), Some([1, 2, 3]));
    // Below the alpha floor, so it does not reach the terminal either.
    assert_eq!(resolved(&palette, 2), None);
    assert_eq!(palette.colors(), &[[1, 2, 3]]);
}

#[test]
fn a_fully_transparent_image_has_an_empty_palette() {
    let palette = Palette::quantize(&rgba(&[([9, 9, 9], 0), ([4, 4, 4], 0)]), PixelFormat::Rgba);

    assert!(palette.colors().is_empty());
    assert!(palette.indices().iter().all(Option::is_none));
}

/// A 32×32×32 sweep — 32 768 pixels over as many distinct 5:5:5 cells,
/// far past the 256 registers sixel allows.
fn sweep() -> Vec<[u8; 3]> {
    let mut colors = Vec::with_capacity(32 * 32 * 32);
    for r in 0..32u8 {
        for g in 0..32u8 {
            for b in 0..32u8 {
                colors.push([r * 8, g * 8, b * 8]);
            }
        }
    }
    colors
}

#[test]
fn a_full_colour_sweep_is_cut_down_to_the_sixel_ceiling() {
    let colors = sweep();
    let palette = Palette::quantize(&rgb(&colors), PixelFormat::Rgb);

    assert_eq!(palette.colors().len(), Palette::MAX);
    assert!(palette.indices().iter().all(Option::is_some));
}

#[test]
fn median_cut_keeps_every_pixel_near_its_own_colour() {
    let colors = sweep();
    let palette = Palette::quantize(&rgb(&colors), PixelFormat::Rgb);

    // 256 boxes over the cube leaves at most an eighth of the range per
    // axis; anything worse than a quarter would be a broken split.
    let mut worst = 0u8;
    for (n, want) in colors.iter().enumerate() {
        let got = resolved(&palette, n).expect("an opaque pixel has an index");
        for channel in 0..3 {
            worst = worst.max(want[channel].abs_diff(got[channel]));
        }
    }
    assert!(worst <= 64, "worst channel error {worst} is too coarse");
}

#[test]
fn quantisation_is_deterministic() {
    let colors = sweep();
    let first = Palette::quantize(&rgb(&colors), PixelFormat::Rgb);
    let second = Palette::quantize(&rgb(&colors), PixelFormat::Rgb);

    assert_eq!(first.colors(), second.colors());
    assert_eq!(first.indices(), second.indices());
}

#[test]
fn an_empty_buffer_quantises_to_nothing() {
    let palette = Palette::quantize(&[], PixelFormat::Rgb);

    assert!(palette.colors().is_empty());
    assert!(palette.indices().is_empty());
}

#[test]
fn a_trailing_partial_pixel_is_ignored() {
    let mut pixels = rgb(&[[1, 2, 3]]);
    pixels.push(9);
    let palette = Palette::quantize(&pixels, PixelFormat::Rgb);

    assert_eq!(palette.indices().len(), 1);
    assert_eq!(palette.colors(), &[[1, 2, 3]]);
}
