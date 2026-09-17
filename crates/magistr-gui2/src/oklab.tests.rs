use super::*;

const WHITE: Srgba = Srgba::new(1.0, 1.0, 1.0, 1.0);
const BLACK: Srgba = Srgba::new(0.0, 0.0, 0.0, 1.0);

fn byte(c: f32) -> u8 {
    (c * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Mixing a color with itself is that color, at every share.
#[test]
fn a_color_mixed_with_itself_is_itself() {
    let mauve = Srgba::new(199.0 / 255.0, 164.0 / 255.0, 245.0 / 255.0, 1.0);
    for share in [0.0, 0.18, 0.5, 1.0] {
        let out = mix(mauve, mauve, share);
        assert_eq!(
            (byte(out.r), byte(out.g), byte(out.b)),
            (199, 164, 245),
            "share {share}"
        );
    }
}

/// The endpoint shares hand back the endpoints.
#[test]
fn the_shares_zero_and_one_are_the_endpoints() {
    let out = mix(WHITE, BLACK, 1.0);
    assert_eq!((byte(out.r), byte(out.g), byte(out.b)), (255, 255, 255));
    let out = mix(WHITE, BLACK, 0.0);
    assert_eq!((byte(out.r), byte(out.g), byte(out.b)), (0, 0, 0));
}

/// The oklab midpoint of white and black is the documented mid grey —
/// near #636363, and decisively lighter than the linear-light midpoint
/// (#bcbcbc) and darker than the naive sRGB one (#808080 would be the
/// sRGB-space answer; oklab's perceptual midpoint sits below it).
#[test]
fn the_white_black_midpoint_is_oklabs_own_grey() {
    let out = mix(WHITE, BLACK, 0.5);
    let grey = byte(out.r);
    assert_eq!(byte(out.g), grey);
    assert_eq!(byte(out.b), grey);
    assert!(
        (grey as i32 - 0x63).abs() <= 2,
        "oklab mid grey should be ~#636363, got {grey:#x}"
    );
}

/// Mixing toward transparent keeps the color's own chroma — CSS's
/// premultiplied interpolation, the identity `--hairc` relies on.
#[test]
fn a_mix_with_transparent_keeps_the_chroma_at_reduced_alpha() {
    let s1 = Srgba::new(65.0 / 255.0, 65.0 / 255.0, 96.0 / 255.0, 1.0);
    let out = mix(s1, Srgba::new(0.0, 0.0, 0.0, 0.0), 0.4);
    assert!((out.a - 0.4).abs() < 1e-6);
    assert_eq!(
        (byte(out.r), byte(out.g), byte(out.b)),
        (65, 65, 96),
        "the chromatic channels must survive the alpha"
    );
}
